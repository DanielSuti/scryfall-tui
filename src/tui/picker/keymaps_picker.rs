//! The keymaps picker: every command with its keys.

use crate::app::command::{CommandId, CommandRegistry};
use crate::tui::keymap::bindings::Keymap;
use crate::tui::picker::picker::Picker;

/// Builds the picker items: one per registered command, in registration order, labelled with
/// all of the command's key bindings and its description.
pub fn keymap_items<C>(commands: &CommandRegistry<C>, keymap: &Keymap) -> Vec<(String, CommandId)> {
    let keys: Vec<(String, CommandId, &str)> = commands
        .iter()
        .map(|command| {
            let mut notations: Vec<String> = keymap
                .bindings_for(command.id)
                .map(|b| b.sequence.to_string())
                .collect();
            notations.dedup();
            (
                notations.join(" "),
                command.id,
                command.description.as_ref(),
            )
        })
        .collect();
    let width = keys
        .iter()
        .map(|(k, _, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    keys.into_iter()
        .map(|(k, id, description)| (format!("{k:width$}  {description}"), id))
        .collect()
}

/// Creates the keymaps picker.
pub fn keymaps_picker<C>(commands: &CommandRegistry<C>, keymap: &Keymap) -> Picker<CommandId> {
    Picker::new("Keymaps", keymap_items(commands, keymap))
}
