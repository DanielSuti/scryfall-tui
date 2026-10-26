//! Entry point of the `scry` binary.

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use scry::app::config::Config;
use scry::app::history::History;
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
    let config_path = paths::config_file();
    let config = match &config_path {
        Some(path) => Config::load(path)?,
        None => Config::default(),
    };
    let history_path = paths::history_file();
    let history = match &history_path {
        Some(path) => History::load(path, config.history.max_entries)?,
        None => History::new(config.history.max_entries),
    };
    let (events, background) = tokio::sync::mpsc::unbounded_channel();
    // One client for all, so searches, rulings and images share its rate limits.
    let api = Arc::new(ApiSearch::new(ScryfallClient::new()?));
    let services = Services {
        search: api.clone(),
        rulings: api.clone(),
        images: api,
        events,
    };
    let mut app = App::with_services(config, services);
    app.set_history(history, history_path);
    app.set_config_path(config_path);
    scry::tui::terminal::run(app, background).await?;
    Ok(())
}
