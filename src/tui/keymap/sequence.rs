//! Key notation such as `<leader>ss`, `gg` or `<C-d>`, and normalised keys.

use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use thiserror::Error;

/// One normalised key press. The case of a character carries Shift, so `Q` and Shift+q are the
/// same key, and Shift+Tab is [`KeyCode::BackTab`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    code: KeyCode,
    ctrl: bool,
    alt: bool,
}

/// Named keys accepted inside `<…>`, with their canonical spelling.
const NAMED_KEYS: &[(&str, KeyCode)] = &[
    ("CR", KeyCode::Enter),
    ("Esc", KeyCode::Esc),
    ("Tab", KeyCode::Tab),
    ("S-Tab", KeyCode::BackTab),
    ("BS", KeyCode::Backspace),
    ("Space", KeyCode::Char(' ')),
    ("lt", KeyCode::Char('<')),
    ("Up", KeyCode::Up),
    ("Down", KeyCode::Down),
    ("Left", KeyCode::Left),
    ("Right", KeyCode::Right),
    ("Home", KeyCode::Home),
    ("End", KeyCode::End),
    ("PageUp", KeyCode::PageUp),
    ("PageDown", KeyCode::PageDown),
];

impl Key {
    /// A key with no modifier.
    pub const fn plain(code: KeyCode) -> Self {
        Self {
            code,
            ctrl: false,
            alt: false,
        }
    }

    /// The character key `c`, without modifiers.
    pub const fn char(c: char) -> Self {
        Self::plain(KeyCode::Char(c))
    }

    /// Ctrl together with the character `c`.
    pub const fn ctrl(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            ctrl: true,
            alt: false,
        }
    }

    /// Returns the key code.
    pub fn code(self) -> KeyCode {
        self.code
    }

    /// Normalises a terminal key event. Returns `None` for key releases, which never trigger
    /// anything.
    pub fn from_event(event: &KeyEvent) -> Option<Self> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        let shift = event.modifiers.contains(KeyModifiers::SHIFT);
        let code = match event.code {
            KeyCode::Tab if shift => KeyCode::BackTab,
            code => code,
        };
        Some(Self {
            code,
            ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
            alt: event.modifiers.contains(KeyModifiers::ALT),
        })
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.alt {
            return write!(f, "<A-{:?}>", self.code);
        }
        match (self.code, self.ctrl) {
            (KeyCode::Char(c), true) => write!(f, "<C-{c}>"),
            (code, false) => match NAMED_KEYS.iter().find(|(_, named)| *named == code) {
                Some((name, _)) => write!(f, "<{name}>"),
                None => match code {
                    KeyCode::Char(c) => write!(f, "{c}"),
                    other => write!(f, "<{other:?}>"),
                },
            },
            (other, true) => write!(f, "<C-{other:?}>"),
        }
    }
}

/// One element of a key sequence: the leader key, or a concrete key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyToken {
    /// Stands for the configured leader key.
    Leader,
    /// A concrete key.
    Key(Key),
}

/// A sequence of keys written in vim-like notation, such as `<leader>ss`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySequence(Vec<KeyToken>);

/// Why a key notation could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyParseError {
    /// The notation is empty.
    #[error("empty key sequence")]
    Empty,
    /// A `<` is not closed by `>`.
    #[error("unterminated `<` in key sequence `{0}`")]
    Unterminated(String),
    /// `<>` contains no name.
    #[error("empty `<>` in key sequence")]
    EmptyTag,
    /// The name inside `<…>` is not a known key.
    #[error("unknown key `<{0}>`")]
    UnknownTag(String),
    /// A space must be written `<Space>`.
    #[error("a literal space must be written `<Space>`")]
    LiteralSpace,
}

impl KeySequence {
    /// Parses vim-like notation. Names inside `<…>` are case-insensitive, except for the
    /// character after `C-`.
    ///
    /// # Errors
    /// Returns a [`KeyParseError`] for empty input, an unterminated or empty `<…>`, an unknown
    /// key name, or a literal space.
    pub fn parse(notation: &str) -> Result<Self, KeyParseError> {
        let mut tokens = Vec::new();
        let mut rest = notation;
        while let Some(ch) = rest.chars().next() {
            match ch {
                '<' => {
                    let end = rest
                        .find('>')
                        .ok_or_else(|| KeyParseError::Unterminated(notation.to_owned()))?;
                    tokens.push(parse_tag(&rest[1..end])?);
                    rest = &rest[end + 1..];
                }
                ' ' => return Err(KeyParseError::LiteralSpace),
                c => {
                    tokens.push(KeyToken::Key(Key::char(c)));
                    rest = &rest[c.len_utf8()..];
                }
            }
        }
        if tokens.is_empty() {
            return Err(KeyParseError::Empty);
        }
        Ok(Self(tokens))
    }

    /// Returns the tokens in order.
    pub fn tokens(&self) -> &[KeyToken] {
        &self.0
    }

    /// Returns the concrete keys, with every [`KeyToken::Leader`] replaced by `leader`.
    pub fn resolve(&self, leader: Key) -> Vec<Key> {
        self.0
            .iter()
            .map(|token| match token {
                KeyToken::Leader => leader,
                KeyToken::Key(key) => *key,
            })
            .collect()
    }
}

fn parse_tag(name: &str) -> Result<KeyToken, KeyParseError> {
    if name.is_empty() {
        return Err(KeyParseError::EmptyTag);
    }
    if name.eq_ignore_ascii_case("leader") {
        return Ok(KeyToken::Leader);
    }
    if let Some(rest) = name.strip_prefix("C-").or_else(|| name.strip_prefix("c-")) {
        let mut chars = rest.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Ok(KeyToken::Key(Key::ctrl(c)));
        }
    }
    NAMED_KEYS
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(name))
        .map(|(_, code)| KeyToken::Key(Key::plain(*code)))
        .ok_or_else(|| KeyParseError::UnknownTag(name.to_owned()))
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for token in &self.0 {
            match token {
                KeyToken::Leader => f.write_str("<leader>")?,
                KeyToken::Key(key) => write!(f, "{key}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn notation_round_trips() {
        for notation in [
            "<leader>ss",
            "gg",
            "G",
            "<C-d>",
            "<CR>",
            "<Esc>",
            "<S-Tab>",
            "<Space>",
            "<lt>",
            "dd",
            "/",
        ] {
            let parsed = KeySequence::parse(notation).unwrap();
            assert_eq!(parsed.to_string(), notation);
            assert_eq!(KeySequence::parse(&parsed.to_string()).unwrap(), parsed);
        }
    }

    #[test]
    fn names_are_case_insensitive() {
        assert_eq!(
            KeySequence::parse("<cr><LEADER>").unwrap(),
            KeySequence::parse("<CR><leader>").unwrap()
        );
    }

    #[test]
    fn invalid_notation_is_rejected() {
        assert_eq!(KeySequence::parse(""), Err(KeyParseError::Empty));
        assert_eq!(KeySequence::parse("<>"), Err(KeyParseError::EmptyTag));
        assert_eq!(
            KeySequence::parse("<leader"),
            Err(KeyParseError::Unterminated("<leader".into()))
        );
        assert_eq!(
            KeySequence::parse("<Foo>"),
            Err(KeyParseError::UnknownTag("Foo".into()))
        );
        assert_eq!(KeySequence::parse("a b"), Err(KeyParseError::LiteralSpace));
    }

    #[test]
    fn ctrl_d_matches_only_ctrl_d() {
        let ctrl_d = KeySequence::parse("<C-d>").unwrap().resolve(Key::char(' '));
        let pressed = |code, modifiers| Key::from_event(&event(code, modifiers)).unwrap();
        assert_eq!(
            ctrl_d,
            vec![pressed(KeyCode::Char('d'), KeyModifiers::CONTROL)]
        );
        assert_ne!(
            ctrl_d,
            vec![pressed(KeyCode::Char('d'), KeyModifiers::NONE)]
        );
        assert_ne!(
            ctrl_d,
            vec![pressed(
                KeyCode::Char('d'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )]
        );
    }

    #[test]
    fn shift_is_carried_by_the_character() {
        let typed = Key::from_event(&event(KeyCode::Char('Q'), KeyModifiers::SHIFT)).unwrap();
        assert_eq!(typed, Key::char('Q'));
        let back_tab = Key::from_event(&event(KeyCode::Tab, KeyModifiers::SHIFT)).unwrap();
        assert_eq!(back_tab, Key::plain(KeyCode::BackTab));
    }

    #[test]
    fn leader_resolves_to_the_given_key() {
        let keys = KeySequence::parse("<leader>s")
            .unwrap()
            .resolve(Key::char(' '));
        assert_eq!(keys, vec![Key::char(' '), Key::char('s')]);
    }

    #[test]
    fn releases_are_ignored() {
        let mut release = event(KeyCode::Char('j'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(Key::from_event(&release), None);
    }
}
