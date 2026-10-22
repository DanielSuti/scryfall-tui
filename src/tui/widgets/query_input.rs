//! A single-line text input.

use crossterm::event::{Event, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::Paragraph;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

/// A single-line text input with a cursor, edited with the usual line-editing keys.
#[derive(Debug, Clone, Default)]
pub struct QueryInput {
    input: Input,
}

impl QueryInput {
    /// Creates an empty input.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current text.
    pub fn value(&self) -> &str {
        self.input.value()
    }

    /// Replaces the text and puts the cursor at its end.
    pub fn set_value(&mut self, value: &str) {
        self.input = Input::new(value.to_owned());
    }

    /// Removes all text.
    pub fn clear(&mut self) {
        self.input.reset();
    }

    /// Applies an editing key. Returns true when the text or the cursor changed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.input.handle_event(&Event::Key(key)).is_some()
    }

    /// Draws the text into `area`, scrolled so the cursor stays visible, and places the
    /// terminal cursor when `show_cursor` is true.
    pub fn render(&self, frame: &mut Frame, area: Rect, show_cursor: bool) {
        let width = usize::from(area.width.max(1));
        let scroll = self.input.visual_scroll(width.saturating_sub(1));
        let scroll_cols = u16::try_from(scroll).unwrap_or(u16::MAX);
        frame.render_widget(
            Paragraph::new(self.input.value()).scroll((0, scroll_cols)),
            area,
        );
        if show_cursor {
            let cursor = self.input.visual_cursor().saturating_sub(scroll);
            let x = area.x + u16::try_from(cursor).unwrap_or(u16::MAX).min(area.width);
            frame.set_cursor_position(Position::new(x, area.y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn press(input: &mut QueryInput, code: KeyCode) {
        input.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn typing_and_backspace_edit_the_text() {
        let mut input = QueryInput::new();
        for c in "t:dragonx".chars() {
            press(&mut input, KeyCode::Char(c));
        }
        press(&mut input, KeyCode::Backspace);
        assert_eq!(input.value(), "t:dragon");
    }

    #[test]
    fn set_value_puts_the_cursor_at_the_end() {
        let mut input = QueryInput::new();
        input.set_value("o:draw");
        press(&mut input, KeyCode::Char('!'));
        assert_eq!(input.value(), "o:draw!");
    }
}
