//! The request gateway: one actor per session that sends every request to Scryfall, so `scry`
//! keeps to Scryfall's rate limits whatever asks for data. See
//! <https://scryfall.com/docs/api/rate-limits>.
//!
//! The actor owns the only HTTP client. Callers hold a [`Gateway`] handle and ask it for a
//! URL; the actor sorts the URL into Scryfall's limits, queues it, and starts it once enough
//! time has passed since the previous request under the same limits. It refuses hosts that are
//! not Scryfall's, pauses every request after a 429, pausing longer each time 429s follow one
//! another, and drops requests whose caller stopped waiting before their turn.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use reqwest::header::{ACCEPT, RETRY_AFTER};
use reqwest::{StatusCode, Url};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::net::schedule::Schedule;
use crate::scryfall::error::ScryfallError;

/// The API's base URL.
pub const BASE_URL: &str = "https://api.scryfall.com";
/// The User-Agent Scryfall requires: the application name and version.
pub const USER_AGENT: &str = concat!("scry/", env!("CARGO_PKG_VERSION"));
/// How long one request may take before it fails.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Hosts under this domain serve Scryfall's files: card images, symbols and bulk data.
pub const FILES_DOMAIN: &str = ".scryfall.io";
/// After this many 429s in a row the pause stops growing: it is at most 8 times the first.
const MAX_PAUSE_DOUBLINGS: u32 = 3;

/// Paths limited to 2 requests per second.
const SEARCH_PATHS: &[&str] = &[
    "/cards/search",
    "/cards/named",
    "/cards/random",
    "/cards/collection",
];
/// The path limited to 10 requests per minute.
const MANIFEST_PATH: &str = "/cards/manifest";

/// Minimum intervals between request starts under each of Scryfall's limits, and the pause
/// after a 429.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimits {
    /// Between requests to the search, named, random and collection endpoints.
    pub search: Duration,
    /// Between requests to the manifest endpoint.
    pub manifest: Duration,
    /// Between any two requests to the API, whatever their endpoint.
    pub other: Duration,
    /// Between requests to Scryfall's file servers, such as card images.
    pub files: Duration,
    /// After a 429 without a `Retry-After` header. Each further 429 before the API answers
    /// anything else doubles the pause, up to 8 times this.
    pub rate_limited_pause: Duration,
}

impl RateLimits {
    /// Scryfall's documented limits: 2 requests per second for the search-like endpoints, 10
    /// per minute for the manifest, 10 per second for the rest, and access limited for 30
    /// seconds after a 429. Every API request also counts against the 10 per second, so the
    /// API never sees more in total. The file servers have no limit; their requests are
    /// spaced as the API's are.
    pub const SCRYFALL: RateLimits = RateLimits {
        search: Duration::from_millis(500),
        manifest: Duration::from_secs(6),
        other: Duration::from_millis(100),
        files: Duration::from_millis(100),
        rate_limited_pause: Duration::from_secs(30),
    };
}

/// The limits a request falls under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    /// The search, named, random and collection endpoints.
    Search,
    /// The manifest endpoint.
    Manifest,
    /// Every API request.
    Api,
    /// The file servers.
    Files,
}

/// Where requests may go: the API's origin, and the file servers'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    api: String,
    files: Files,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Files {
    /// Any `https` host under [`FILES_DOMAIN`].
    Scryfall,
    /// This one origin.
    Origin(String),
}

impl Policy {
    /// Scryfall itself: the API at [`BASE_URL`] and the file servers under [`FILES_DOMAIN`].
    ///
    /// # Panics
    /// Panics if [`BASE_URL`] is not a valid URL, which is a programming error.
    pub fn scryfall() -> Self {
        Self {
            api: origin(&Url::parse(BASE_URL).expect("BASE_URL is valid")),
            files: Files::Scryfall,
        }
    }

    /// A stand-in for Scryfall, such as a test server: the API at `api` and the files at
    /// `files`, each a URL whose origin is used.
    pub fn local(api: &Url, files: &Url) -> Self {
        Self {
            api: origin(api),
            files: Files::Origin(origin(files)),
        }
    }

    /// Returns the buckets a request to `url` counts in, or `None` when `url` is on neither
    /// the API nor the file servers, and must not be sent.
    pub fn buckets(&self, url: &Url) -> Option<&'static [Bucket]> {
        let url_origin = origin(url);
        if url_origin == self.api {
            let path = url.path();
            return Some(if SEARCH_PATHS.contains(&path) {
                &[Bucket::Search, Bucket::Api]
            } else if path == MANIFEST_PATH {
                &[Bucket::Manifest, Bucket::Api]
            } else {
                &[Bucket::Api]
            });
        }
        let is_files = match &self.files {
            Files::Scryfall => {
                url.scheme() == "https"
                    && url
                        .host_str()
                        .is_some_and(|host| host.ends_with(FILES_DOMAIN))
            }
            Files::Origin(files) => url_origin == *files,
        };
        is_files.then_some(&[Bucket::Files])
    }
}

fn origin(url: &Url) -> String {
    url.origin().ascii_serialization()
}

/// A handle on the request gateway. Clones share one actor.
#[derive(Debug, Clone)]
pub struct Gateway {
    requests: mpsc::UnboundedSender<Message>,
}

/// The gateway shared by everything that talks to the real Scryfall.
static LIVE: Mutex<Option<Gateway>> = Mutex::new(None);

impl Gateway {
    /// Returns the gateway to the real Scryfall, starting it on first use. Every call returns
    /// the same actor, so no part of `scry` can send requests past another's rate limits.
    ///
    /// # Errors
    /// Returns [`ScryfallError::Transport`] when the HTTP client cannot be built.
    ///
    /// # Panics
    /// Panics outside a Tokio runtime, or if another thread panicked while starting it.
    pub fn live() -> Result<Self, ScryfallError> {
        let mut live = LIVE.lock().expect("gateway lock poisoned");
        if let Some(gateway) = live.as_ref().filter(|g| !g.requests.is_closed()) {
            return Ok(gateway.clone());
        }
        let gateway = Self::start(Policy::scryfall(), RateLimits::SCRYFALL)?;
        *live = Some(gateway.clone());
        Ok(gateway)
    }

    /// Starts a gateway of its own that sends to the hosts `policy` allows with `limits`, for
    /// tests against a stand-in for Scryfall. Use [`Gateway::live`] for the real one.
    ///
    /// # Errors
    /// Returns [`ScryfallError::Transport`] when the HTTP client cannot be built.
    ///
    /// # Panics
    /// Panics outside a Tokio runtime.
    pub fn start(policy: Policy, limits: RateLimits) -> Result<Self, ScryfallError> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        let (requests, inbox) = mpsc::unbounded_channel();
        let actor = Actor {
            http,
            policy,
            limits,
            schedule: Schedule::new(),
            queues: Default::default(),
            rate_limited_in_a_row: 0,
            inbox,
            outbox: requests.downgrade(),
        };
        tokio::spawn(actor.run());
        Ok(Self { requests })
    }

    /// Sends a GET request for `url` with the Accept header `accept` once Scryfall's limits
    /// allow, and returns the response, whatever its status except 429.
    ///
    /// # Errors
    /// Returns [`ScryfallError::Refused`] for a host that isn't Scryfall's,
    /// [`ScryfallError::RateLimited`] for a 429, [`ScryfallError::Transport`] for network
    /// failures, and [`ScryfallError::GatewayStopped`] when the gateway no longer runs.
    pub async fn get(
        &self,
        url: Url,
        accept: &'static str,
    ) -> Result<reqwest::Response, ScryfallError> {
        let (reply, answer) = oneshot::channel();
        self.requests
            .send(Message::Get { url, accept, reply })
            .map_err(|_| ScryfallError::GatewayStopped)?;
        answer.await.map_err(|_| ScryfallError::GatewayStopped)?
    }

    /// Returns true when `self` and `other` are handles on the same actor.
    pub fn same_as(&self, other: &Gateway) -> bool {
        self.requests.same_channel(&other.requests)
    }
}

type Reply = oneshot::Sender<Result<reqwest::Response, ScryfallError>>;

#[derive(Debug)]
enum Message {
    /// A caller asks for `url`.
    Get {
        url: Url,
        accept: &'static str,
        reply: Reply,
    },
    /// A request got a 429 with this `Retry-After`. The actor pauses, then answers `reply`.
    RateLimited {
        retry_after: Option<Duration>,
        reply: Reply,
    },
    /// A request got another answer, from the API when `api` is true.
    Answered { api: bool },
}

/// A request waiting for its turn.
#[derive(Debug)]
struct Job {
    url: Url,
    accept: &'static str,
    buckets: &'static [Bucket],
    reply: Reply,
}

struct Actor {
    http: reqwest::Client,
    policy: Policy,
    limits: RateLimits,
    schedule: Schedule<Bucket>,
    /// One queue per first bucket, so a search waiting for its interval doesn't hold back an
    /// image.
    queues: [VecDeque<Job>; 4],
    rate_limited_in_a_row: u32,
    inbox: mpsc::UnboundedReceiver<Message>,
    /// For the tasks it starts to report back. Weak, so the actor stops once every
    /// [`Gateway`] and every request is gone.
    outbox: mpsc::WeakUnboundedSender<Message>,
}

impl Actor {
    async fn run(mut self) {
        loop {
            let next = self.next_turn();
            tokio::select! {
                biased;
                message = self.inbox.recv() => match message {
                    Some(message) => self.handle(message),
                    None => return,
                },
                () = sleep_until(next.map(|(_, at)| at)) => {
                    if let Some((queue, _)) = next {
                        self.start_next(queue);
                    }
                }
            }
        }
    }

    fn handle(&mut self, message: Message) {
        match message {
            Message::Get { url, accept, reply } => match self.policy.buckets(&url) {
                Some(buckets) => self.queues[queue_index(buckets[0])].push_back(Job {
                    url,
                    accept,
                    buckets,
                    reply,
                }),
                None => {
                    let host = url.host_str().unwrap_or_default().to_owned();
                    // The caller may have stopped waiting; then nobody needs the answer.
                    let _ = reply.send(Err(ScryfallError::Refused { host }));
                }
            },
            Message::RateLimited { retry_after, reply } => {
                let doublings = self.rate_limited_in_a_row.min(MAX_PAUSE_DOUBLINGS);
                self.rate_limited_in_a_row += 1;
                let pause =
                    retry_after.unwrap_or(self.limits.rate_limited_pause) * (1 << doublings);
                self.schedule.pause(pause, Instant::now());
                let _ = reply.send(Err(ScryfallError::RateLimited { retry_after: pause }));
            }
            // Only the API's answers show that its limit was kept.
            Message::Answered { api: true } => self.rate_limited_in_a_row = 0,
            Message::Answered { api: false } => {}
        }
    }

    /// Returns the queue whose first request may start soonest, and when. Requests whose
    /// caller stopped waiting are dropped unsent.
    fn next_turn(&mut self) -> Option<(usize, Instant)> {
        let now = Instant::now();
        let mut next: Option<(usize, Instant)> = None;
        for (index, queue) in self.queues.iter_mut().enumerate() {
            while queue.front().is_some_and(|job| job.reply.is_closed()) {
                queue.pop_front();
            }
            let Some(job) = queue.front() else { continue };
            let at = self
                .schedule
                .ready_at(&intervals(job.buckets, &self.limits), now);
            if next.is_none_or(|(_, soonest)| at < soonest) {
                next = Some((index, at));
            }
        }
        next
    }

    /// Starts the first request of `queue` and records its start.
    fn start_next(&mut self, queue: usize) {
        let Some(job) = self.queues[queue].pop_front() else {
            return;
        };
        if job.reply.is_closed() {
            return;
        }
        let Some(outbox) = self.outbox.upgrade() else {
            return;
        };
        self.schedule
            .record(&intervals(job.buckets, &self.limits), Instant::now());
        let api = job.buckets.contains(&Bucket::Api);
        let request = self.http.get(job.url).header(ACCEPT, job.accept);
        tokio::spawn(async move {
            let response = match request.send().await {
                Ok(response) => response,
                Err(err) => {
                    let _ = job.reply.send(Err(err.into()));
                    return;
                }
            };
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                let retry_after = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .map(Duration::from_secs);
                let reply = job.reply;
                // A closed inbox means the gateway is gone, and with it every later request.
                let _ = outbox.send(Message::RateLimited { retry_after, reply });
                return;
            }
            let _ = outbox.send(Message::Answered { api });
            let _ = job.reply.send(Ok(response));
        });
    }
}

fn queue_index(bucket: Bucket) -> usize {
    match bucket {
        Bucket::Search => 0,
        Bucket::Manifest => 1,
        Bucket::Api => 2,
        Bucket::Files => 3,
    }
}

/// Returns each of `buckets` with its interval under `limits`.
fn intervals(buckets: &[Bucket], limits: &RateLimits) -> Vec<(Bucket, Duration)> {
    buckets
        .iter()
        .map(|bucket| {
            let interval = match bucket {
                Bucket::Search => limits.search,
                Bucket::Manifest => limits.manifest,
                Bucket::Api => limits.other,
                Bucket::Files => limits.files,
            };
            (*bucket, interval)
        })
        .collect()
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn scryfall_requests_fall_under_their_endpoints_limits() {
        let policy = Policy::scryfall();
        for path in SEARCH_PATHS {
            let request = url(&format!("https://api.scryfall.com{path}?q=x"));
            assert_eq!(
                policy.buckets(&request),
                Some(&[Bucket::Search, Bucket::Api][..])
            );
        }
        assert_eq!(
            policy.buckets(&url("https://api.scryfall.com/cards/manifest")),
            Some(&[Bucket::Manifest, Bucket::Api][..])
        );
        assert_eq!(
            policy.buckets(&url("https://api.scryfall.com/cards/abc/rulings")),
            Some(&[Bucket::Api][..])
        );
        assert_eq!(
            policy.buckets(&url("https://cards.scryfall.io/normal/front/a.jpg")),
            Some(&[Bucket::Files][..])
        );
    }

    #[test]
    fn other_hosts_are_refused() {
        let policy = Policy::scryfall();
        for refused in [
            "https://example.com/cards/search",
            "http://api.scryfall.com/cards/search",
            "https://api.scryfall.com:8443/cards/search",
            "http://cards.scryfall.io/normal/front/a.jpg",
            "https://cards.scryfall.io.example.com/a.jpg",
            "https://scryfall.io/a.jpg",
        ] {
            assert_eq!(policy.buckets(&url(refused)), None, "{refused}");
        }
    }

    #[test]
    fn a_local_policy_allows_only_its_two_origins() {
        let api = url("http://127.0.0.1:4000");
        let files = url("http://127.0.0.1:4001");
        let policy = Policy::local(&api, &files);
        assert_eq!(
            policy.buckets(&url("http://127.0.0.1:4000/cards/search")),
            Some(&[Bucket::Search, Bucket::Api][..])
        );
        assert_eq!(
            policy.buckets(&url("http://127.0.0.1:4001/images/a.png")),
            Some(&[Bucket::Files][..])
        );
        assert_eq!(policy.buckets(&url("http://127.0.0.1:4002/a")), None);
        assert_eq!(
            policy.buckets(&url("https://cards.scryfall.io/a.jpg")),
            None,
            "a stand-in never reaches the real Scryfall"
        );
    }

    #[tokio::test]
    async fn every_call_to_live_returns_the_same_gateway() {
        let first = Gateway::live().unwrap();
        let second = Gateway::live().unwrap();
        assert!(first.same_as(&second));
        let own = Gateway::start(Policy::scryfall(), RateLimits::SCRYFALL).unwrap();
        assert!(!first.same_as(&own));
    }

    #[tokio::test]
    async fn refused_requests_are_never_sent() {
        let gateway = Gateway::start(Policy::scryfall(), RateLimits::SCRYFALL).unwrap();
        let err = gateway
            .get(url("http://127.0.0.1:9/cards/search"), "application/json")
            .await
            .unwrap_err();
        assert!(
            matches!(&err, ScryfallError::Refused { host } if host == "127.0.0.1"),
            "{err:?}"
        );
    }
}
