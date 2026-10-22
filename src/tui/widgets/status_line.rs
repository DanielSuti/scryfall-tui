//! The bottom status line: mode, context, messages and key hints.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::tui::keymap::mode::Mode;
use crate::tui::text::{display_width, truncate};

/// How a status message is styled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Neutral information.
    Info,
    /// Something failed.
    Error,
}

/// A message shown after the status sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessage {
    /// The text, shown verbatim.
    pub text: String,
    /// How the text is styled.
    pub severity: Severity,
}

impl StatusMessage {
    /// An informational message.
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: Severity::Info,
        }
    }

    /// An error message.
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: Severity::Error,
        }
    }
}

/// One line at the bottom of the screen. The mode, the sections and the message sit on the
/// left. The key hints sit on the right and are cut with `...` when they don't fit.
#[derive(Debug, Clone)]
pub struct StatusLine<'a> {
    mode: Mode,
    sections: Vec<String>,
    message: Option<&'a StatusMessage>,
    hints: &'a [String],
}

impl<'a> StatusLine<'a> {
    /// Creates a status line for `mode` with no sections, message or hints.
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            sections: Vec::new(),
            message: None,
            hints: &[],
        }
    }

    /// Adds a section, such as the focused panel or the loaded count.
    #[must_use]
    pub fn section(mut self, text: impl Into<String>) -> Self {
        self.sections.push(text.into());
        self
    }

    /// Sets the message.
    #[must_use]
    pub fn message(mut self, message: Option<&'a StatusMessage>) -> Self {
        self.message = message;
        self
    }

    /// Sets the key hints, each already formatted such as `d detail`.
    #[must_use]
    pub fn hints(mut self, hints: &'a [String]) -> Self {
        self.hints = hints;
        self
    }
}

impl Widget for StatusLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut left = vec![Span::styled(
            format!(" {} ", self.mode.label()),
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
        )];
        for section in &self.sections {
            left.push(Span::raw(format!(" │ {section}")));
        }
        if let Some(message) = self.message {
            let style = match message.severity {
                Severity::Info => Style::default(),
                Severity::Error => Style::default().fg(Color::Red),
            };
            left.push(Span::styled(format!("  {}", message.text), style));
        }
        let left = Line::from(left);
        let left_width = left.width();
        left.render(area, buf);

        let width = usize::from(area.width);
        let available = width.saturating_sub(left_width + 2);
        if self.hints.is_empty() || available == 0 {
            return;
        }
        let hints = truncate(&self.hints.join(" · "), available);
        let hints_width = display_width(&hints);
        let x = area.x + u16::try_from(width - hints_width - 1).unwrap_or(0);
        Line::from(hints).render(
            Rect {
                x,
                width: u16::try_from(hints_width).unwrap_or(u16::MAX),
                ..area
            },
            buf,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(line: StatusLine<'_>, width: u16) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        line.render(area, &mut buf);
        buf.content().iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn layout_puts_hints_on_the_right() {
        let hints = vec!["d detail".to_owned(), "q quit".to_owned()];
        let text = render(
            StatusLine::new(Mode::Normal)
                .section("[3] Results")
                .hints(&hints),
            50,
        );
        let left = " NORMAL  │ [3] Results";
        let right = "d detail · q quit ";
        let gap = 50 - display_width(left) - display_width(right);
        assert_eq!(text, format!("{left}{}{right}", " ".repeat(gap)));
    }

    #[test]
    fn hints_that_do_not_fit_are_cut() {
        let hints = vec!["d detail".to_owned(), "q quit".to_owned()];
        let text = render(StatusLine::new(Mode::Normal).hints(&hints), 20);
        // 20 columns minus the mode and a two-column gap leaves 10 for the hints.
        assert_eq!(
            text,
            format!(" NORMAL  {} ", truncate("d detail · q quit", 10))
        );
        assert_eq!(truncate("d detail · q quit", 10), "d detai...");
    }
}
