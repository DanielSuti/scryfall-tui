//! The history panel's commands and saving the history.

use std::path::PathBuf;

use crate::app::command::CommandId;
use crate::app::history::History;
use crate::tui::app::App;
use crate::tui::app::registrar::{Registrar, RegistrationError, command};
use crate::tui::keymap::mode::Mode;
use crate::tui::layout::{HISTORY_SCOPE, Panel};
use crate::tui::widgets::status_line::StatusMessage;

/// Runs the history entry under the cursor.
pub const RUN_ENTRY: CommandId = CommandId("history.run");
/// Copies the history entry under the cursor to the search bar for editing.
pub const EDIT_ENTRY: CommandId = CommandId("history.edit");
/// Deletes the history entry under the cursor.
pub const DELETE_ENTRY: CommandId = CommandId("history.delete");

pub(super) fn register(reg: &mut Registrar<'_>) -> Result<(), RegistrationError> {
    let normal = Mode::Normal;
    reg.add(
        normal,
        &["<CR>"],
        command(
            RUN_ENTRY,
            HISTORY_SCOPE,
            "Run the search again",
            Some("run"),
            |app| {
                let Some(query) = app.history_view.selected().map(|e| e.query.clone()) else {
                    return;
                };
                app.search_bar.set_query(&query);
                app.set_focus(Panel::Main);
                app.record_search(&query);
                app.run_query(&query);
            },
        ),
    )?;
    reg.add(
        normal,
        &["e"],
        command(
            EDIT_ENTRY,
            HISTORY_SCOPE,
            "Edit the search in the search bar",
            Some("edit"),
            |app| {
                let Some(query) = app.history_view.selected().map(|e| e.query.clone()) else {
                    return;
                };
                app.search_bar.set_query(&query);
                app.set_focus(Panel::SearchBar);
                app.enter_insert_mode();
            },
        ),
    )?;
    reg.add(
        normal,
        &["dd"],
        command(
            DELETE_ENTRY,
            HISTORY_SCOPE,
            "Delete the entry",
            Some("delete"),
            |app| {
                let index = app.history_view.cursor();
                if app.history.delete(index).is_some() {
                    app.history_changed();
                }
            },
        ),
    )?;
    Ok(())
}

impl App {
    /// Replaces the history, which is saved to `path` after every change, or never when
    /// `path` is `None`.
    pub fn set_history(&mut self, history: History, path: Option<PathBuf>) {
        self.history = history;
        self.history_path = path;
        self.history_view.show(&self.history);
    }

    /// Returns the history.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Returns the index of the history entry under the cursor.
    pub fn history_cursor(&self) -> usize {
        self.history_view.cursor()
    }

    /// Puts `query` at the top of the history, with its count when its results are cached,
    /// and moves the history cursor onto it.
    pub(super) fn record_search(&mut self, query: &str) {
        self.history.record(query);
        let request = self.request_for(query);
        if let Some(total) = self.cache.get(&request).and_then(|r| r.total_cards()) {
            self.history.set_total(query, Some(total));
        }
        self.history_changed();
        self.history_view.select_first();
    }

    /// Shows the history and saves it.
    pub(super) fn history_changed(&mut self) {
        self.history_view.show(&self.history);
        if let Some(path) = &self.history_path
            && let Err(err) = self.history.save(path)
        {
            self.message = Some(StatusMessage::error(err.to_string()));
        }
    }
}
