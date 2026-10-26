//! The config window on `<leader>c`: settings change for the session as they are chosen, and
//! `<Enter>` saves them to the config file.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::command::{CommandId, Scope};
use crate::app::config_file;
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

/// Shown when `<Enter>` finds nothing that differs from the config file.
pub const NOTHING_TO_SAVE: &str = "Nothing to save: the settings match the config file";
/// Shown when `<Enter>` has no config file to save to, which happens when the home directory
/// is unknown.
pub const NO_CONFIG_FILE: &str = "No config file location is known, so nothing was saved";

/// The keys the window answers to, as the status line lists them.
const HINTS: [&str; 6] = [
    "j/k move",
    "⇥ next value",
    "i/a edit",
    "⏎ save",
    "<Esc> keep",
    "q cancel",
];
/// The keys that end typing a value.
const EDITING_HINTS: [&str; 1] = ["⏎/<Esc> done"];

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
    /// Shift-Tab cycle the selected setting's presets, `i`/`I` and `a`/`A` start typing its
    /// value at the start or the end, `<Enter>` saves the changes and closes the window,
    /// `<Esc>` closes it keeping the changes for the session, and `q` closes it undoing them.
    /// While a value is typed, `<Esc>` and `<Enter>` end the edit and other keys edit the
    /// text. Other keys do nothing.
    pub(super) fn handle_config_key(&mut self, event: KeyEvent) {
        let Some(window) = self.config_window.as_mut() else {
            return;
        };
        if let Some(input) = window.editing_mut() {
            match event.code {
                KeyCode::Esc | KeyCode::Enter => self.finish_typing(),
                _ => {
                    input.handle_key(event);
                }
            }
            return;
        }
        match event.code {
            KeyCode::Char(c @ ('i' | 'I' | 'a' | 'A')) => {
                let value = window.selected().value(&self.config);
                window.start_editing(&value, matches!(c, 'a' | 'A'));
                self.mode = Mode::Insert;
            }
            KeyCode::Enter => self.save_settings(),
            KeyCode::Char('j') | KeyCode::Down => window.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => window.move_by(-1),
            KeyCode::Tab => self.cycle_setting(1),
            KeyCode::BackTab => self.cycle_setting(-1),
            KeyCode::Esc => self.config_window = None,
            KeyCode::Char('q') => self.cancel_config_window(),
            _ => {}
        }
    }

    /// Applies the typed value to the selected setting and returns to the window's normal
    /// mode. A value the setting refuses leaves it as it was, and the status line says why.
    fn finish_typing(&mut self) {
        self.mode = Mode::Normal;
        let Some(window) = self.config_window.as_mut() else {
            return;
        };
        let setting = window.selected();
        let Some(text) = window.finish_editing() else {
            return;
        };
        match self.apply_setting(setting, &text) {
            Ok(()) => {
                let value = setting.value(&self.config);
                if let Some(window) = self.config_window.as_mut() {
                    window.remember_typed(setting, value);
                }
            }
            Err(err) => self.message = Some(StatusMessage::error(err.to_string())),
        }
    }

    /// Saves every setting that differs from the config file into it, and closes the window.
    /// A failure leaves the window open with the reason in the status line.
    fn save_settings(&mut self) {
        let changed: Vec<Setting> = Setting::ALL
            .into_iter()
            .filter(|s| s.value(&self.config) != s.value(&self.saved))
            .collect();
        if changed.is_empty() {
            self.config_window = None;
            self.message = Some(StatusMessage::info(NOTHING_TO_SAVE));
            return;
        }
        let Some(path) = self.config_path.clone() else {
            self.message = Some(StatusMessage::error(NO_CONFIG_FILE));
            return;
        };
        match config_file::save(&path, &self.config, &changed) {
            Ok(()) => {
                self.saved = self.config.clone();
                self.config_window = None;
                let settings = if changed.len() == 1 {
                    "setting"
                } else {
                    "settings"
                };
                self.message = Some(StatusMessage::info(format!(
                    "Saved {} {settings} to {}",
                    changed.len(),
                    home_relative(&path)
                )));
            }
            Err(err) => self.message = Some(StatusMessage::error(err.to_string())),
        }
    }

    /// Draws the value being typed, with the terminal cursor, over the window drawn in
    /// `body`.
    pub(super) fn render_config_input(&self, frame: &mut Frame, body: Rect) {
        let Some(window) = &self.config_window else {
            return;
        };
        if let Some(input) = window.editing() {
            input.render(frame, window.value_area(body, &self.config), true);
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
        let typing = self
            .config_window
            .as_ref()
            .is_some_and(|w| w.editing().is_some());
        let hints: &[&str] = if typing { &EDITING_HINTS } else { &HINTS };
        hints.iter().map(|h| (*h).to_owned()).collect()
    }
}

/// Returns `path` with the home directory written `~`, as people write config paths.
fn home_relative(path: &Path) -> String {
    match etcetera::home_dir()
        .ok()
        .and_then(|home| path.strip_prefix(home).ok().map(Path::to_owned))
    {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
