//! Spaces request starts per bucket and pauses all buckets after a rate-limit response.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

/// Enforces a minimum interval between request starts in each named bucket. Waiting callers
/// reserve their start time while holding the lock, so concurrent callers are spaced as well.
#[derive(Debug, Default)]
pub struct Throttle {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    next_start: HashMap<&'static str, Instant>,
    paused_until: Option<Instant>,
}

impl Throttle {
    /// Creates a throttle with no history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Waits until a request in `bucket` may start, then records the start. A request may
    /// start `interval` after the previous start in the same bucket, and not before the end of
    /// a [`pause`](Self::pause).
    ///
    /// # Panics
    /// Panics if another thread panicked while holding the throttle's lock.
    pub async fn acquire(&self, bucket: &'static str, interval: Duration) {
        let start = {
            let mut state = self.state.lock().expect("throttle lock poisoned");
            let now = Instant::now();
            let mut start = now;
            if let Some(next) = state.next_start.get(bucket) {
                start = start.max(*next);
            }
            if let Some(paused_until) = state.paused_until {
                start = start.max(paused_until);
            }
            state.next_start.insert(bucket, start + interval);
            start
        };
        tokio::time::sleep_until(start).await;
    }

    /// Delays every bucket so that no request starts within `duration` from now.
    ///
    /// # Panics
    /// Panics if another thread panicked while holding the throttle's lock.
    pub fn pause(&self, duration: Duration) {
        let mut state = self.state.lock().expect("throttle lock poisoned");
        let until = Instant::now() + duration;
        state.paused_until = Some(state.paused_until.map_or(until, |p| p.max(until)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOW: Duration = Duration::from_millis(500);
    const FAST: Duration = Duration::from_millis(100);

    #[tokio::test(start_paused = true)]
    async fn starts_in_a_bucket_are_spaced_by_its_interval() {
        let throttle = Throttle::new();
        let origin = Instant::now();
        let mut slow = Vec::new();
        for _ in 0..3 {
            throttle.acquire("search", SLOW).await;
            slow.push(Instant::now() - origin);
        }
        assert_eq!(slow, [Duration::ZERO, SLOW, SLOW * 2]);

        let origin = Instant::now();
        let mut fast = Vec::new();
        for _ in 0..3 {
            throttle.acquire("other", FAST).await;
            fast.push(Instant::now() - origin);
        }
        assert_eq!(fast, [Duration::ZERO, FAST, FAST * 2]);
    }

    #[tokio::test(start_paused = true)]
    async fn buckets_do_not_delay_each_other() {
        let throttle = Throttle::new();
        throttle.acquire("search", SLOW).await;
        let before = Instant::now();
        throttle.acquire("other", FAST).await;
        assert_eq!(Instant::now(), before);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_callers_are_spaced() {
        let throttle = std::sync::Arc::new(Throttle::new());
        let origin = Instant::now();
        let tasks: Vec<_> = (0..3)
            .map(|_| {
                let throttle = throttle.clone();
                tokio::spawn(async move {
                    throttle.acquire("search", SLOW).await;
                    Instant::now() - origin
                })
            })
            .collect();
        let mut starts = Vec::new();
        for task in tasks {
            starts.push(task.await.unwrap());
        }
        starts.sort();
        assert_eq!(starts, [Duration::ZERO, SLOW, SLOW * 2]);
    }

    #[tokio::test(start_paused = true)]
    async fn pause_delays_every_bucket() {
        let throttle = Throttle::new();
        throttle.acquire("search", SLOW).await;
        let paused_at = Instant::now();
        throttle.pause(Duration::from_secs(30));
        throttle.acquire("other", FAST).await;
        assert_eq!(Instant::now() - paused_at, Duration::from_secs(30));
        throttle.acquire("search", SLOW).await;
        assert_eq!(Instant::now() - paused_at, Duration::from_secs(30));
    }
}
