//! Terminal setup and restore around the event loop.

use std::io;

use tokio::sync::mpsc::UnboundedReceiver;

use crate::app::config::LoadMode;
use crate::tui::app::{App, AppEvent};
use crate::tui::{event_loop, graphics};

/// Switches the terminal to the alternate screen in raw mode, asks it for a graphics protocol
/// unless `detail.images.mode` is `never`, runs `app` with the background events arriving on
/// `background`, and restores the terminal afterwards, also when the loop fails.
/// `ratatui::init` installs a panic hook that restores the terminal on panic.
///
/// # Errors
/// Returns the error of the event loop.
pub async fn run(mut app: App, mut background: UnboundedReceiver<AppEvent>) -> io::Result<()> {
    let mut terminal = ratatui::init();
    if app.config().detail.images.mode != LoadMode::Never {
        // Before the event loop starts reading the terminal's input, where the answers arrive.
        app.set_image_picker(graphics::detect());
    }
    let result = event_loop::run(&mut terminal, &mut app, &mut background).await;
    ratatui::restore();
    result
}
