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
    /// The position of the current line among the matching lines, such as `[2/9]`: a badge
    /// with a magenta background and text in the terminal's background color.
    pub find_position: Style,
    /// Panel text while flash is open, as flash.nvim's `FlashBackdrop`.
    pub flash_backdrop: Style,
    /// Text that flash matches, as flash.nvim's `FlashMatch`.
    pub flash_match: Style,
    /// The match nearest the cursor, which `<Enter>` jumps to, as flash.nvim's `FlashCurrent`.
    pub flash_current: Style,
    /// A flash jump label, as flash.nvim's `FlashLabel`.
    pub flash_label: Style,
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
            // Reversed, so the text takes the terminal's background color on any theme.
            find_position: Style::new()
                .fg(Color::Magenta)
                .bg(Color::Reset)
                .add_modifier(Modifier::BOLD.union(Modifier::REVERSED)),
            flash_backdrop: Style::new().fg(Color::DarkGray),
            flash_match: Style::new().fg(Color::White).bg(Color::Blue),
            // 256-color orange, the closest to flash.nvim's FlashCurrent.
            flash_current: Style::new().fg(Color::Black).bg(Color::Indexed(208)),
            flash_label: Style::new()
                .fg(Color::White)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        }
    }
}
