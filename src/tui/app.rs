//! Application state and event handling.

use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tokio::time::Instant;

use crate::app::command::{Command, CommandId, CommandRegistry, Scope};
use crate::tui::keymap::bindings::Keymap;
use crate::tui::keymap::dispatcher::{Dispatch, Dispatcher};
use crate::tui::keymap::mode::Mode;
use crate::tui::keymap::sequence::Key;
use crate::tui::layout::{
    DETAIL_SCOPE, FULL_DETAIL_SCOPE, HISTORY_SCOPE, Panel, RESULTS_SCOPE, SEARCH_BAR_SCOPE,
    SPLIT_SCOPE, ScreenAreas, panel_block,
};
use crate::tui::main_panel::{Back, MainView};
use crate::tui::widgets::query_input::QueryInput;
use crate::tui::widgets::status_line::StatusLine;
use crate::tui::widgets::which_key::{WhichKey, WhichKeyEntry, WhichKeyPopup};

/// The key that `<leader>` stands for.
pub const LEADER: Key = Key::char(' ');

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

/// An input to the application, from the terminal, a timer or a background task.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// A key event from the terminal.
    Key(KeyEvent),
    /// A deadline returned by [`App::next_deadline`] has passed.
    Tick,
    /// The terminal now has this many columns and rows.
    Resize(u16, u16),
}

/// The whole application state. Rendering is a pure function of it.
#[derive(Debug)]
pub struct App {
    mode: Mode,
    quit: bool,
    commands: CommandRegistry<App>,
    keymap: Keymap,
    dispatcher: Dispatcher,
    input: QueryInput,
    which_key: WhichKey,
    area: Rect,
    focus: Panel,
    history_overlay: bool,
    main_view: MainView,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Creates the application in normal mode with all commands and keys registered.
    ///
    /// # Panics
    /// Panics if two built-in commands share an id or two built-in key bindings conflict. Both
    /// are programming errors caught by the tests.
    pub fn new() -> Self {
        let mut app = Self {
            mode: Mode::Normal,
            quit: false,
            commands: CommandRegistry::new(),
            keymap: Keymap::new(LEADER),
            dispatcher: Dispatcher::new(),
            input: QueryInput::new(),
            which_key: WhichKey::default(),
            area: Rect::default(),
            focus: Panel::Main,
            history_overlay: false,
            main_view: MainView::Results,
        };
        register_app_commands(&mut app.commands, &mut app.keymap)
            .expect("built-in commands and keys are consistent");
        app
    }

    /// Returns true once the application has been asked to quit.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Returns the current mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Returns the command registry.
    pub fn commands(&self) -> &CommandRegistry<App> {
        &self.commands
    }

    /// Returns the keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Applies one event to the state.
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Tick => self.which_key.tick(Instant::now()),
            AppEvent::Resize(width, height) => self.area = Rect::new(0, 0, width, height),
        }
    }

    /// Returns the focused panel.
    pub fn focus(&self) -> Panel {
        self.focus
    }

    /// Returns the screen areas for the last known terminal size.
    pub fn areas(&self) -> ScreenAreas {
        self.areas_for(self.area)
    }

    fn areas_for(&self, area: Rect) -> ScreenAreas {
        ScreenAreas::compute(
            area,
            self.main_view == MainView::Split,
            self.history_overlay,
        )
    }

    /// Returns what the main panel shows.
    pub fn main_view(&self) -> MainView {
        self.main_view
    }

    fn show_main(&mut self, view: MainView) {
        self.main_view = view;
        if view != MainView::Split && self.focus == Panel::Detail {
            self.focus = Panel::Main;
        }
    }

    fn main_back(&mut self) {
        match self.main_view.on_back() {
            Back::To(view) => self.show_main(view),
            Back::Quit => self.quit = true,
        }
    }

    fn set_focus(&mut self, panel: Panel) {
        if panel != Panel::History {
            self.history_overlay = false;
        }
        self.focus = panel;
    }

    /// Returns when the app next needs an [`AppEvent::Tick`], or `None` when it waits only
    /// for other events.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.which_key.deadline()
    }

    /// Returns the entries of the which-key popup, or an empty list when it is hidden.
    pub fn which_key_entries(&self) -> Vec<WhichKeyEntry> {
        if !self.which_key.visible() {
            return Vec::new();
        }
        self.which_key
            .continuations()
            .iter()
            .map(|c| WhichKeyEntry {
                keys: c.rest.to_string(),
                description: self
                    .commands
                    .get(c.command)
                    .map_or_else(|| c.command.to_string(), |cmd| cmd.description.to_owned()),
            })
            .collect()
    }

    /// Runs the command with `id`. Unknown ids are ignored, because every bound id is
    /// registered at start-up.
    pub fn run_command(&mut self, id: CommandId) {
        if let Some(run) = self.commands.get(id).map(|command| command.run) {
            run(self);
        }
    }

    /// Returns the scope of the focused panel.
    pub fn focused_scope(&self) -> Scope {
        match self.focus {
            Panel::SearchBar => SEARCH_BAR_SCOPE,
            Panel::History => HISTORY_SCOPE,
            Panel::Main => match self.main_view {
                MainView::Results => RESULTS_SCOPE,
                MainView::Split => SPLIT_SCOPE,
                MainView::FullDetail => FULL_DETAIL_SCOPE,
            },
            Panel::Detail => DETAIL_SCOPE,
        }
    }

    fn active_scopes(&self) -> Vec<Scope> {
        let focused = self.focused_scope();
        if focused == Scope::GLOBAL {
            vec![Scope::GLOBAL]
        } else {
            vec![focused, Scope::GLOBAL]
        }
    }

    /// Returns the status-line hints of `scope`: each command with a hint label and a
    /// normal-mode binding in that scope, in registration order, formatted as its first such
    /// key and the label.
    pub fn hints_for(&self, scope: Scope) -> Vec<String> {
        self.commands
            .iter()
            .filter_map(|command| {
                let hint = command.hint?;
                let binding = self
                    .keymap
                    .bindings_for(command.id)
                    .find(|b| b.mode == Mode::Normal && b.scope == scope)?;
                Some(format!(
                    "{} {hint}",
                    hint_keys(&binding.sequence.to_string())
                ))
            })
            .collect()
    }

    fn handle_key(&mut self, event: KeyEvent) {
        if event.kind == KeyEventKind::Release {
            return;
        }
        let Some(key) = Key::from_event(&event) else {
            return;
        };
        let scopes = self.active_scopes();
        match self.dispatcher.feed(&self.keymap, self.mode, &scopes, key) {
            Dispatch::Matched(id) => {
                self.which_key.clear();
                self.run_command(id);
            }
            Dispatch::Pending(continuations) => {
                self.which_key.start(continuations, Instant::now());
            }
            Dispatch::NoMatch(_) => {
                self.which_key.clear();
                if self.mode == Mode::Insert {
                    self.input.handle_key(event);
                }
            }
        }
    }

    /// Switches to insert mode, so unbound keys edit the focused text input.
    pub fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
        self.dispatcher.reset();
    }

    /// Draws the whole screen.
    pub fn render(&self, frame: &mut Frame) {
        let areas = self.areas_for(frame.area());
        let focused = |panel| self.focus == panel;

        let block = panel_block(self.title(Panel::SearchBar), focused(Panel::SearchBar));
        let inner = block.inner(areas.search);
        frame.render_widget(block, areas.search);
        self.input.render(
            frame,
            inner,
            focused(Panel::SearchBar) && self.mode == Mode::Insert,
        );

        if let Some(history) = areas.history {
            frame.render_widget(
                panel_block(self.title(Panel::History), focused(Panel::History)),
                history,
            );
        }
        frame.render_widget(
            panel_block(self.title(Panel::Main), focused(Panel::Main)),
            areas.main,
        );
        if let Some(detail) = areas.detail {
            frame.render_widget(
                panel_block(self.title(Panel::Detail), focused(Panel::Detail)),
                detail,
            );
        }
        if let Some(overlay) = areas.overlay {
            frame.render_widget(ratatui::widgets::Clear, overlay);
            frame.render_widget(
                panel_block(self.title(Panel::History), focused(Panel::History)),
                overlay,
            );
        }

        let hints = self.hints_for(self.focused_scope());
        let panel = self.title(self.focus);
        frame.render_widget(
            StatusLine::new(self.mode).section(panel).hints(&hints),
            areas.status,
        );
        let entries = self.which_key_entries();
        if !entries.is_empty() {
            let body = Rect {
                height: areas.status.y.saturating_sub(areas.search.y),
                ..frame.area()
            };
            WhichKeyPopup::new(&entries).render(body, frame.buffer_mut());
        }
    }

    fn title(&self, panel: Panel) -> String {
        let name = match panel {
            Panel::SearchBar => "Search",
            Panel::History => "History",
            Panel::Main if self.main_view == MainView::FullDetail => "Detail",
            Panel::Main => "Results",
            Panel::Detail => "Detail",
        };
        format!("[{}] {name}", panel.jump_key())
    }
}

/// Shortens key notation for the hint line: `<CR>` becomes `⏎`.
fn hint_keys(notation: &str) -> String {
    notation.replace("<CR>", "⏎")
}

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
    commands: &'a mut CommandRegistry<App>,
    keymap: &'a mut Keymap,
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

fn command(
    id: CommandId,
    scope: Scope,
    description: &'static str,
    hint: Option<&'static str>,
    run: fn(&mut App),
) -> Command<App> {
    Command {
        id,
        description,
        scope,
        hint,
        run,
    }
}

fn register_app_commands(
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
    register_main_panel_commands(&mut reg)
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn key(code: KeyCode) -> AppEvent {
        AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_event(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn capital_q_quits_in_normal_mode() {
        let mut app = App::new();
        type_text(&mut app, "2");
        app.handle_event(key(KeyCode::Char('q')));
        assert!(!app.should_quit());
        app.handle_event(key(KeyCode::Char('Q')));
        assert!(app.should_quit());
    }

    #[test]
    fn insert_mode_keys_reach_the_input() {
        let mut app = App::new();
        app.enter_insert_mode();
        type_text(&mut app, "Q x");
        assert!(!app.should_quit());
        assert_eq!(app.input.value(), "Q x");
        app.handle_event(key(KeyCode::Esc));
        assert_eq!(app.mode(), Mode::Normal);
        type_text(&mut app, "y");
        assert_eq!(app.input.value(), "Q x");
        app.handle_event(key(KeyCode::Char('Q')));
        assert!(app.should_quit());
    }

    fn test_command(app: &mut App, id: &'static str, notation: &str, description: &'static str) {
        scoped_command(app, Scope::GLOBAL, id, notation, description, None);
    }

    fn scoped_command(
        app: &mut App,
        scope: Scope,
        id: &'static str,
        notation: &str,
        description: &'static str,
        hint: Option<&'static str>,
    ) {
        app.commands
            .register(Command {
                id: CommandId(id),
                description,
                scope,
                hint,
                run: |_| {},
            })
            .unwrap();
        app.keymap
            .bind(Mode::Normal, scope, notation, CommandId(id))
            .unwrap();
    }

    #[test]
    fn hints_come_from_the_registry_of_the_scope() {
        let mut app = App::new();
        let results = Scope("test_results");
        let history = Scope("test_history");
        scoped_command(&mut app, results, "r.detail", "d", "Detail", Some("detail"));
        scoped_command(&mut app, results, "r.split", "<CR>", "Split", Some("split"));
        scoped_command(&mut app, results, "r.top", "gg", "Top", None);
        scoped_command(&mut app, history, "h.edit", "e", "Edit", Some("edit"));
        assert_eq!(app.hints_for(results), ["d detail", "⏎ split"]);
        assert_eq!(app.hints_for(history), ["e edit"]);
        assert!(app.hints_for(Scope::GLOBAL).is_empty());
    }

    fn app_with_leader_commands() -> App {
        let mut app = App::new();
        test_command(&mut app, "test.one", "<leader>ab", "First");
        test_command(&mut app, "test.two", "<leader>c", "Second");
        app
    }

    #[tokio::test(start_paused = true)]
    async fn which_key_appears_after_the_delay_and_lists_the_registry() {
        let mut app = app_with_leader_commands();
        app.handle_event(key(KeyCode::Char(' ')));
        app.handle_event(AppEvent::Tick);
        assert!(app.which_key_entries().is_empty());
        let deadline = app.next_deadline().unwrap();
        tokio::time::advance(deadline - Instant::now()).await;
        app.handle_event(AppEvent::Tick);
        let listed: Vec<(String, String)> = app
            .which_key_entries()
            .into_iter()
            .map(|e| (e.keys, e.description))
            .collect();
        let expected: Vec<(String, String)> = app
            .keymap
            .bindings()
            .filter(|b| b.keys.first() == Some(&LEADER))
            .map(|b| {
                (
                    b.sequence.tail(1).unwrap().to_string(),
                    app.commands.get(b.command).unwrap().description.to_owned(),
                )
            })
            .collect();
        assert_eq!(listed, expected);
        assert_eq!(listed.len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn escape_closes_which_key() {
        let mut app = app_with_leader_commands();
        app.handle_event(key(KeyCode::Char(' ')));
        tokio::time::advance(crate::tui::widgets::which_key::WHICH_KEY_DELAY).await;
        app.handle_event(AppEvent::Tick);
        assert!(!app.which_key_entries().is_empty());
        app.handle_event(key(KeyCode::Esc));
        assert!(app.which_key_entries().is_empty());
        assert_eq!(app.next_deadline(), None);
    }

    #[tokio::test(start_paused = true)]
    async fn renders_which_key_popup() {
        let mut app = app_with_leader_commands();
        app.handle_event(key(KeyCode::Char(' ')));
        tokio::time::advance(crate::tui::widgets::which_key::WHICH_KEY_DELAY).await;
        app.handle_event(AppEvent::Tick);
        app.handle_event(AppEvent::Resize(40, 10));
        insta::assert_snapshot!(render(&app));
    }

    fn sized(width: u16, height: u16) -> App {
        let mut app = App::new();
        app.handle_event(AppEvent::Resize(width, height));
        app
    }

    fn render(app: &App) -> TestBackend {
        let mut terminal =
            Terminal::new(TestBackend::new(app.area.width, app.area.height)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        terminal.backend().clone()
    }

    #[test]
    fn number_keys_jump_between_panels() {
        let mut app = sized(112, 12);
        assert_eq!(app.focus(), Panel::Main);
        type_text(&mut app, "2");
        assert_eq!(app.focus(), Panel::History);
        assert_eq!(app.focused_scope(), HISTORY_SCOPE);
        type_text(&mut app, "0");
        assert_eq!(app.focus(), Panel::History, "0 is a no-op without a split");
        type_text(&mut app, "3");
        assert_eq!(app.focus(), Panel::Main);
        type_text(&mut app, "1");
        assert_eq!(app.focus(), Panel::SearchBar);
        assert_eq!(app.mode(), Mode::Insert);
        type_text(&mut app, "2");
        assert_eq!(
            app.focus(),
            Panel::SearchBar,
            "digits are text in insert mode"
        );
        assert_eq!(app.input.value(), "2");
    }

    #[test]
    fn history_opens_as_overlay_on_narrow_terminals() {
        let mut app = sized(99, 12);
        type_text(&mut app, "2");
        assert!(app.areas().overlay.is_some());
        app.handle_event(key(KeyCode::Esc));
        assert_eq!(app.focus(), Panel::Main);
        assert!(app.areas().overlay.is_none());
        type_text(&mut app, "23");
        assert!(
            app.areas().overlay.is_none(),
            "leaving the history closes the overlay"
        );
        let mut wide = sized(100, 12);
        type_text(&mut wide, "2");
        assert!(wide.areas().overlay.is_none());
    }

    fn press(app: &mut App, notation: &str) {
        let keys = crate::tui::keymap::sequence::KeySequence::parse(notation)
            .unwrap()
            .resolve(LEADER);
        for k in keys {
            let modifiers = KeyModifiers::NONE;
            app.handle_event(AppEvent::Key(KeyEvent::new(k.code(), modifiers)));
        }
    }

    #[test]
    fn main_panel_keys_follow_the_transition_table() {
        let mut app = sized(112, 12);
        press(&mut app, "<CR>");
        assert_eq!(app.main_view(), MainView::Split);
        assert_eq!(app.focus(), Panel::Main, "results keep focus");
        press(&mut app, "<CR>");
        assert_eq!(app.main_view(), MainView::Split, "no-op");
        press(&mut app, "d");
        assert_eq!(app.main_view(), MainView::FullDetail);
        press(&mut app, "d");
        assert_eq!(app.main_view(), MainView::FullDetail, "no-op");
        press(&mut app, "<CR>");
        press(&mut app, "0");
        assert_eq!(app.focus(), Panel::Detail);
        press(&mut app, "q");
        assert_eq!(app.main_view(), MainView::Results);
        assert_eq!(app.focus(), Panel::Main);
        press(&mut app, "d");
        press(&mut app, "q");
        assert_eq!(app.main_view(), MainView::Results);
        assert!(!app.should_quit());
        press(&mut app, "q");
        assert!(app.should_quit(), "q quits only from the results");
    }

    #[test]
    fn hints_follow_the_main_view() {
        let mut app = sized(112, 12);
        assert_eq!(
            app.hints_for(app.focused_scope()),
            ["d detail", "⏎ split", "q quit"]
        );
        press(&mut app, "<CR>");
        assert_eq!(app.hints_for(app.focused_scope()), ["d detail", "q back"]);
        press(&mut app, "d");
        assert_eq!(app.hints_for(app.focused_scope()), ["⏎ split", "q back"]);
    }

    #[test]
    fn renders_split() {
        let mut app = sized(112, 8);
        press(&mut app, "<CR>");
        insta::assert_snapshot!(render(&app));
    }

    #[test]
    fn renders_layout_at_the_threshold() {
        insta::assert_snapshot!(render(&sized(100, 10)));
    }

    #[test]
    fn renders_layout_below_the_threshold() {
        insta::assert_snapshot!(render(&sized(99, 10)));
    }

    #[test]
    fn renders_history_overlay() {
        let mut app = sized(99, 10);
        type_text(&mut app, "2");
        insta::assert_snapshot!(render(&app));
    }
}
