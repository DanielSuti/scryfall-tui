//! The asynchronous loop that draws the screen and feeds events to the app.

use std::io;

use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use ratatui::DefaultTerminal;

use crate::tui::app::{App, AppEvent};

/// Draws `app` and feeds it terminal events until it asks to quit.
///
/// # Errors
/// Returns an error when drawing fails or the terminal event stream fails or ends.
pub async fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut terminal_events = EventStream::new();
    while !app.should_quit() {
        terminal.draw(|frame| app.render(frame))?;
        match terminal_events.next().await {
            Some(Ok(Event::Key(key))) => app.handle_event(AppEvent::Key(key)),
            Some(Ok(_)) => {}
            Some(Err(err)) => return Err(err),
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "terminal event stream ended",
                ));
            }
        }
    }
    Ok(())
}
