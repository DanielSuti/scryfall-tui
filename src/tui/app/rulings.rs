//! Loading the rulings of the card the detail shows.

use std::sync::Arc;

use tokio::time::Instant;

use crate::app::config::RulingsMode;
use crate::domain::card::Card;
use crate::domain::ruling::{Ruling, RulingsState};
use crate::search::MAX_RATE_LIMIT_RETRIES;
use crate::tui::app::{App, AppEvent};
use crate::tui::main_panel::MainView;

/// Shown when the app runs without a rulings source, which only happens in tests.
const NO_RULINGS_SOURCE: &str = "no rulings source is configured";

/// A fetch that starts once the cursor has rested on its card until `due`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingRulings {
    key: String,
    card_id: String,
    due: Instant,
}

impl PendingRulings {
    /// Returns when the fetch starts.
    pub(super) fn due(&self) -> Instant {
        self.due
    }
}

/// Returns the key `card`'s rulings are kept under: its oracle id, shared by every printing,
/// or its printing id when it has none.
pub(super) fn rulings_key(card: &Card) -> String {
    card.oracle_id().unwrap_or(card.id()).to_owned()
}

impl App {
    /// Returns the rulings the detail shows for `card`: those known, or loading while a fetch
    /// waits for the cursor to rest. Returns `None` when they are not fetched in this view.
    pub(super) fn rulings_for(&self, card: &Card) -> Option<&RulingsState> {
        const LOADING: &RulingsState = &RulingsState::Loading;
        let key = rulings_key(card);
        self.rulings.get(&key).or_else(|| {
            self.pending_rulings
                .as_ref()
                .is_some_and(|p| p.key == key)
                .then_some(LOADING)
        })
    }

    /// Prepares the rulings of `card`, the card the detail shows, or of no card. In `auto`
    /// mode a fetch starts once the cursor has rested for `pause_ms`; in `full_detail` mode it
    /// starts at once in the full detail and not at all in the split. Rulings already known
    /// are not fetched again, except after a failure when the card is shown anew.
    pub(super) fn schedule_rulings(&mut self, card: Option<&Card>) {
        let key = card.map(rulings_key);
        let shown_anew = key != self.rulings_shown;
        self.rulings_shown.clone_from(&key);
        let (Some(card), Some(key)) = (card, key) else {
            self.pending_rulings = None;
            return;
        };
        if shown_anew && matches!(self.rulings.get(&key), Some(RulingsState::Failed(_))) {
            self.rulings.remove(&key);
        }
        let settings = &self.config.detail.rulings;
        let pause = match settings.mode {
            RulingsMode::Auto => settings.pause(),
            RulingsMode::FullDetail if self.main_view == MainView::FullDetail => {
                std::time::Duration::ZERO
            }
            RulingsMode::FullDetail => {
                self.pending_rulings = None;
                return;
            }
        };
        if self.rulings.contains_key(&key) {
            self.pending_rulings = None;
        } else if pause.is_zero() {
            self.pending_rulings = None;
            self.fetch_rulings(key, card.id().to_owned());
        } else if self.pending_rulings.as_ref().is_none_or(|p| p.key != key) {
            self.pending_rulings = Some(PendingRulings {
                key,
                card_id: card.id().to_owned(),
                due: Instant::now() + pause,
            });
        }
    }

    /// Starts the waiting fetch once its time has come.
    pub(super) fn start_due_rulings(&mut self, now: Instant) {
        if let Some(pending) = self.pending_rulings.take_if(|p| p.due <= now) {
            self.fetch_rulings(pending.key, pending.card_id);
        }
    }

    /// Fetches the rulings of the card with printing id `card_id` in the background, keeping
    /// them under `key`. A rate-limited request is retried up to [`MAX_RATE_LIMIT_RETRIES`]
    /// times; the source's own throttle does the waiting.
    fn fetch_rulings(&mut self, key: String, card_id: String) {
        let Some(services) = self.services.clone() else {
            self.rulings
                .insert(key, RulingsState::Failed(NO_RULINGS_SOURCE.to_owned()));
            return;
        };
        self.rulings.insert(key.clone(), RulingsState::Loading);
        let source = Arc::clone(&services.rulings);
        tokio::spawn(async move {
            let mut retries = 0;
            let result = loop {
                match source.rulings(&card_id).await {
                    Ok(rulings) => break Ok(rulings),
                    Err(err) if err.retry_after().is_some() && retries < MAX_RATE_LIMIT_RETRIES => {
                        retries += 1;
                    }
                    Err(err) => break Err(err.to_string()),
                }
            };
            // A closed channel means the app has quit; the rulings have no reader left.
            let _ = services.events.send(AppEvent::Rulings { key, result });
        });
    }

    /// Keeps the rulings fetched under `key`, or the reason they could not be.
    pub(super) fn handle_rulings(&mut self, key: String, result: Result<Vec<Ruling>, String>) {
        let state = match result {
            Ok(rulings) => RulingsState::Loaded(rulings),
            Err(message) => RulingsState::Failed(message),
        };
        self.rulings.insert(key, state);
    }
}
