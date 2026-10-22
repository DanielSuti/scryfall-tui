//! Application state and event handling.

use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::app::command::{Command, CommandId, CommandRegistry, Scope};
use crate::tui::keymap::bindings::Keymap;
use crate::tui::keymap::dispatcher::{Dispatch, Dispatcher};
use crate::tui::keymap::mode::Mode;
use crate::tui::keymap::sequence::Key;
use crate::tui::widgets::query_input::QueryInput;
use crate::tui::widgets::status_line::StatusLine;

/// The key that `<leader>` stands for.
pub const LEADER: Key = Key::char(' ');

/// Quits the application.
pub const QUIT: CommandId = CommandId("app.quit");
/// Leaves insert mode.
pub const NORMAL_MODE: CommandId = CommandId("mode.normal");

/// An input to the application, from the terminal, a timer or a background task.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// A key event from the terminal.
    Key(KeyEvent),
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
        }
    }

    /// Runs the command with `id`. Unknown ids are ignored, because every bound id is
    /// registered at start-up.
    pub fn run_command(&mut self, id: CommandId) {
        if let Some(run) = self.commands.get(id).map(|command| command.run) {
            run(self);
        }
    }

    fn active_scopes(&self) -> Vec<Scope> {
        vec![Scope::GLOBAL]
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
            Dispatch::Matched(id) => self.run_command(id),
            Dispatch::Pending(_) => {}
            Dispatch::NoMatch(_) => {
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
        let [_body, status] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
        frame.render_widget(StatusLine::new(self.mode), status);
    }
}

fn register_app_commands(
    commands: &mut CommandRegistry<App>,
    keymap: &mut Keymap,
) -> Result<(), Box<dyn std::error::Error>> {
    commands.register(Command {
        id: QUIT,
        description: "Quit scry",
        scope: Scope::GLOBAL,
        hint: None,
        run: |app| app.quit = true,
    })?;
    commands.register(Command {
        id: NORMAL_MODE,
        description: "Back to normal mode",
        scope: Scope::GLOBAL,
        hint: None,
        run: |app| {
            app.mode = Mode::Normal;
            app.dispatcher.reset();
        },
    })?;
    keymap.bind(Mode::Normal, Scope::GLOBAL, "Q", QUIT)?;
    keymap.bind(Mode::Insert, Scope::GLOBAL, "<Esc>", NORMAL_MODE)?;
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

    #[test]
    fn renders_mode_in_status_line() {
        let app = App::new();
        let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }
}
