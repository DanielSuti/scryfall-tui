//! Loading the rulings of the card the detail shows.

use std::sync::Arc;

use tokio::time::Instant;

use crate::app::config::LoadMode;
use crate::domain::card::Card;
use crate::domain::ruling::{Ruling, RulingsState};
use crate::search::MAX_RATE_LIMIT_RETRIES;
use crate::tui::app::detail_load::Known;
use crate::tui::app::{App, AppEvent};
use crate::tui::main_panel::MainView;
use crate::tui::views::detail::RulingsShown;

/// Shown when the app runs without a rulings source, which only happens in tests.
const NO_RULINGS_SOURCE: &str = "no rulings source is configured";

/// Returns the key `card`'s rulings are kept under: its oracle id, shared by every printing,
/// or its printing id when it has none.
pub(super) fn rulings_key(card: &Card) -> String {
    card.oracle_id().unwrap_or(card.id()).to_owned()
}

impl App {
    /// Returns the rulings section the detail shows for `card`: the rulings known, loading
    /// while a fetch waits for the cursor to rest, or the key that loads them in `trigger`
    /// mode. Returns `None` when they are not fetched yet in `auto` mode, or when the view is
    /// set to `never`, which hides them even when known.
    pub(super) fn rulings_shown(&self, card: &Card) -> Option<RulingsShown> {
        let mode = self.rulings_mode();
        if mode == LoadMode::Never {
            return None;
        }
        let key = rulings_key(card);
        if let Some(state) = self.rulings.get(&key) {
            return Some(RulingsShown::State(state.clone()));
        }
        if self.rulings_schedule.is_pending(&key) {
            return Some(RulingsShown::State(RulingsState::Loading));
        }
        (mode == LoadMode::Trigger).then(|| RulingsShown::OnTrigger(self.load_key()))
    }

    /// Fetches the rulings of the shown card at once, unless they are known or loading or the
    /// view is set to `never`. A failure is fetched again. Returns true when a fetch started.
    pub(super) fn load_rulings_now(&mut self) -> bool {
        if self.rulings_mode() == LoadMode::Never || self.detail_rect(&self.areas()).is_none() {
            return false;
        }
        let Some(card) = self.selection().map(|s| s.card.clone()) else {
            return false;
        };
        let key = rulings_key(&card);
        let known = self.rulings_known(&key);
        let Some(fetch) = self.rulings_schedule.fetch_now((&key, card.id()), known) else {
            return false;
        };
        self.rulings.remove(&key);
        self.fetch_rulings(fetch.key, fetch.card_id);
        true
    }

    /// Prepares the rulings of `card`, the card the detail shows, or of no card, as
    /// `detail.rulings` asks. Rulings already known are not fetched again, except after a
    /// failure when the card is shown anew.
    pub(super) fn schedule_rulings(&mut self, card: Option<&Card>) {
        let key = card.map(rulings_key);
        let known = key
            .as_ref()
            .map_or(Known::Missing, |k| self.rulings_known(k));
        let settings = &self.config.detail.rulings;
        let plan = self.rulings_schedule.schedule(
            key.as_deref().zip(card.map(Card::id)),
            known,
            settings.mode_in(self.main_view == MainView::FullDetail),
            settings.pause(),
        );
        if let (true, Some(key)) = (plan.forget_failure, &key) {
            self.rulings.remove(key);
        }
        if let Some(fetch) = plan.fetch {
            self.fetch_rulings(fetch.key, fetch.card_id);
        }
    }

    /// Returns what is kept under rulings key `key`.
    fn rulings_known(&self, key: &str) -> Known {
        match self.rulings.get(key) {
            None => Known::Missing,
            Some(RulingsState::Failed(_)) => Known::Failed,
            Some(_) => Known::Kept,
        }
    }

    /// Returns the mode of `detail.rulings` in the view the detail is shown in.
    fn rulings_mode(&self) -> LoadMode {
        let full_detail = self.main_view == MainView::FullDetail;
        self.config.detail.rulings.mode_in(full_detail)
    }

    /// Starts the waiting fetch once its time has come.
    pub(super) fn start_due_rulings(&mut self, now: Instant) {
        if let Some(fetch) = self.rulings_schedule.take_due(now) {
            self.fetch_rulings(fetch.key, fetch.card_id);
        }
    }

    /// Fetches the rulings of the card with printing id `card_id` in the background, keeping
    /// them under `key`. A rate-limited request is retried up to [`MAX_RATE_LIMIT_RETRIES`]
    /// times; the source's request gateway does the waiting.
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
