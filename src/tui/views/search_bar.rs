//! The search bar at the top of the screen.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::tui::layout::panel_block;
use crate::tui::widgets::query_input::QueryInput;

/// The query input of the search bar.
#[derive(Debug, Clone, Default)]
pub struct SearchBar {
    input: QueryInput,
}

impl SearchBar {
    /// Creates an empty search bar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the query as typed.
    pub fn query(&self) -> &str {
        self.input.value()
    }

    /// Replaces the query and puts the cursor at its end.
    pub fn set_query(&mut self, query: &str) {
        self.input.set_value(query);
    }

    /// Removes the query.
    pub fn clear(&mut self) {
        self.input.clear();
    }

    /// Applies an editing key.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        self.input.handle_key(key);
    }

    /// Draws the bar with `title`, with a heavy border when `focused` and the terminal cursor
    /// when `editing`.
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: String,
        focused: bool,
        editing: bool,
    ) {
        let block = panel_block(title, focused);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let inner = Rect {
            x: inner.x + 1,
            width: inner.width.saturating_sub(1),
            ..inner
        };
        self.input.render(frame, inner, editing);
    }
}
