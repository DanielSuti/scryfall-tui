//! The config window on `<leader>c`: settings change for the session as they are chosen.

use crossterm::event::{KeyCode, KeyEvent};

use crate::app::command::{CommandId, Scope};
use crate::app::settings::{Setting, SettingError};
use crate::tui::app::App;
use crate::tui::app::registrar::{Registrar, RegistrationError, command};
use crate::tui::keymap::mode::Mode;
use crate::tui::views::config_window::ConfigWindow;
use crate::tui::widgets::status_line::StatusMessage;

/// Opens the config window.
pub const OPEN_CONFIG: CommandId = CommandId("config.open");

/// Shown when images are switched on while the terminal was never asked for a graphics
/// protocol, because both views were set to `never` at start-up.
pub const IMAGES_NEED_RESTART: &str =
    "Images show after a restart: the terminal was not asked for a graphics protocol at start-up";

/// The keys the window answers to, as the status line lists them.
const HINTS: [&str; 4] = ["j/k move", "⇥ next value", "<Esc> keep", "q cancel"];

pub(super) fn register(reg: &mut Registrar<'_>) -> Result<(), RegistrationError> {
    reg.add(
        Mode::Normal,
        &["<leader>c"],
        command(
            OPEN_CONFIG,
            Scope::GLOBAL,
            "Change settings",
            None,
            App::open_config_window,
        ),
    )
}

impl App {
    /// Returns the open config window, if any.
    pub fn config_window(&self) -> Option<&ConfigWindow> {
        self.config_window.as_ref()
    }

    fn open_config_window(&mut self) {
        self.config_window = Some(ConfigWindow::new(&self.config));
    }

    /// Handles a key while the config window is open: `j`/`k` and the arrows move, Tab and
    /// Shift-Tab cycle the selected setting's presets, `<Esc>` closes the window keeping the
    /// changes for the session, and `q` closes it undoing them. Other keys do nothing.
    pub(super) fn handle_config_key(&mut self, event: KeyEvent) {
        let Some(window) = self.config_window.as_mut() else {
            return;
        };
        match event.code {
            KeyCode::Char('j') | KeyCode::Down => window.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => window.move_by(-1),
            KeyCode::Tab => self.cycle_setting(1),
            KeyCode::BackTab => self.cycle_setting(-1),
            KeyCode::Esc => self.config_window = None,
            KeyCode::Char('q') => self.cancel_config_window(),
            _ => {}
        }
    }

    /// Sets the selected setting to the preset `step` places from its value.
    fn cycle_setting(&mut self, step: isize) {
        let Some(window) = &self.config_window else {
            return;
        };
        let setting = window.selected();
        let current = setting.value(&self.config);
        let next = setting.cycle(&current, window.typed(setting), step);
        if let Err(err) = self.apply_setting(setting, &next) {
            self.message = Some(StatusMessage::error(err.to_string()));
        }
    }

    /// Closes the window and sets back every setting changed since it opened.
    fn cancel_config_window(&mut self) {
        let Some(window) = self.config_window.take() else {
            return;
        };
        for setting in Setting::ALL {
            let opened = setting.value(window.opened_with());
            if setting.value(&self.config) != opened {
                // The value came from a valid configuration, so it is accepted again.
                let _ = self.apply_setting(setting, &opened);
            }
        }
    }

    /// Sets `setting` from `text` for the session, and brings what depends on it up to date:
    /// the history's length, the format check, and a note when images can't show before a
    /// restart. The detail follows on the next sync.
    ///
    /// # Errors
    /// Returns the [`SettingError`] when `text` is not a value `setting` accepts.
    pub(super) fn apply_setting(
        &mut self,
        setting: Setting,
        text: &str,
    ) -> Result<(), SettingError> {
        setting.set(&mut self.config, text)?;
        match setting {
            Setting::MaxEntries => {
                self.history
                    .set_max_entries(self.config.history.max_entries);
                self.history_changed();
            }
            Setting::Formats => {
                self.formats_checked = false;
                self.format_warning = None;
                if let Some(card) = self.selection().map(|s| s.card.clone()) {
                    self.check_formats(&card);
                }
            }
            Setting::ImagesSplit | Setting::ImagesFullDetail
                if !self.graphics_asked && !self.config.detail.images.is_never() =>
            {
                self.message = Some(StatusMessage::info(IMAGES_NEED_RESTART));
            }
            _ => {}
        }
        Ok(())
    }

    /// Returns how many settings differ from the config file.
    pub fn unsaved_settings(&self) -> usize {
        Setting::ALL
            .iter()
            .filter(|s| s.value(&self.config) != s.value(&self.saved))
            .count()
    }

    /// Returns the status-line hints while the config window is open.
    pub(super) fn config_hints(&self) -> Vec<String> {
        HINTS.iter().map(|h| (*h).to_owned()).collect()
    }
}
