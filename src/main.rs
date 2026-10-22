//! Entry point of the `scry` binary.

use std::process::ExitCode;

use clap::Parser;
use scry::cli::Cli;
use scry::tui::app::App;

#[tokio::main]
async fn main() -> ExitCode {
    let _cli = Cli::parse();
    match scry::tui::terminal::run(App::new()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("scry: {err}");
            ExitCode::FAILURE
        }
    }
}
