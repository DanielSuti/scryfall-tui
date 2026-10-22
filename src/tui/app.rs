//! Application state and event handling.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::tui::keymap::mode::Mode;
use crate::tui::widgets::status_line::StatusLine;

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
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Creates the application in normal mode.
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            quit: false,
        }
    }

    /// Returns true once the application has been asked to quit.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Returns the current mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Applies one event to the state.
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        if key.code == KeyCode::Char('Q') {
            self.quit = true;
        }
    }

    /// Draws the whole screen.
    pub fn render(&self, frame: &mut Frame) {
        let [_body, status] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
        frame.render_widget(StatusLine::new(self.mode), status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn key(code: KeyCode) -> AppEvent {
        AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn capital_q_quits() {
        let mut app = App::new();
        app.handle_event(key(KeyCode::Char('q')));
        assert!(!app.should_quit());
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
