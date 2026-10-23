//! The search bar at the top of the screen.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::tui::text::truncate;
use crate::tui::theme::Theme;
use crate::tui::widgets::query_input::QueryInput;
use ratatui::widgets::Block;

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

    /// Puts the text cursor on the character drawn at `column` of a text area `width` columns
    /// wide.
    pub fn set_cursor_at_column(&mut self, column: usize, width: usize) {
        self.input.set_cursor_at_column(column, width);
    }

    /// Removes the query.
    pub fn clear(&mut self) {
        self.input.clear();
    }

    /// Applies an editing key.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        self.input.handle_key(key);
    }

    /// Draws the bar inside `block`, the panel's border and title, with the terminal cursor
    /// when `editing`. Non-empty `base` terms, which every search adds, are shown in
    /// `theme.dim` on the right of the top border as `+ base`, cut with `...` to at most half
    /// the width.
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        block: Block<'static>,
        base: &str,
        editing: bool,
        theme: &Theme,
    ) {
        let mut block = block;
        let base = base.trim();
        if !base.is_empty() {
            let room = usize::from(area.width / 2).saturating_sub(5);
            let note = format!(" + {} ", truncate(base, room));
            block = block.title_top(Line::from(note).style(theme.dim).right_aligned());
        }
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
