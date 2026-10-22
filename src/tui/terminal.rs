//! Terminal setup and restore around the event loop.

use std::io;

use tokio::sync::mpsc::UnboundedReceiver;

use crate::tui::app::{App, AppEvent};
use crate::tui::event_loop;

/// Switches the terminal to the alternate screen in raw mode, runs `app` with the background
/// events arriving on `background`, and restores the
/// terminal afterwards, also when the loop fails. `ratatui::init` installs a panic hook that
/// restores the terminal on panic.
///
/// # Errors
/// Returns the error of the event loop.
pub async fn run(mut app: App, mut background: UnboundedReceiver<AppEvent>) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop::run(&mut terminal, &mut app, &mut background).await;
    ratatui::restore();
    result
}
