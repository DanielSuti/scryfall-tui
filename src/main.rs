//! Entry point of the `scry` binary.

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use scry::app::config::Config;
use scry::app::paths;
use scry::cli::Cli;
use scry::scryfall::client::ScryfallClient;
use scry::search::api::ApiSearch;
use scry::tui::app::{App, Services};

#[tokio::main]
async fn main() -> ExitCode {
    let _cli = Cli::parse();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("scry: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = match paths::config_file() {
        Some(path) => Config::load(&path)?,
        None => Config::default(),
    };
    let (events, background) = tokio::sync::mpsc::unbounded_channel();
    let services = Services {
        search: Arc::new(ApiSearch::new(ScryfallClient::new()?)),
        events,
    };
    scry::tui::terminal::run(App::with_services(config, services), background).await?;
    Ok(())
}
