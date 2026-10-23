//! The styles the interface is drawn with.
//!
//! Every color is a slot of the terminal's 16-color palette, or the terminal's own default
//! foreground and background, so the interface follows the terminal's color theme. No style
//! names a fixed color: where text sits on a colored background, it keeps the default
//! foreground, or is reversed so it takes the terminal's background color.
//!
//! The flash styles follow LazyVim's flash.nvim groups and the tokyonight colors LazyVim gives
//! them, each mapped to the palette slot of the same hue: `FlashBackdrop` (`Comment`, grey) is
//! bright black, `FlashMatch` (`Search`) blue, `FlashCurrent` (`IncSearch`, orange) yellow, and
//! `FlashLabel` magenta.

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
    /// Panel text while flash is open, as flash.nvim's `FlashBackdrop`: bright black, the
    /// palette's grey.
    pub flash_backdrop: Style,
    /// Text that flash matches, as flash.nvim's `FlashMatch`: the default foreground on blue.
    pub flash_match: Style,
    /// The match nearest the cursor, which `<Enter>` jumps to, as flash.nvim's `FlashCurrent`:
    /// yellow, reversed, so the text takes the terminal's background color.
    pub flash_current: Style,
    /// A flash jump label, as flash.nvim's `FlashLabel`: the default foreground on magenta, bold.
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
            flash_match: Style::new().fg(Color::Reset).bg(Color::Blue),
            flash_current: Style::new()
                .fg(Color::Yellow)
                .bg(Color::Reset)
                .add_modifier(Modifier::REVERSED),
            flash_label: Style::new()
                .fg(Color::Reset)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 16 palette slots a terminal theme defines, and the terminal's defaults.
    fn follows_the_terminal_theme(color: Option<Color>) -> bool {
        matches!(
            color,
            None | Some(
                Color::Reset
                    | Color::Black
                    | Color::Red
                    | Color::Green
                    | Color::Yellow
                    | Color::Blue
                    | Color::Magenta
                    | Color::Cyan
                    | Color::Gray
                    | Color::DarkGray
                    | Color::LightRed
                    | Color::LightGreen
                    | Color::LightYellow
                    | Color::LightBlue
                    | Color::LightMagenta
                    | Color::LightCyan
                    | Color::White
            )
        )
    }

    #[test]
    fn every_style_uses_the_terminal_palette_and_no_forced_text_color() {
        let t = Theme::default();
        let styles = [
            ("focus_border", t.focus_border),
            ("selected_row", t.selected_row),
            ("key", t.key),
            ("mode", t.mode),
            ("info", t.info),
            ("error", t.error),
            ("dim", t.dim),
            ("find_keyword", t.find_keyword),
            ("find_position", t.find_position),
            ("flash_backdrop", t.flash_backdrop),
            ("flash_match", t.flash_match),
            ("flash_current", t.flash_current),
            ("flash_label", t.flash_label),
        ];
        for (name, style) in styles {
            assert!(follows_the_terminal_theme(style.fg), "{name}: {style:?}");
            assert!(follows_the_terminal_theme(style.bg), "{name}: {style:?}");
            // Black or white text would ignore the theme's own foreground and background.
            for fixed in [Color::Black, Color::White] {
                assert_ne!(style.fg, Some(fixed), "{name}: {style:?}");
            }
        }
    }
}
