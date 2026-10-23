//! Built-in commands: quitting, modes, focus, the main panel, motions, `/` search and the
//! keymaps picker.

use crate::app::command::{CommandId, CommandRegistry, Scope};
use crate::tui::app::App;
use crate::tui::app::registrar::{Registrar, RegistrationError, command};
use crate::tui::keymap::bindings::Keymap;
use crate::tui::keymap::mode::Mode;
use crate::tui::layout::{
    DETAIL_SCOPE, FULL_DETAIL_SCOPE, HISTORY_SCOPE, PROMPT_SCOPE, Panel, RESULTS_SCOPE, SPLIT_SCOPE,
};
use crate::tui::picker::keymaps_picker::keymaps_picker;
use crate::tui::widgets::item_list::Motion;
use crate::tui::widgets::pane_search::{Direction, find};
use crate::tui::widgets::query_input::QueryInput;
use crate::tui::widgets::status_line::StatusMessage;

/// Quits the application.
pub const QUIT: CommandId = CommandId("app.quit");
/// Leaves insert mode.
pub const NORMAL_MODE: CommandId = CommandId("mode.normal");
/// Focuses the search bar in insert mode, keeping the query.
pub const FOCUS_SEARCH_BAR: CommandId = CommandId("focus.search_bar");
/// Focuses the history, as an overlay when the panel is hidden.
pub const FOCUS_HISTORY: CommandId = CommandId("focus.history");
/// Focuses the main panel.
pub const FOCUS_MAIN: CommandId = CommandId("focus.main");
/// Focuses the detail of the split.
pub const FOCUS_DETAIL: CommandId = CommandId("focus.detail");
/// Closes the history overlay.
pub const CLOSE_OVERLAY: CommandId = CommandId("history.close_overlay");
/// Shows the full detail in the main panel.
pub const FULL_DETAIL: CommandId = CommandId("main.full_detail");
/// Shows the detail next to the results.
pub const SPLIT: CommandId = CommandId("main.split");
/// Steps back from the split or the full detail to the results.
pub const BACK: CommandId = CommandId("main.back");
/// Steps back from the results, which quits.
pub const QUIT_FROM_RESULTS: CommandId = CommandId("main.quit");
/// Opens the `/` prompt to search the focused panel.
pub const SEARCH_PANE: CommandId = CommandId("pane.search");
/// Runs the search typed in the `/` prompt.
pub const RUN_PANE_SEARCH: CommandId = CommandId("pane.search.run");
/// Jumps to the next match of the last `/` search.
pub const NEXT_MATCH: CommandId = CommandId("pane.search.next");
/// Jumps to the previous match of the last `/` search.
pub const PREVIOUS_MATCH: CommandId = CommandId("pane.search.previous");
/// Shows the next card in the full detail.
pub const NEXT_CARD: CommandId = CommandId("detail.next_card");
/// Shows the previous card in the full detail.
pub const PREVIOUS_CARD: CommandId = CommandId("detail.previous_card");
/// Opens the keymaps picker.
pub const KEYMAPS: CommandId = CommandId("keymaps.picker");

impl App {
    /// Moves the results cursor while the full detail is shown, loading more results at the end.
    fn step_card(&mut self, motion: Motion) {
        let viewport = self.viewport(Panel::Main);
        if self.results.list_mut().apply(motion, viewport).end_reached {
            self.load_more();
        }
    }

    fn open_keymaps_picker(&mut self) {
        self.picker = Some(keymaps_picker(&self.commands, &self.keymap));
        self.enter_insert_mode();
    }

    fn open_prompt(&mut self) {
        self.prompt = Some(QueryInput::new());
        self.enter_insert_mode();
    }

    fn run_prompt(&mut self) {
        let pattern = self
            .prompt
            .take()
            .map(|p| p.value().to_owned())
            .unwrap_or_default();
        self.mode = Mode::Normal;
        if pattern.is_empty() {
            return;
        }
        self.last_pattern = Some(pattern);
        self.repeat_search(Direction::Forward);
    }

    fn repeat_search(&mut self, direction: Direction) {
        let Some(pattern) = self.last_pattern.clone() else {
            self.message = Some(StatusMessage::error("No previous search pattern"));
            return;
        };
        let Some((pane, viewport)) = self.focused_pane() else {
            self.message = Some(StatusMessage::error("Nothing to search in this panel"));
            return;
        };
        let found = find(
            pane.search_lines(),
            &pattern,
            pane.current_line(),
            direction,
        );
        if let Some(found) = found {
            pane.jump_to(found.index, viewport);
        }
        self.message = match found {
            None => Some(StatusMessage::error(format!(
                "Pattern not found: {pattern}"
            ))),
            Some(found) if found.wrapped => Some(StatusMessage::info(match direction {
                Direction::Forward => "Search hit the end, continued at the start",
                Direction::Backward => "Search hit the start, continued at the end",
            })),
            Some(_) => None,
        };
    }
}

/// Registers every built-in command and key binding.
pub(super) fn register_all(
    commands: &mut CommandRegistry<App>,
    keymap: &mut Keymap,
) -> Result<(), RegistrationError> {
    let mut reg = Registrar { commands, keymap };
    let global = Scope::GLOBAL;
    reg.add(
        Mode::Normal,
        &["Q"],
        command(QUIT, global, "Quit scry", None, |app| app.quit = true),
    )?;
    reg.add(
        Mode::Insert,
        &["<Esc>"],
        command(NORMAL_MODE, global, "Back to normal mode", None, |app| {
            app.mode = Mode::Normal;
            app.prompt = None;
            app.dispatcher.reset();
        }),
    )?;
    reg.add(
        Mode::Normal,
        &["1"],
        command(
            FOCUS_SEARCH_BAR,
            global,
            "Focus the search bar",
            None,
            |app| {
                app.set_focus(Panel::SearchBar);
                app.enter_insert_mode();
            },
        ),
    )?;
    reg.add(
        Mode::Normal,
        &["2"],
        command(FOCUS_HISTORY, global, "Focus the history", None, |app| {
            app.set_focus(Panel::History);
            app.history_overlay = app.areas_for(app.area).history.is_none();
        }),
    )?;
    reg.add(
        Mode::Normal,
        &["3"],
        command(FOCUS_MAIN, global, "Focus the main panel", None, |app| {
            app.set_focus(Panel::Main);
        }),
    )?;
    reg.add(
        Mode::Normal,
        &["0"],
        command(FOCUS_DETAIL, global, "Focus the detail", None, |app| {
            if app.areas().detail.is_some() {
                app.set_focus(Panel::Detail);
            }
        }),
    )?;
    reg.add(
        Mode::Normal,
        &["<Esc>"],
        command(
            CLOSE_OVERLAY,
            HISTORY_SCOPE,
            "Close the history overlay",
            None,
            |app| {
                if app.history_overlay {
                    app.set_focus(Panel::Main);
                }
            },
        ),
    )?;
    reg.add(
        Mode::Normal,
        &["<leader>sk"],
        command(
            KEYMAPS,
            global,
            "List every key and command",
            None,
            App::open_keymaps_picker,
        ),
    )?;
    register_main_panel_commands(&mut reg)?;
    register_pane_commands(&mut reg)?;
    super::search::register(&mut reg)?;
    super::history::register(&mut reg)?;
    super::sort::register(&mut reg)
}

fn register_pane_commands(reg: &mut Registrar<'_>) -> Result<(), RegistrationError> {
    let motion = |id: &'static str, description: &'static str, run: fn(&mut App)| {
        command(CommandId(id), Scope::GLOBAL, description, None, run)
    };
    let normal = Mode::Normal;
    reg.add(
        normal,
        &["j", "<Down>"],
        motion("pane.down", "Next row", |app| {
            app.motion(Motion::Down);
        }),
    )?;
    reg.add(
        normal,
        &["k", "<Up>"],
        motion("pane.up", "Previous row", |app| {
            app.motion(Motion::Up);
        }),
    )?;
    reg.add(
        normal,
        &["gg", "<Home>"],
        motion("pane.top", "First row", |app| {
            app.motion(Motion::Top);
        }),
    )?;
    reg.add(
        normal,
        &["G", "<End>"],
        motion("pane.bottom", "Last row", |app| {
            app.motion(Motion::Bottom);
        }),
    )?;
    reg.add(
        normal,
        &["<C-d>", "<PageDown>"],
        motion("pane.half_down", "Half a page down", |app| {
            app.motion(Motion::HalfPageDown);
        }),
    )?;
    reg.add(
        normal,
        &["<C-u>", "<PageUp>"],
        motion("pane.half_up", "Half a page up", |app| {
            app.motion(Motion::HalfPageUp);
        }),
    )?;
    reg.add(
        Mode::Normal,
        &["/"],
        command(
            SEARCH_PANE,
            Scope::GLOBAL,
            "Search the focused panel",
            None,
            App::open_prompt,
        ),
    )?;
    reg.add(
        Mode::Insert,
        &["<CR>"],
        command(
            RUN_PANE_SEARCH,
            PROMPT_SCOPE,
            "Run the search",
            None,
            App::run_prompt,
        ),
    )?;
    reg.add(
        Mode::Normal,
        &["n"],
        command(NEXT_MATCH, Scope::GLOBAL, "Next match", None, |app| {
            app.repeat_search(Direction::Forward);
        }),
    )?;
    reg.add(
        Mode::Normal,
        &["N"],
        command(
            PREVIOUS_MATCH,
            Scope::GLOBAL,
            "Previous match",
            None,
            |app| {
                app.repeat_search(Direction::Backward);
            },
        ),
    )?;
    Ok(())
}

fn register_main_panel_commands(reg: &mut Registrar<'_>) -> Result<(), RegistrationError> {
    let normal = Mode::Normal;
    reg.add(
        normal,
        &["d"],
        command(
            FULL_DETAIL,
            RESULTS_SCOPE,
            "Show the full detail",
            Some("detail"),
            |app| app.show_main(app.main_view.on_detail()),
        ),
    )?;
    reg.bind(normal, SPLIT_SCOPE, &["d"], FULL_DETAIL)?;
    reg.bind(normal, DETAIL_SCOPE, &["d"], FULL_DETAIL)?;
    reg.add(
        normal,
        &["<CR>"],
        command(
            SPLIT,
            RESULTS_SCOPE,
            "Show the detail next to the results",
            Some("split"),
            |app| app.show_main(app.main_view.on_split()),
        ),
    )?;
    reg.bind(normal, FULL_DETAIL_SCOPE, &["<CR>"], SPLIT)?;
    reg.add(
        normal,
        &["q"],
        command(
            QUIT_FROM_RESULTS,
            RESULTS_SCOPE,
            "Quit from the results",
            Some("quit"),
            App::main_back,
        ),
    )?;
    reg.add(
        normal,
        &["q"],
        command(
            BACK,
            SPLIT_SCOPE,
            "Back to the results",
            Some("back"),
            App::main_back,
        ),
    )?;
    reg.bind(normal, FULL_DETAIL_SCOPE, &["q"], BACK)?;
    reg.bind(normal, DETAIL_SCOPE, &["q"], BACK)?;
    reg.add(
        normal,
        &["J"],
        command(
            NEXT_CARD,
            FULL_DETAIL_SCOPE,
            "Next card",
            Some("next"),
            |app| app.step_card(Motion::Down),
        ),
    )?;
    reg.add(
        normal,
        &["K"],
        command(
            PREVIOUS_CARD,
            FULL_DETAIL_SCOPE,
            "Previous card",
            Some("prev"),
            |app| app.step_card(Motion::Up),
        ),
    )?;
    Ok(())
}
