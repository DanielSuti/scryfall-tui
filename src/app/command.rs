//! Named commands that keys can be bound to.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use thiserror::Error;

/// The unique identifier of a command, such as `search.new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(pub &'static str);

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Where a command or key binding applies: everywhere, or in one panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Scope(pub &'static str);

impl Scope {
    /// Applies regardless of which panel has focus.
    pub const GLOBAL: Scope = Scope("global");
}

/// A command handler. It may capture values, such as the sort order a command applies.
pub type Handler<C> = Arc<dyn Fn(&mut C) + Send + Sync>;

/// A command that runs against a context `C`, such as the application state.
pub struct Command<C> {
    /// Unique identifier.
    pub id: CommandId,
    /// One-line description shown in pickers and which-key.
    pub description: Cow<'static, str>,
    /// The scope the command belongs to.
    pub scope: Scope,
    /// Short label for the status-line hints, or `None` to leave the command out of them.
    pub hint: Option<&'static str>,
    /// The handler.
    pub run: Handler<C>,
}

impl<C> Command<C> {
    /// Creates a command running `run`.
    pub fn new(
        id: CommandId,
        description: impl Into<Cow<'static, str>>,
        scope: Scope,
        hint: Option<&'static str>,
        run: impl Fn(&mut C) + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            description: description.into(),
            scope,
            hint,
            run: Arc::new(run),
        }
    }
}

impl<C> Clone for Command<C> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            description: self.description.clone(),
            scope: self.scope,
            hint: self.hint,
            run: Arc::clone(&self.run),
        }
    }
}

impl<C> fmt::Debug for Command<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("id", &self.id)
            .field("description", &self.description)
            .field("scope", &self.scope)
            .field("hint", &self.hint)
            .finish_non_exhaustive()
    }
}

/// An error raised while registering commands.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandError {
    /// A command with this id is already registered.
    #[error("command `{0}` is registered twice")]
    Duplicate(CommandId),
}

/// All commands known to the application, in registration order.
pub struct CommandRegistry<C> {
    commands: Vec<Command<C>>,
}

impl<C> Default for CommandRegistry<C> {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
}

impl<C> fmt::Debug for CommandRegistry<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.commands).finish()
    }
}

impl<C> CommandRegistry<C> {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `command`.
    ///
    /// # Errors
    /// Returns [`CommandError::Duplicate`] when a command with the same id exists.
    pub fn register(&mut self, command: Command<C>) -> Result<(), CommandError> {
        if self.get(command.id).is_some() {
            return Err(CommandError::Duplicate(command.id));
        }
        self.commands.push(command);
        Ok(())
    }

    /// Returns the command with `id`, or `None` when none is registered.
    pub fn get(&self, id: CommandId) -> Option<&Command<C>> {
        self.commands.iter().find(|command| command.id == id)
    }

    /// Iterates over all commands in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &Command<C>> {
        self.commands.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn increment(counter: &mut u32) {
        *counter += 1;
    }

    fn command(id: &'static str) -> Command<u32> {
        Command::new(CommandId(id), "Increment", Scope::GLOBAL, None, increment)
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let mut registry = CommandRegistry::new();
        registry.register(command("count")).unwrap();
        assert_eq!(
            registry.register(command("count")),
            Err(CommandError::Duplicate(CommandId("count")))
        );
        assert_eq!(registry.iter().count(), 1);
    }

    #[test]
    fn lookup_returns_the_registered_handler() {
        let mut registry = CommandRegistry::new();
        registry.register(command("a")).unwrap();
        registry.register(command("b")).unwrap();
        let mut counter = 0;
        (registry.get(CommandId("b")).unwrap().run)(&mut counter);
        assert_eq!(counter, 1);
        assert!(registry.get(CommandId("c")).is_none());
    }

    #[test]
    fn handlers_can_capture_values() {
        let mut registry = CommandRegistry::new();
        for step in [2, 5] {
            let id = if step == 2 { "by_two" } else { "by_five" };
            let description = format!("Add {step}");
            registry
                .register(Command::new(
                    CommandId(id),
                    description,
                    Scope::GLOBAL,
                    None,
                    move |n: &mut u32| *n += step,
                ))
                .unwrap();
        }
        let mut counter = 0;
        (registry.get(CommandId("by_five")).unwrap().run)(&mut counter);
        (registry.get(CommandId("by_two")).unwrap().run)(&mut counter);
        assert_eq!(counter, 7);
        assert_eq!(
            registry.get(CommandId("by_two")).unwrap().description,
            "Add 2"
        );
    }
}
