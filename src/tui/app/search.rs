//! Running searches: the search bar commands, background batches and their updates.

use crate::app::command::{CommandId, Scope};
use crate::search::{BatchEvent, BatchStart, SearchRequest, load_batch};
use crate::tui::app::registrar::{Registrar, RegistrationError, command};
use crate::tui::app::{App, AppEvent, SearchUpdate};
use crate::tui::keymap::mode::Mode;
use crate::tui::layout::{Panel, SEARCH_BAR_SCOPE};
use crate::tui::main_panel::MainView;
use crate::tui::widgets::status_line::StatusMessage;

/// Focuses the search bar in insert mode with an empty query.
pub const NEW_SEARCH: CommandId = CommandId("search.new");
/// Runs the query typed in the search bar.
pub const RUN_SEARCH: CommandId = CommandId("search.run");

/// Shown when the app runs without a search source, which only happens in tests.
const NO_SEARCH_SOURCE: &str = "No search source is configured";

pub(super) fn register(reg: &mut Registrar<'_>) -> Result<(), RegistrationError> {
    reg.add(
        Mode::Normal,
        &["<leader>ss"],
        command(NEW_SEARCH, Scope::GLOBAL, "New search", None, |app| {
            app.set_focus(Panel::SearchBar);
            app.search_bar.clear();
            app.enter_insert_mode();
        }),
    )?;
    reg.add(
        Mode::Insert,
        &["<CR>"],
        command(
            RUN_SEARCH,
            SEARCH_BAR_SCOPE,
            "Run the search",
            None,
            App::submit_search,
        ),
    )?;
    Ok(())
}

impl App {
    fn submit_search(&mut self) {
        let query = self.search_bar.query().to_owned();
        if query.trim().is_empty() {
            self.message = Some(StatusMessage::error("Type a query first"));
            return;
        }
        self.mode = Mode::Normal;
        self.set_focus(Panel::Main);
        if self.main_view == MainView::FullDetail {
            self.show_main(MainView::Results);
        }
        self.record_search(&query);
        self.run_query(&query);
    }

    /// Shows the results of `query` in the session's sort order, from the cache when it holds
    /// them, and otherwise starts loading their first batch.
    pub(super) fn run_query(&mut self, query: &str) {
        let request = SearchRequest::new(query, self.sort_order);
        if let Some(start) = self.cache.begin(&request) {
            self.spawn_batch(&request, start);
        }
        self.refresh_results(&request);
    }

    /// Starts loading the next batch of the displayed request when its loading is paused.
    pub(super) fn load_more(&mut self) {
        let Some(request) = self.results.request().cloned() else {
            return;
        };
        if let Some(start) = self.cache.resume(&request) {
            self.spawn_batch(&request, start);
            self.refresh_results(&request);
        }
    }

    fn spawn_batch(&mut self, request: &SearchRequest, start: BatchStart) {
        let Some(services) = self.services.clone() else {
            if let Some(results) = self.cache.get_mut(request) {
                results.fail(NO_SEARCH_SOURCE);
            }
            return;
        };
        self.batches_running += 1;
        let max_pages = self.config.search.max_pages;
        let request = request.clone();
        tokio::spawn(async move {
            let events = services.events.clone();
            let send = |update| {
                // A closed channel means the app has quit; the update has no reader left.
                let _ = events.send(AppEvent::Search {
                    request: request.clone(),
                    update,
                });
            };
            load_batch(services.search.as_ref(), start, max_pages, |event| {
                send(match event {
                    BatchEvent::Page(page) => SearchUpdate::Page(page),
                    BatchEvent::RateLimited(pause) => SearchUpdate::RateLimited(pause),
                    BatchEvent::Failed(err) => SearchUpdate::Failed(err.to_string()),
                });
            })
            .await;
            send(SearchUpdate::BatchDone);
        });
    }

    pub(super) fn handle_search_update(&mut self, request: &SearchRequest, update: SearchUpdate) {
        let displayed = self.results.request() == Some(request);
        let Some(results) = self.cache.get_mut(request) else {
            return;
        };
        let first_card = match &update {
            SearchUpdate::Page(page) => page.cards.first().cloned(),
            _ => None,
        };
        let first_total = match &update {
            SearchUpdate::Page(page) if results.pages_loaded() == 0 => Some(page.total_cards),
            _ => None,
        };
        match update {
            SearchUpdate::Page(page) => results.apply(page),
            SearchUpdate::RateLimited(pause) => {
                if displayed {
                    self.message = Some(StatusMessage::info(format!(
                        "Scryfall rate limit reached, retrying in {} s",
                        pause.as_secs_f64().ceil()
                    )));
                }
            }
            SearchUpdate::Failed(message) => {
                results.fail(&message);
                if displayed {
                    self.message = Some(StatusMessage::error(message));
                }
            }
            SearchUpdate::BatchDone => {
                results.finish_batch();
                self.batches_running = self.batches_running.saturating_sub(1);
            }
        }
        if let Some(card) = first_card {
            self.check_formats(&card);
        }
        if let Some(total) = first_total {
            self.history.set_total(&request.query, total);
            self.history_changed();
        }
        if displayed {
            self.refresh_results(request);
        }
    }

    pub(super) fn refresh_results(&mut self, request: &SearchRequest) {
        if let Some(results) = self.cache.get(request) {
            self.results.show(request, results);
        }
    }
}
