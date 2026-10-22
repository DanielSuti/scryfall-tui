//! Entry point of the `scry` binary.

use std::process::ExitCode;

use clap::Parser;
use scry::app::config::Config;
use scry::app::paths;
use scry::cli::Cli;
use scry::tui::app::App;

#[tokio::main]
async fn main() -> ExitCode {
    let _cli = Cli::parse();
    let config = match paths::config_file() {
        Some(path) => match Config::load(&path) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("scry: {err}");
                return ExitCode::FAILURE;
            }
        },
        None => Config::default(),
    };
    match scry::tui::terminal::run(App::with_config(config)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("scry: {err}");
            ExitCode::FAILURE
        }
    }
}
