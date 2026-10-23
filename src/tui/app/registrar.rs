//! Registration of commands together with their key bindings.

use crate::app::command::{Command, CommandId, CommandRegistry, Scope};
use crate::tui::app::App;
use crate::tui::keymap::bindings::Keymap;
use crate::tui::keymap::mode::Mode;

/// An error raised while registering built-in commands and keys.
#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    /// A command id is registered twice.
    #[error(transparent)]
    Command(#[from] crate::app::command::CommandError),
    /// A key binding is invalid or conflicts with another.
    #[error(transparent)]
    Keymap(#[from] crate::tui::keymap::bindings::KeymapError),
}

/// Registers commands and binds their keys in one step.
pub struct Registrar<'a> {
    pub(super) commands: &'a mut CommandRegistry<App>,
    pub(super) keymap: &'a mut Keymap,
}

impl Registrar<'_> {
    /// Registers a command and binds each of `keys` to it in `mode`, in the command's scope.
    ///
    /// # Errors
    /// Returns an error when the id is already registered or a key binding is invalid or
    /// conflicts with another.
    pub fn add(
        &mut self,
        mode: Mode,
        keys: &[&str],
        command: Command<App>,
    ) -> Result<(), RegistrationError> {
        let (id, scope) = (command.id, command.scope);
        self.commands.register(command)?;
        self.bind(mode, scope, keys, id)
    }

    /// Binds each of `keys` to the registered command `id` in `mode` and `scope`.
    ///
    /// # Errors
    /// Returns an error when a key binding is invalid or conflicts with another.
    pub fn bind(
        &mut self,
        mode: Mode,
        scope: Scope,
        keys: &[&str],
        id: CommandId,
    ) -> Result<(), RegistrationError> {
        for notation in keys {
            self.keymap.bind(mode, scope, notation, id)?;
        }
        Ok(())
    }
}

/// Builds a command for the application.
pub fn command(
    id: CommandId,
    scope: Scope,
    description: &'static str,
    hint: Option<&'static str>,
    run: fn(&mut App),
) -> Command<App> {
    Command::new(id, description, scope, hint, run)
}
