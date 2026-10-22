//! The asynchronous loop that draws the screen and feeds events to the app.

use std::io;

use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;

use crate::tui::app::{App, AppEvent};

/// Draws `app` and feeds it terminal events, the events of its background tasks from
/// `background`, and a tick at each deadline the app asks for, until it asks to quit.
///
/// # Errors
/// Returns an error when drawing fails or the terminal event stream fails or ends.
pub async fn run(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    background: &mut UnboundedReceiver<AppEvent>,
) -> io::Result<()> {
    let mut terminal_events = EventStream::new();
    let size = terminal.size()?;
    app.handle_event(AppEvent::Resize(size.width, size.height));
    while !app.should_quit() {
        terminal.draw(|frame| app.render(frame))?;
        let deadline = app.next_deadline();
        tokio::select! {
            event = terminal_events.next() => match event {
                Some(Ok(Event::Key(key))) => app.handle_event(AppEvent::Key(key)),
                Some(Ok(Event::Resize(width, height))) => {
                    app.handle_event(AppEvent::Resize(width, height));
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => return Err(err),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "terminal event stream ended",
                    ));
                }
            },
            Some(event) = background.recv() => app.handle_event(event),
            () = sleep_until(deadline) => app.handle_event(AppEvent::Tick),
        }
    }
    Ok(())
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}
