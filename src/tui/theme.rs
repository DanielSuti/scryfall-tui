//! The styles the interface is drawn with.

use ratatui::style::{Color, Modifier, Style};

/// Every style the interface uses, each defined once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Border and title of the panel or popup that receives the keys.
    pub focus_border: Style,
    /// The row under the cursor in the focused list: bold on blue, lazygit's default for dark
    /// terminals.
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
    /// The text that `/` matches, as yazi's `find_keyword`.
    pub find_keyword: Style,
    /// The position of the current line among the matching lines, as yazi's `find_position`.
    pub find_position: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            focus_border: Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            selected_row: Style::new().bg(Color::Blue).add_modifier(Modifier::BOLD),
            key: Style::new().add_modifier(Modifier::BOLD),
            mode: Style::new().add_modifier(Modifier::BOLD.union(Modifier::REVERSED)),
            info: Style::new(),
            error: Style::new().fg(Color::Red),
            dim: Style::new().add_modifier(Modifier::DIM),
            find_keyword: Style::new().fg(Color::Yellow).add_modifier(
                Modifier::BOLD
                    .union(Modifier::ITALIC)
                    .union(Modifier::UNDERLINED),
            ),
            find_position: Style::new()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD.union(Modifier::ITALIC)),
        }
    }
}
