//! Flash mode: `s`, then characters, then a label jumps to that text anywhere on screen.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::app::command::{CommandId, Scope};
use crate::tui::app::App;
use crate::tui::app::registrar::{Registrar, RegistrationError, command};
use crate::tui::flash::{FlashMatch, ScreenRow, assign_labels, find_matches, read_screen};
use crate::tui::keymap::mode::Mode;
use crate::tui::layout::{Panel, ScreenAreas};
use crate::tui::main_panel::MainView;

/// Opens flash mode.
pub const FLASH: CommandId = CommandId("flash.open");
/// The prompt drawn in the status line while flash mode is open.
pub const FLASH_PROMPT: &str = "⚡ ";

/// The state of an open flash: the typed characters and what they match on screen.
#[derive(Debug, Clone, Default)]
pub struct Flash {
    pattern: String,
    rows: Vec<ScreenRow>,
    matches: Vec<FlashMatch>,
}

impl Flash {
    /// Returns the typed characters.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Returns the matches, nearest first, with their labels.
    pub fn matches(&self) -> &[FlashMatch] {
        &self.matches
    }
}

pub(super) fn register(reg: &mut Registrar<'_>) -> Result<(), RegistrationError> {
    reg.add(
        Mode::Normal,
        &["s"],
        command(
            FLASH,
            Scope::GLOBAL,
            "Jump to text anywhere on screen",
            None,
            App::open_flash,
        ),
    )
}

/// Returns the area inside a panel's border.
fn inside(rect: Rect) -> Rect {
    Rect {
        x: rect.x.saturating_add(1),
        y: rect.y.saturating_add(1),
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    }
}

impl App {
    /// Returns the open flash, if any.
    pub fn flash(&self) -> Option<&Flash> {
        self.flash.as_ref()
    }

    /// Returns the text area of every panel on screen, as flash searches it.
    fn flash_areas(&self, areas: &ScreenAreas) -> Vec<(Panel, Rect)> {
        let search = inside(areas.search);
        // The search bar draws its text one column in from the border.
        let search = Rect {
            x: search.x.saturating_add(1),
            width: search.width.saturating_sub(1),
            ..search
        };
        let mut panels = vec![(Panel::SearchBar, search)];
        if let Some(history) = areas.overlay.or(areas.history) {
            panels.push((Panel::History, inside(history)));
        }
        panels.push((Panel::Main, inside(areas.main)));
        if let Some(detail) = areas.detail {
            panels.push((Panel::Detail, inside(detail)));
        }
        panels
    }

    /// Draws the screen without flash into a buffer and reads every panel's text from it.
    fn read_flash_screen(&self) -> Vec<ScreenRow> {
        let backend = TestBackend::new(self.area.width, self.area.height);
        let Ok(mut terminal) = Terminal::new(backend);
        let Ok(_) = terminal.draw(|frame| self.render_screen(frame));
        read_screen(
            terminal.backend().buffer(),
            &self.flash_areas(&self.areas()),
        )
    }

    /// Returns the screen position flash measures distances from: the focused panel's
    /// current line, at the left of its text area.
    fn flash_cursor(&mut self) -> (u16, u16) {
        let areas = self.areas();
        let area = self
            .flash_areas(&areas)
            .into_iter()
            .find(|(panel, _)| *panel == self.focus)
            .map_or(Rect::default(), |(_, area)| area);
        let row = self.focused_pane().map_or(0, |(pane, viewport)| {
            pane.current_line()
                .saturating_sub(pane.first_visible(viewport))
        });
        let row = u16::try_from(row).unwrap_or(0);
        (area.x, area.y.saturating_add(row))
    }

    fn open_flash(&mut self) {
        self.flash = Some(Flash {
            rows: self.read_flash_screen(),
            ..Flash::default()
        });
    }

    /// Reads the screen again and recomputes the matches, after the panels changed.
    pub(super) fn refresh_flash(&mut self) {
        if self.flash.is_none() {
            return;
        }
        let rows = self.read_flash_screen();
        if let Some(flash) = self.flash.as_mut() {
            flash.rows = rows;
        }
        self.update_flash_matches();
    }

    fn update_flash_matches(&mut self) {
        let cursor = self.flash_cursor();
        let focus = self.focus;
        if let Some(flash) = self.flash.as_mut() {
            flash.matches = find_matches(&flash.rows, &flash.pattern);
            assign_labels(&mut flash.matches, focus, cursor);
        }
    }

    /// Handles a key while flash is open: `<Esc>` closes it, `<Enter>` jumps to the nearest
    /// match, `<BS>` deletes a character, a label jumps, and any other character narrows the
    /// search.
    pub(super) fn handle_flash_key(&mut self, event: KeyEvent) {
        let Some(flash) = self.flash.as_mut() else {
            return;
        };
        if event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return;
        }
        match event.code {
            KeyCode::Esc => self.flash = None,
            KeyCode::Enter => {
                let target = flash.matches.first().cloned();
                self.flash = None;
                if let Some(target) = target {
                    self.flash_jump(&target);
                }
            }
            KeyCode::Backspace => {
                if flash.pattern.pop().is_none() {
                    self.flash = None;
                } else {
                    self.update_flash_matches();
                }
            }
            KeyCode::Char(c) => {
                let target = flash.matches.iter().find(|m| m.label == Some(c)).cloned();
                match target {
                    Some(target) => {
                        self.flash = None;
                        self.flash_jump(&target);
                    }
                    None => {
                        flash.pattern.push(c);
                        self.update_flash_matches();
                    }
                }
            }
            _ => {}
        }
    }

    /// Moves the focus and cursor to `target`.
    fn flash_jump(&mut self, target: &FlashMatch) {
        if target.panel == Panel::SearchBar {
            let areas = self.areas();
            let Some((_, area)) = self
                .flash_areas(&areas)
                .into_iter()
                .find(|(panel, _)| *panel == Panel::SearchBar)
            else {
                return;
            };
            self.set_focus(Panel::SearchBar);
            self.search_bar.set_cursor_at_column(
                usize::from(target.columns.0 - area.x),
                usize::from(area.width),
            );
            self.enter_insert_mode();
            return;
        }
        if target.panel == Panel::Detail && self.main_view != MainView::Split {
            return;
        }
        self.set_focus(target.panel);
        if let Some((pane, viewport)) = self.focused_pane() {
            let line = pane.first_visible(viewport) + usize::from(target.row);
            if line < pane.search_lines().len() {
                pane.jump_to(line, viewport);
            }
        }
    }

    /// Draws flash over `buf`, the drawn screen: every panel's text dimmed, the matches
    /// highlighted with the nearest in its own style, and each label on the cell after its
    /// match, or on the match's last cell at the panel's edge.
    pub(super) fn render_flash(&self, buf: &mut Buffer, areas: &ScreenAreas) {
        let Some(flash) = &self.flash else {
            return;
        };
        let panels = self.flash_areas(areas);
        for (_, area) in &panels {
            buf.set_style(*area, self.theme.flash_backdrop);
        }
        for (i, m) in flash.matches.iter().enumerate() {
            let style = if i == 0 {
                self.theme.flash_current
            } else {
                self.theme.flash_match
            };
            let (first, last) = m.columns;
            let cells = Rect::new(first, m.y, last - first + 1, 1);
            buf.set_style(cells, style);
            let Some(label) = m.label else { continue };
            let right = panels
                .iter()
                .find(|(panel, _)| *panel == m.panel)
                .map_or(last + 1, |(_, area)| area.right());
            let x = if last + 1 < right { last + 1 } else { last };
            buf.set_string(x, m.y, label.to_string(), self.theme.flash_label);
        }
        let status = areas.status;
        ratatui::widgets::Clear.render(status, buf);
        Line::from(format!("{FLASH_PROMPT}{}", flash.pattern)).render(status, buf);
    }
}
