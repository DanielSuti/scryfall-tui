//! The styles the interface is drawn with.

use ratatui::style::{Color, Modifier, Style};

/// Every style the interface uses, each defined once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Border and title of the panel or popup that receives the keys.
    pub focus_border: Style,
    /// The row under the cursor in the focused list.
    pub selected_row: Style,
    /// Keys in the which-key popup.
    pub key: Style,
    /// The mode label in the status line.
    pub mode: Style,
    /// An informational status message.
    pub info: Style,
    /// An error status message.
    pub error: Style,
    /// Secondary text, such as the base terms of the search bar.
    pub dim: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            focus_border: Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            selected_row: Style::new().add_modifier(Modifier::BOLD),
            key: Style::new().add_modifier(Modifier::BOLD),
            mode: Style::new().add_modifier(Modifier::BOLD.union(Modifier::REVERSED)),
            info: Style::new(),
            error: Style::new().fg(Color::Red),
            dim: Style::new().add_modifier(Modifier::DIM),
        }
    }
}
