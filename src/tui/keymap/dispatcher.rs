//! Turns key presses into commands, one key at a time.

use crate::app::command::{CommandId, Scope};
use crate::tui::keymap::bindings::{Binding, Keymap};
use crate::tui::keymap::mode::Mode;
use crate::tui::keymap::sequence::{Key, KeySequence};

/// A key sequence that completes the keys pressed so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Continuation {
    /// The keys still to press.
    pub rest: KeySequence,
    /// The command they run.
    pub command: CommandId,
}

/// The outcome of feeding one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispatch {
    /// The keys pressed so far form a binding.
    Matched(CommandId),
    /// The keys pressed so far start one or more bindings.
    Pending(Vec<Continuation>),
    /// No binding starts with the keys pressed so far. The pending keys are discarded.
    NoMatch(Key),
}

/// Collects keys until they form a binding.
#[derive(Debug, Clone, Default)]
pub struct Dispatcher {
    pending: Vec<Key>,
}

impl Dispatcher {
    /// Creates a dispatcher with no pending keys.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the keys pressed so far without completing a binding.
    pub fn pending(&self) -> &[Key] {
        &self.pending
    }

    /// Discards the pending keys.
    pub fn reset(&mut self) {
        self.pending.clear();
    }

    /// Feeds `key` and looks the pending keys up in `keymap`.
    ///
    /// `scopes` are the active scopes in precedence order, such as the focused panel and then
    /// [`Scope::GLOBAL`]. The first scope with a binding that equals or extends the pending keys
    /// decides the outcome. A pending outcome lists the continuations of every active scope,
    /// except those shadowed by a binding of a higher-precedence scope.
    pub fn feed(&mut self, keymap: &Keymap, mode: Mode, scopes: &[Scope], key: Key) -> Dispatch {
        self.pending.push(key);
        let in_scope = |scope: Scope| {
            keymap
                .bindings()
                .filter(move |b| b.mode == mode && b.scope == scope)
        };
        for scope in scopes {
            let mut extends = in_scope(*scope)
                .filter(|b| b.keys.starts_with(&self.pending))
                .peekable();
            if extends.peek().is_none() {
                continue;
            }
            if let Some(exact) = in_scope(*scope).find(|b| b.keys == self.pending) {
                self.pending.clear();
                return Dispatch::Matched(exact.command);
            }
            return Dispatch::Pending(self.continuations(keymap, mode, scopes));
        }
        self.pending.clear();
        Dispatch::NoMatch(key)
    }

    fn continuations(&self, keymap: &Keymap, mode: Mode, scopes: &[Scope]) -> Vec<Continuation> {
        let active: Vec<(usize, &Binding)> = keymap
            .bindings()
            .filter(|b| b.mode == mode)
            .filter_map(|b| Some((scopes.iter().position(|s| *s == b.scope)?, b)))
            .collect();
        let shadowed = |rank: usize, binding: &Binding| {
            active.iter().any(|(other_rank, other)| {
                *other_rank < rank && binding.keys.starts_with(&other.keys)
            })
        };
        active
            .iter()
            .filter(|(_, b)| b.keys.len() > self.pending.len() && b.keys.starts_with(&self.pending))
            .filter(|(rank, b)| !shadowed(*rank, b))
            .filter_map(|(_, b)| {
                Some(Continuation {
                    rest: b.sequence.tail(self.pending.len())?,
                    command: b.command,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::keymap::bindings::KeymapError;

    const RESULTS: Scope = Scope("results");
    const SPACE: Key = Key::char(' ');

    fn keymap() -> Keymap {
        let mut keymap = Keymap::new(SPACE);
        keymap
            .bind(
                Mode::Normal,
                Scope::GLOBAL,
                "<leader>ss",
                CommandId("search.new"),
            )
            .unwrap();
        keymap
            .bind(
                Mode::Normal,
                Scope::GLOBAL,
                "<leader>sk",
                CommandId("keymaps"),
            )
            .unwrap();
        keymap
            .bind(Mode::Normal, Scope::GLOBAL, "q", CommandId("global.q"))
            .unwrap();
        keymap
            .bind(Mode::Normal, RESULTS, "q", CommandId("results.back"))
            .unwrap();
        keymap
            .bind(Mode::Normal, RESULTS, "gg", CommandId("top"))
            .unwrap();
        keymap
    }

    fn feed_all(dispatcher: &mut Dispatcher, keymap: &Keymap, keys: &str) -> Dispatch {
        let scopes = [RESULTS, Scope::GLOBAL];
        let mut last = None;
        for key in KeySequence::parse(keys).unwrap().resolve(SPACE) {
            last = Some(dispatcher.feed(keymap, Mode::Normal, &scopes, key));
        }
        last.unwrap()
    }

    #[test]
    fn pending_lists_every_continuation() {
        let keymap = keymap();
        let mut dispatcher = Dispatcher::new();
        let Dispatch::Pending(continuations) = feed_all(&mut dispatcher, &keymap, "<leader>")
        else {
            panic!("expected pending");
        };
        let listed: Vec<String> = continuations
            .iter()
            .map(|c| format!("{} {}", c.rest, c.command))
            .collect();
        assert_eq!(listed, ["ss search.new", "sk keymaps"]);
        assert_eq!(
            feed_all(&mut dispatcher, &keymap, "sk"),
            Dispatch::Matched(CommandId("keymaps"))
        );
        assert!(dispatcher.pending().is_empty());
    }

    #[test]
    fn view_binding_beats_global_binding() {
        let keymap = keymap();
        let mut dispatcher = Dispatcher::new();
        assert_eq!(
            feed_all(&mut dispatcher, &keymap, "q"),
            Dispatch::Matched(CommandId("results.back"))
        );
        let global_only = [Scope::GLOBAL];
        assert_eq!(
            dispatcher.feed(&keymap, Mode::Normal, &global_only, Key::char('q')),
            Dispatch::Matched(CommandId("global.q"))
        );
    }

    #[test]
    fn prefix_conflict_in_the_same_scope_is_rejected() {
        let mut keymap = keymap();
        let err = keymap
            .bind(Mode::Normal, Scope::GLOBAL, "<leader>s", CommandId("x"))
            .unwrap_err();
        assert!(matches!(err, KeymapError::Conflict { .. }), "{err:?}");
        let err = keymap
            .bind(Mode::Normal, RESULTS, "ggg", CommandId("x"))
            .unwrap_err();
        assert!(matches!(err, KeymapError::Conflict { .. }), "{err:?}");
        // The same keys in another scope or mode are allowed.
        keymap
            .bind(Mode::Insert, Scope::GLOBAL, "<leader>s", CommandId("x"))
            .unwrap();
    }

    #[test]
    fn unknown_key_resets_the_sequence() {
        let keymap = keymap();
        let mut dispatcher = Dispatcher::new();
        assert!(matches!(
            feed_all(&mut dispatcher, &keymap, "<leader>"),
            Dispatch::Pending(_)
        ));
        assert_eq!(
            feed_all(&mut dispatcher, &keymap, "x"),
            Dispatch::NoMatch(Key::char('x'))
        );
        assert!(dispatcher.pending().is_empty());
        assert_eq!(
            feed_all(&mut dispatcher, &keymap, "gg"),
            Dispatch::Matched(CommandId("top"))
        );
    }

    #[test]
    fn shadowed_global_continuations_are_not_listed() {
        let mut keymap = keymap();
        keymap
            .bind(Mode::Normal, Scope::GLOBAL, "gx", CommandId("global.gx"))
            .unwrap();
        keymap
            .bind(Mode::Normal, Scope::GLOBAL, "g", CommandId("global.g"))
            .unwrap_err();
        let mut dispatcher = Dispatcher::new();
        let Dispatch::Pending(continuations) = feed_all(&mut dispatcher, &keymap, "g") else {
            panic!("expected pending");
        };
        let commands: Vec<CommandId> = continuations.iter().map(|c| c.command).collect();
        assert_eq!(commands, [CommandId("top"), CommandId("global.gx")]);
    }
}
