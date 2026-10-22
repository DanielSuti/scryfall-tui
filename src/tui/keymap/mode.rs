//! Editing modes.

/// The modal state that decides how keys are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Keys run commands.
    Normal,
    /// Keys are typed into the focused text input.
    Insert,
}

impl Mode {
    /// Returns the label shown in the status line, such as `NORMAL`.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
        }
    }
}
