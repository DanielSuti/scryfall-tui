//! The config window on `<leader>c`: every editable setting with its value in the session,
//! a cursor, and a mark on the values that differ from the config file.

use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Widget};

use crate::app::config::Config;
use crate::app::settings::Setting;
use crate::tui::text::{display_width, truncate};
use crate::tui::theme::Theme;
use crate::tui::widgets::query_input::QueryInput;

/// The window's title.
pub const CONFIG_TITLE: &str = "Config";
/// Marks a value that differs from the config file.
pub const CHANGED_MARK: &str = "•";
/// Shown for an empty value, such as no base terms.
pub const EMPTY_VALUE: &str = "(none)";
/// Values longer than this many columns are cut with `...`.
pub const MAX_VALUE_WIDTH: usize = 40;

/// The window's state while it is open.
#[derive(Debug, Clone)]
pub struct ConfigWindow {
    cursor: usize,
    opened_with: Config,
    typed: HashMap<Setting, String>,
    editing: Option<QueryInput>,
}

/// Where the window's parts go in an area.
struct Layout {
    popup: Rect,
    label_width: usize,
    value_width: usize,
    values: Vec<String>,
}

impl ConfigWindow {
    /// Opens the window on the first setting. `config` is kept, so `q` can restore it.
    pub fn new(config: &Config) -> Self {
        let typed = Setting::ALL
            .into_iter()
            .map(|s| (s, s.value(config)))
            .filter(|(s, value)| !s.presets().contains(value))
            .collect();
        Self {
            cursor: 0,
            opened_with: config.clone(),
            typed,
            editing: None,
        }
    }

    /// Returns the setting under the cursor.
    pub fn selected(&self) -> Setting {
        Setting::ALL[self.cursor]
    }

    /// Moves the cursor `step` settings down, or up when negative, stopping at the ends.
    pub fn move_by(&mut self, step: isize) {
        let last = Setting::ALL.len() - 1;
        self.cursor = self.cursor.saturating_add_signed(step).min(last);
    }

    /// Returns the configuration as it was when the window opened.
    pub fn opened_with(&self) -> &Config {
        &self.opened_with
    }

    /// Returns the value typed for `setting` that is not one of its presets, if any. Tab
    /// cycles through it as well.
    pub fn typed(&self, setting: Setting) -> Option<&str> {
        self.typed.get(&setting).map(String::as_str)
    }

    /// Remembers `value` as typed for `setting` unless it is a preset.
    pub fn remember_typed(&mut self, setting: Setting, value: String) {
        if !setting.presets().contains(&value) {
            self.typed.insert(setting, value);
        }
    }

    /// Starts editing the selected setting's value `value`, with the text cursor at its end
    /// when `at_end` is true, else at its start.
    pub fn start_editing(&mut self, value: &str, at_end: bool) {
        let mut input = QueryInput::new();
        input.set_value(value);
        if !at_end {
            input.set_cursor_at_column(0, MAX_VALUE_WIDTH);
        }
        self.editing = Some(input);
    }

    /// Returns the text being typed, while a value is edited.
    pub fn editing(&self) -> Option<&QueryInput> {
        self.editing.as_ref()
    }

    /// Returns the text being typed, to apply a key to it.
    pub fn editing_mut(&mut self) -> Option<&mut QueryInput> {
        self.editing.as_mut()
    }

    /// Stops editing and returns the typed text, or `None` when nothing was being edited.
    pub fn finish_editing(&mut self) -> Option<String> {
        self.editing.take().map(|input| input.value().to_owned())
    }

    /// Returns the cell row where the selected value is typed when the window is drawn into
    /// `area` with the values of `config`.
    pub fn value_area(&self, area: Rect, config: &Config) -> Rect {
        let layout = self.layout(area, config);
        let x = layout.popup.x + 2 + u16::try_from(layout.label_width + 2).unwrap_or(u16::MAX);
        let y = layout.popup.y + 1 + u16::try_from(self.cursor).unwrap_or(u16::MAX);
        let width = u16::try_from(layout.value_width).unwrap_or(u16::MAX);
        Rect::new(x, y, width, 1).intersection(layout.popup)
    }

    fn layout(&self, area: Rect, config: &Config) -> Layout {
        let label_width = Setting::ALL
            .iter()
            .map(|s| display_width(s.label()))
            .max()
            .unwrap_or(0);
        let values: Vec<String> = Setting::ALL
            .iter()
            .map(|s| match s.value(config) {
                value if value.is_empty() => EMPTY_VALUE.to_owned(),
                value => truncate(&value, MAX_VALUE_WIDTH),
            })
            .collect();
        let mut value_width = values.iter().map(|v| display_width(v)).max().unwrap_or(0);
        if self.editing.is_some() {
            // Room to type; longer text scrolls.
            value_width = value_width.max(MAX_VALUE_WIDTH);
        }
        // A space each side of the label, two before the value, and the mark with its space.
        let width = label_width + value_width + 6;
        let width = u16::try_from(width + 2).unwrap_or(u16::MAX).min(area.width);
        let height = u16::try_from(Setting::ALL.len() + 2)
            .unwrap_or(u16::MAX)
            .min(area.height);
        let popup = Rect {
            x: area.right() - width,
            y: area.bottom() - height,
            width,
            height,
        };
        Layout {
            popup,
            label_width,
            value_width,
            values,
        }
    }

    /// Draws the window in the bottom-right corner of `area`, with the values of `config`.
    /// Values that differ from `saved`, the config file's, are marked with [`CHANGED_MARK`].
    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        config: &Config,
        saved: &Config,
        theme: &Theme,
    ) {
        let Layout {
            popup,
            label_width,
            value_width,
            values,
        } = self.layout(area, config);
        Clear.render(popup, buf);
        let block = Block::bordered()
            .border_type(BorderType::Thick)
            .border_style(theme.focus_border)
            .title(format!(" {CONFIG_TITLE} "))
            .title_style(theme.focus_border);
        let inner = block.inner(popup);
        block.render(popup, buf);
        for (index, (setting, value)) in Setting::ALL.iter().zip(&values).enumerate() {
            let Ok(row) = u16::try_from(index) else { break };
            if row >= inner.height {
                break;
            }
            let changed = setting.value(config) != setting.value(saved);
            let mark = if changed { CHANGED_MARK } else { " " };
            // The value being typed is drawn over this one, by the caller.
            let editing = index == self.cursor && self.editing.is_some();
            let value = if editing { "" } else { value.as_str() };
            let text = format!(
                " {:label_width$}  {:value_width$} {mark}",
                setting.label(),
                value
            );
            let style = if index == self.cursor {
                theme.selected_row
            } else {
                Style::default()
            };
            let row_area = Rect {
                y: inner.y + row,
                height: 1,
                ..inner
            };
            buf.set_style(row_area, style);
            Line::from(Span::styled(text, style)).render(row_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::config::LoadMode;

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn the_cursor_stops_at_both_ends() {
        let mut window = ConfigWindow::new(&Config::default());
        assert_eq!(window.selected(), Setting::Base);
        window.move_by(-1);
        assert_eq!(window.selected(), Setting::Base);
        window.move_by(4);
        assert_eq!(window.selected(), Setting::RulingsSplit);
        window.move_by(20);
        assert_eq!(window.selected(), Setting::ImagesPause);
    }

    #[test]
    fn values_that_are_not_presets_join_the_cycle() {
        let mut config = Config::default();
        config.search.max_pages = 3.try_into().unwrap();
        let mut window = ConfigWindow::new(&config);
        assert_eq!(window.typed(Setting::MaxPages), Some("3"));
        assert_eq!(window.typed(Setting::MaxEntries), None, "40 is a preset");
        window.remember_typed(Setting::MaxEntries, "70".into());
        window.remember_typed(Setting::RulingsSplit, "auto".into());
        assert_eq!(window.typed(Setting::MaxEntries), Some("70"));
        assert_eq!(window.typed(Setting::RulingsSplit), None);
    }

    #[test]
    fn changed_values_are_marked_and_the_cursor_row_is_highlighted() {
        let saved = Config::default();
        let mut config = saved.clone();
        config.detail.rulings.split = LoadMode::Never;
        let mut window = ConfigWindow::new(&config);
        window.move_by(4);
        let area = Rect::new(0, 0, 80, 14);
        let mut buf = Buffer::empty(area);
        let theme = Theme::default();
        window.render(area, &mut buf, &config, &saved, &theme);
        // The window is 12 rows high at the bottom of the area: its title is on row 2.
        assert!(row(&buf, 2).contains(" Config "), "{}", row(&buf, 2));
        let rulings = row(&buf, 7);
        assert!(rulings.contains("Rulings in the split"), "{rulings}");
        assert!(rulings.contains("never "), "{rulings}");
        assert!(
            rulings.trim_end().ends_with(&format!("{CHANGED_MARK} ┃")),
            "the mark stands at the right edge: {rulings}"
        );
        assert!(!row(&buf, 8).contains(CHANGED_MARK));
        let label_x = (0..80).find(|x| buf[(*x, 7)].symbol() == "R").unwrap();
        assert_eq!(buf[(label_x, 7)].bg, theme.selected_row.bg.unwrap());
        assert_ne!(buf[(label_x, 8)].bg, theme.selected_row.bg.unwrap());
    }
}
