//! When the detail fetches data beyond the card itself, such as its rulings: at once, after the
//! cursor rests, or not at all, as a [`LoadMode`] asks.

use std::time::Duration;

use tokio::time::Instant;

use crate::app::config::LoadMode;

/// A fetch to start: the data kept under `key`, for the card with printing id `card_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fetch {
    /// The key the data is kept under.
    pub key: String,
    /// The printing id of the card.
    pub card_id: String,
}

/// What the caller holds under the key of the shown card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Known {
    /// Nothing.
    Missing,
    /// The data, or a fetch that is running.
    Kept,
    /// A failed fetch.
    Failed,
}

/// What the caller does after [`LoadSchedule::schedule`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Plan {
    /// Drop the failure kept for the card, because the card is shown anew.
    pub forget_failure: bool,
    /// Start this fetch now.
    pub fetch: Option<Fetch>,
}

/// Which card the detail last showed, and the fetch that waits for the cursor to rest.
#[derive(Debug, Clone, Default)]
pub(super) struct LoadSchedule {
    shown: Option<String>,
    pending: Option<(Fetch, Instant)>,
}

impl LoadSchedule {
    /// Plans the fetch for the card the detail shows, given as its key and printing id, or for
    /// no card, in the view whose mode is `mode`. In `auto` mode a fetch starts once the cursor
    /// has rested for `pause`; in `never` mode it never starts. Nothing is fetched when `known`
    /// is [`Known::Kept`], or when it is [`Known::Failed`] and the card was already shown.
    pub(super) fn schedule(
        &mut self,
        card: Option<(&str, &str)>,
        known: Known,
        mode: LoadMode,
        pause: Duration,
    ) -> Plan {
        let key = card.map(|(key, _)| key.to_owned());
        let shown_anew = key != self.shown;
        self.shown = key;
        let Some((key, card_id)) = card else {
            self.pending = None;
            return Plan::default();
        };
        let forget_failure = shown_anew && known == Known::Failed;
        let missing = forget_failure || known == Known::Missing;
        let mut plan = Plan {
            forget_failure,
            fetch: None,
        };
        let pause = match mode {
            LoadMode::Auto => pause,
            LoadMode::Never => {
                self.pending = None;
                return plan;
            }
        };
        let fetch = Fetch {
            key: key.to_owned(),
            card_id: card_id.to_owned(),
        };
        if !missing {
            self.pending = None;
        } else if pause.is_zero() {
            self.pending = None;
            plan.fetch = Some(fetch);
        } else if !self.is_pending(key) {
            self.pending = Some((fetch, Instant::now() + pause));
        }
        plan
    }

    /// Returns when the waiting fetch starts, or `None` when none waits.
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.pending.as_ref().map(|(_, due)| *due)
    }

    /// Returns true while a fetch for `key` waits for the cursor to rest.
    pub(super) fn is_pending(&self, key: &str) -> bool {
        self.pending.as_ref().is_some_and(|(f, _)| f.key == key)
    }

    /// Returns the waiting fetch once its time has come, and forgets it.
    pub(super) fn take_due(&mut self, now: Instant) -> Option<Fetch> {
        self.pending
            .take_if(|(_, due)| *due <= now)
            .map(|(fetch, _)| fetch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAUSE: Duration = Duration::from_millis(300);
    const A: Option<(&str, &str)> = Some(("oracle-a", "id-a"));
    const B: Option<(&str, &str)> = Some(("oracle-b", "id-b"));

    fn fetch_a() -> Fetch {
        Fetch {
            key: "oracle-a".into(),
            card_id: "id-a".into(),
        }
    }

    #[test]
    fn auto_mode_waits_for_the_pause() {
        let mut s = LoadSchedule::default();
        let plan = s.schedule(A, Known::Missing, LoadMode::Auto, PAUSE);
        assert_eq!(plan, Plan::default());
        assert!(s.is_pending("oracle-a"));
        let due = s.deadline().unwrap();
        assert_eq!(s.take_due(due - Duration::from_millis(1)), None);
        assert_eq!(s.take_due(due), Some(fetch_a()));
        assert_eq!(s.deadline(), None);
    }

    #[test]
    fn a_zero_pause_fetches_at_once() {
        let mut s = LoadSchedule::default();
        let plan = s.schedule(A, Known::Missing, LoadMode::Auto, Duration::ZERO);
        assert_eq!(plan.fetch, Some(fetch_a()));
        assert_eq!(s.deadline(), None);
    }

    #[test]
    fn never_mode_fetches_nothing() {
        let mut s = LoadSchedule::default();
        let plan = s.schedule(A, Known::Missing, LoadMode::Never, Duration::ZERO);
        assert_eq!(plan, Plan::default());
        assert_eq!(s.deadline(), None);
    }

    #[test]
    fn kept_data_is_not_fetched_again() {
        let mut s = LoadSchedule::default();
        let plan = s.schedule(A, Known::Kept, LoadMode::Auto, Duration::ZERO);
        assert_eq!(plan, Plan::default());
    }

    #[test]
    fn a_failure_is_retried_only_when_the_card_is_shown_anew() {
        let mut s = LoadSchedule::default();
        let first = s.schedule(A, Known::Failed, LoadMode::Auto, Duration::ZERO);
        assert!(first.forget_failure);
        assert_eq!(first.fetch, Some(fetch_a()));
        let again = s.schedule(A, Known::Failed, LoadMode::Auto, Duration::ZERO);
        assert_eq!(again, Plan::default(), "still shown: the failure stays");
        s.schedule(B, Known::Missing, LoadMode::Never, PAUSE);
        let back = s.schedule(A, Known::Failed, LoadMode::Never, PAUSE);
        assert!(
            back.forget_failure,
            "forgotten even when nothing is fetched"
        );
        assert_eq!(back.fetch, None);
    }

    #[test]
    fn moving_on_drops_the_waiting_fetch() {
        let mut s = LoadSchedule::default();
        s.schedule(A, Known::Missing, LoadMode::Auto, PAUSE);
        let due = s.deadline().unwrap();
        s.schedule(A, Known::Missing, LoadMode::Auto, PAUSE);
        assert_eq!(s.deadline(), Some(due), "staying keeps the deadline");
        s.schedule(B, Known::Missing, LoadMode::Auto, PAUSE);
        assert!(s.is_pending("oracle-b"));
        assert!(!s.is_pending("oracle-a"));
        s.schedule(None, Known::Missing, LoadMode::Auto, PAUSE);
        assert_eq!(s.deadline(), None);
    }
}
