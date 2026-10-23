//! Key bindings per mode and scope.

use thiserror::Error;

use crate::app::command::{CommandId, Scope};
use crate::tui::keymap::mode::Mode;
use crate::tui::keymap::sequence::{Key, KeyParseError, KeySequence};

/// A key sequence bound to a command in one mode and scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The mode the binding is active in.
    pub mode: Mode,
    /// The scope the binding is active in.
    pub scope: Scope,
    /// The keys as written, which may contain `<leader>`.
    pub sequence: KeySequence,
    /// The keys with the leader resolved.
    pub keys: Vec<Key>,
    /// The command to run.
    pub command: CommandId,
}

/// An error raised while binding keys.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeymapError {
    /// The notation could not be parsed.
    #[error("cannot bind `{notation}`: {source}")]
    Parse {
        /// The notation as given.
        notation: String,
        /// The parse failure.
        source: KeyParseError,
    },
    /// The sequence equals another one, or one is a prefix of the other, in the same mode and
    /// scope.
    #[error("`{notation}` conflicts with `{existing}` in scope `{}`", scope.0)]
    Conflict {
        /// The notation as given.
        notation: String,
        /// The notation of the binding it conflicts with.
        existing: String,
        /// The shared scope.
        scope: Scope,
    },
}

/// All key bindings, with the leader key used to resolve `<leader>`.
#[derive(Debug, Clone)]
pub struct Keymap {
    leader: Key,
    bindings: Vec<Binding>,
    groups: Vec<PrefixGroup>,
}

/// A titled key prefix, such as `,` for sorting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixGroup {
    /// The mode the prefix applies in.
    pub mode: Mode,
    /// The scope the prefix applies in.
    pub scope: Scope,
    /// The prefix keys with the leader resolved.
    pub keys: Vec<Key>,
    /// The title of the which-key popup while the prefix is pending.
    pub title: &'static str,
    /// True when the popup appears at once instead of after the usual delay.
    pub instant: bool,
}

impl Keymap {
    /// Creates an empty keymap whose `<leader>` stands for `leader`.
    pub fn new(leader: Key) -> Self {
        Self {
            leader,
            bindings: Vec::new(),
            groups: Vec::new(),
        }
    }

    /// Gives the prefix `notation` in `mode` and `scope` a popup `title`, and makes the popup
    /// appear at once when `instant` is true.
    ///
    /// # Errors
    /// Returns [`KeymapError::Parse`] for invalid notation.
    pub fn describe_prefix(
        &mut self,
        mode: Mode,
        scope: Scope,
        notation: &str,
        title: &'static str,
        instant: bool,
    ) -> Result<(), KeymapError> {
        let sequence = KeySequence::parse(notation).map_err(|source| KeymapError::Parse {
            notation: notation.to_owned(),
            source,
        })?;
        self.groups.push(PrefixGroup {
            mode,
            scope,
            keys: sequence.resolve(self.leader),
            title,
            instant,
        });
        Ok(())
    }

    /// Returns the group described for exactly the keys `pending` in `mode`, taking the first
    /// of `scopes` that has one, or `None`.
    pub fn group_for(&self, mode: Mode, scopes: &[Scope], pending: &[Key]) -> Option<&PrefixGroup> {
        scopes.iter().find_map(|scope| {
            self.groups
                .iter()
                .find(|g| g.mode == mode && g.scope == *scope && g.keys == pending)
        })
    }

    /// Returns the leader key.
    pub fn leader(&self) -> Key {
        self.leader
    }

    /// Binds `notation` to `command` in `mode` and `scope`.
    ///
    /// # Errors
    /// Returns [`KeymapError::Parse`] for invalid notation, and [`KeymapError::Conflict`] when
    /// the sequence equals another, or one is a prefix of the other, in the same mode and scope.
    pub fn bind(
        &mut self,
        mode: Mode,
        scope: Scope,
        notation: &str,
        command: CommandId,
    ) -> Result<(), KeymapError> {
        let sequence = KeySequence::parse(notation).map_err(|source| KeymapError::Parse {
            notation: notation.to_owned(),
            source,
        })?;
        let keys = sequence.resolve(self.leader);
        if let Some(existing) = self.bindings.iter().find(|b| {
            b.mode == mode
                && b.scope == scope
                && (b.keys.starts_with(&keys) || keys.starts_with(&b.keys))
        }) {
            return Err(KeymapError::Conflict {
                notation: notation.to_owned(),
                existing: existing.sequence.to_string(),
                scope,
            });
        }
        self.bindings.push(Binding {
            mode,
            scope,
            sequence,
            keys,
            command,
        });
        Ok(())
    }

    /// Iterates over all bindings in registration order.
    pub fn bindings(&self) -> impl Iterator<Item = &Binding> {
        self.bindings.iter()
    }

    /// Iterates over the bindings of `command`.
    pub fn bindings_for(&self, command: CommandId) -> impl Iterator<Item = &Binding> {
        self.bindings.iter().filter(move |b| b.command == command)
    }
}
