//! The bottom status line.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::tui::keymap::mode::Mode;

/// One line at the bottom of the screen showing the current mode.
#[derive(Debug, Clone)]
pub struct StatusLine {
    mode: Mode,
}

impl StatusLine {
    /// Creates a status line for `mode`.
    pub fn new(mode: Mode) -> Self {
        Self { mode }
    }
}

impl Widget for StatusLine {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mode = Span::styled(
            format!(" {} ", self.mode.label()),
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
        );
        Line::from(vec![mode]).render(area, buf);
    }
}
