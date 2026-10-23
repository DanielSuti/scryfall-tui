//! The search bar at the top of the screen.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

use crate::tui::layout::panel_block;
use crate::tui::text::truncate;
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
    /// when `editing`. Non-empty `base` terms, which every search adds, are shown dimmed on the
    /// right of the top border as `+ base`, cut with `...` to at most half the width.
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: String,
        base: &str,
        focused: bool,
        editing: bool,
    ) {
        let mut block = panel_block(title, focused);
        let base = base.trim();
        if !base.is_empty() {
            let room = usize::from(area.width / 2).saturating_sub(5);
            let note = format!(" + {} ", truncate(base, room));
            block = block.title_top(
                Line::from(note)
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .right_aligned(),
            );
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
