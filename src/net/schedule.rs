//! When requests may start: a minimum interval between starts in each bucket, and pauses after
//! rate-limit responses. It only keeps time; the request gateway decides what to send.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::Duration;

use tokio::time::Instant;

/// The start times of requests in buckets of type `B`. A request can count in several buckets,
/// such as its endpoint's and the whole API's, and waits for the interval of each.
#[derive(Debug)]
pub struct Schedule<B> {
    next_start: HashMap<B, Instant>,
    paused_until: Option<Instant>,
}

impl<B> Default for Schedule<B> {
    fn default() -> Self {
        Self {
            next_start: HashMap::new(),
            paused_until: None,
        }
    }
}

impl<B: Copy + Eq + Hash> Schedule<B> {
    /// Creates a schedule with no history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns when a request counted in each of `buckets` may start, `now` at the earliest:
    /// once each bucket's interval has passed since its previous start, and not during a pause.
    pub fn ready_at(&self, buckets: &[(B, Duration)], now: Instant) -> Instant {
        let mut at = now;
        for (bucket, _) in buckets {
            if let Some(next) = self.next_start.get(bucket) {
                at = at.max(*next);
            }
        }
        self.paused_until.map_or(at, |paused| at.max(paused))
    }

    /// Records that a request counted in `buckets` starts at `at`, so the next one in each
    /// bucket waits for that bucket's interval.
    pub fn record(&mut self, buckets: &[(B, Duration)], at: Instant) {
        for (bucket, interval) in buckets {
            let next = self.next_start.entry(*bucket).or_insert(at);
            *next = (*next).max(at + *interval);
        }
    }

    /// Holds every bucket until `duration` after `now`. A pause that ends later is kept.
    pub fn pause(&mut self, duration: Duration, now: Instant) {
        let until = now + duration;
        self.paused_until = Some(self.paused_until.map_or(until, |p| p.max(until)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOW: Duration = Duration::from_millis(500);
    const FAST: Duration = Duration::from_millis(100);

    #[test]
    fn starts_in_a_bucket_are_spaced_by_its_interval() {
        let mut schedule = Schedule::new();
        let origin = Instant::now();
        let search = [("search", SLOW)];
        assert_eq!(schedule.ready_at(&search, origin), origin);
        schedule.record(&search, origin);
        assert_eq!(schedule.ready_at(&search, origin), origin + SLOW);
        assert_eq!(
            schedule.ready_at(&[("other", FAST)], origin),
            origin,
            "buckets don't delay each other"
        );
    }

    #[test]
    fn a_request_waits_for_every_bucket_it_counts_in() {
        let mut schedule = Schedule::new();
        let origin = Instant::now();
        schedule.record(&[("search", SLOW), ("api", FAST)], origin);
        assert_eq!(
            schedule.ready_at(&[("other", FAST), ("api", FAST)], origin),
            origin + FAST,
            "a search counts against the whole API"
        );
        assert_eq!(
            schedule.ready_at(&[("search", SLOW), ("api", FAST)], origin),
            origin + SLOW
        );
    }

    #[test]
    fn a_pause_holds_every_bucket_and_the_longer_one_wins() {
        let mut schedule = Schedule::new();
        let origin = Instant::now();
        schedule.pause(Duration::from_secs(30), origin);
        schedule.pause(Duration::from_secs(5), origin);
        for bucket in ["search", "other"] {
            assert_eq!(
                schedule.ready_at(&[(bucket, FAST)], origin),
                origin + Duration::from_secs(30)
            );
        }
    }
}
