//! Drives an [`App`] wired to the fake Scryfall, without a terminal.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use scry::app::config::Config;
use scry::scryfall::client::{RateLimits, ScryfallClient};
use scry::search::api::ApiSearch;
use scry::tui::app::{App, AppEvent, LEADER, Services};
use scry::tui::keymap::sequence::KeySequence;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedReceiver;
use wiremock::MockServer;

use super::fake_scryfall::FakeScryfall;

/// No throttling, so tests run fast; the fake's own interval checks still apply.
pub const NO_LIMITS: RateLimits = RateLimits {
    search: Duration::ZERO,
    other: Duration::ZERO,
    images: Duration::ZERO,
    rate_limited_pause: Duration::from_millis(50),
};

/// An app, its background events and the fake it searches.
pub struct Harness {
    /// The app under test.
    pub app: App,
    /// The fake Scryfall.
    pub fake: FakeScryfall,
    background: UnboundedReceiver<AppEvent>,
    _server: MockServer,
}

impl Harness {
    /// Starts a fake holding `cards` with `page_size` cards per page, and an app of 112 by 24
    /// cells loading `max_pages` pages per batch.
    pub async fn start(cards: Vec<Value>, page_size: usize, max_pages: u32) -> Self {
        let mut config = Config::default();
        config.search.max_pages = NonZeroU32::new(max_pages).unwrap();
        // Tests that are not about base terms search without them.
        config.search.base = String::new();
        Self::start_with_config(cards, page_size, config).await
    }

    /// Like [`Harness::start`], with `config`.
    pub async fn start_with_config(cards: Vec<Value>, page_size: usize, config: Config) -> Self {
        let (server, fake) = FakeScryfall::start(cards).await;
        fake.set_page_size(page_size);
        let base_url = server.uri();
        Self::assemble(server, fake, &base_url, config)
    }

    /// Starts an app whose search source is an address nothing listens on.
    pub async fn start_unreachable() -> Self {
        let (server, fake) = FakeScryfall::start(Vec::new()).await;
        Self::assemble(server, fake, "http://127.0.0.1:9", Config::default())
    }

    fn assemble(server: MockServer, fake: FakeScryfall, base_url: &str, config: Config) -> Self {
        let client = ScryfallClient::with_base_url(base_url.parse().unwrap(), NO_LIMITS).unwrap();
        let (events, background) = tokio::sync::mpsc::unbounded_channel();
        let api = Arc::new(ApiSearch::new(client));
        let services = Services {
            search: api.clone(),
            rulings: api,
            events,
        };
        let mut app = App::with_services(config, services);
        app.handle_event(AppEvent::Resize(112, 24));
        Self {
            app,
            fake,
            background,
            _server: server,
        }
    }

    /// Loads the history from `path` and saves it there after every change.
    pub fn use_history_file(&mut self, path: &std::path::Path) {
        let max = self.app.config().history.max_entries;
        let history = scry::app::history::History::load(path, max).unwrap();
        self.app.set_history(history, Some(path.to_owned()));
    }

    /// Presses the keys of `notation`, such as `<leader>ss`.
    pub fn press(&mut self, notation: &str) {
        for key in KeySequence::parse(notation).unwrap().resolve(LEADER) {
            self.app.handle_event(AppEvent::Key(key.to_event()));
        }
    }

    /// Types `text` character by character, spaces included.
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            let key = scry::tui::keymap::sequence::Key::char(c);
            self.app.handle_event(AppEvent::Key(key.to_event()));
        }
    }

    /// Feeds the next background event to the app.
    pub async fn next_event(&mut self) {
        let event = tokio::time::timeout(Duration::from_secs(5), self.background.recv())
            .await
            .expect("a background event within 5 s")
            .expect("the channel stays open");
        self.app.handle_event(event);
    }

    /// Waits until the app's next deadline, then sends it a tick.
    pub async fn tick_at_deadline(&mut self) {
        let deadline = self
            .app
            .next_deadline()
            .expect("the app waits for a deadline");
        tokio::time::sleep_until(deadline).await;
        self.app.handle_event(AppEvent::Tick);
    }

    /// Returns the screen as text, one line per row.
    pub fn screen(&self) -> String {
        let backend = self.render();
        let buffer = backend.buffer();
        let width = usize::from(buffer.area.width);
        buffer
            .content()
            .chunks(width)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Returns the status line.
    pub fn status_line(&self) -> String {
        self.screen().lines().last().unwrap_or_default().to_owned()
    }

    /// Feeds background events to the app until no batch is running.
    pub async fn settle(&mut self) {
        while self.app.is_loading() {
            self.next_event().await;
        }
    }

    /// Runs `query` from the search bar and waits for its batch.
    pub async fn search(&mut self, query: &str) {
        self.press("<leader>ss");
        self.type_text(query);
        self.press("<CR>");
        self.settle().await;
    }

    /// Returns the names of the displayed results.
    pub fn result_names(&self) -> Vec<String> {
        self.app
            .displayed_results()
            .map(|r| r.cards().iter().map(|c| c.display_name()).collect())
            .unwrap_or_default()
    }

    /// Draws the app and returns the screen.
    pub fn render(&self) -> TestBackend {
        let mut terminal = Terminal::new(TestBackend::new(112, 24)).unwrap();
        terminal.draw(|frame| self.app.render(frame)).unwrap();
        terminal.backend().clone()
    }
}
