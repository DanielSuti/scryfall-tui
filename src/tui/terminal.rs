//! Terminal setup and restore around the event loop.

use std::io;

use crate::tui::app::App;
use crate::tui::event_loop;

/// Switches the terminal to the alternate screen in raw mode, runs `app`, and restores the
/// terminal afterwards, also when the loop fails. `ratatui::init` installs a panic hook that
/// restores the terminal on panic.
///
/// # Errors
/// Returns the error of the event loop.
pub async fn run(mut app: App) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop::run(&mut terminal, &mut app).await;
    ratatui::restore();
    result
}
