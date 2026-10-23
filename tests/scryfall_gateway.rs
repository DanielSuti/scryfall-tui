//! Tests of the request gateway against local servers standing in for Scryfall's API and file
//! servers.

use std::time::{Duration, Instant};

use reqwest::Url;
use scry::scryfall::client::{ACCEPT_IMAGE, ACCEPT_JSON, USER_AGENT};
use scry::scryfall::error::ScryfallError;
use scry::scryfall::gateway::{Gateway, Policy, RateLimits};
use wiremock::matchers::{any, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const LIMITS: RateLimits = RateLimits {
    search: Duration::from_millis(300),
    manifest: Duration::from_secs(1),
    other: Duration::from_millis(100),
    files: Duration::ZERO,
    rate_limited_pause: Duration::from_millis(100),
};

/// A stand-in for the API, answering 429 on `/cards/search` and `/cards/named` when
/// `rate_limited` is true and 200 otherwise, and one for the file servers.
struct Servers {
    api: MockServer,
    files: MockServer,
}

impl Servers {
    async fn start(rate_limited: bool) -> Self {
        let api = MockServer::start().await;
        if rate_limited {
            Mock::given(path("/cards/search"))
                .respond_with(ResponseTemplate::new(429))
                .mount(&api)
                .await;
            Mock::given(path("/cards/named"))
                .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
                .mount(&api)
                .await;
        }
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .mount(&api)
            .await;
        let files = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .mount(&files)
            .await;
        Self { api, files }
    }

    fn gateway(&self) -> Gateway {
        let policy = Policy::local(&self.api(""), &self.file(""));
        Gateway::start(policy, LIMITS).unwrap()
    }

    fn api(&self, path: &str) -> Url {
        format!("{}{path}", self.api.uri()).parse().unwrap()
    }

    fn file(&self, path: &str) -> Url {
        format!("{}{path}", self.files.uri()).parse().unwrap()
    }
}

/// Sends a GET for `url` and returns how long after `origin` it was answered.
async fn get_at(gateway: &Gateway, url: Url, origin: Instant) -> Duration {
    gateway.get(url, ACCEPT_JSON).await.unwrap();
    origin.elapsed()
}

#[tokio::test]
async fn requests_are_spaced_by_the_limits_they_fall_under() {
    let servers = Servers::start(false).await;
    let gateway = servers.gateway();
    let origin = Instant::now();
    get_at(&gateway, servers.api("/cards/search"), origin).await;
    let ruling = get_at(&gateway, servers.api("/cards/x/rulings"), origin).await;
    assert!(
        ruling >= Duration::from_millis(100) && ruling < Duration::from_millis(300),
        "a ruling waits for the API-wide interval only: {ruling:?}"
    );
    let file = get_at(&gateway, servers.file("/a.jpg"), origin).await;
    assert!(
        file - ruling < Duration::from_millis(80),
        "files have their own limit"
    );
    let search = get_at(&gateway, servers.api("/cards/named"), origin).await;
    assert!(
        search >= Duration::from_millis(300),
        "a second search waits for the search interval: {search:?}"
    );
}

#[tokio::test]
async fn requests_sent_at_once_still_leave_one_at_a_time() {
    let servers = Servers::start(false).await;
    let gateway = servers.gateway();
    let origin = Instant::now();
    let tasks: Vec<_> = (0..4)
        .map(|i| {
            let gateway = gateway.clone();
            let url = servers.api(&format!("/cards/{i}/rulings"));
            tokio::spawn(async move { get_at(&gateway, url, origin).await })
        })
        .collect();
    let mut answered = Vec::new();
    for task in tasks {
        answered.push(task.await.unwrap());
    }
    answered.sort();
    assert!(
        answered[3] >= Duration::from_millis(300),
        "four requests take three intervals: {answered:?}"
    );
}

#[tokio::test]
async fn every_request_carries_the_user_agent_and_its_accept_header() {
    let servers = Servers::start(false).await;
    let gateway = servers.gateway();
    gateway
        .get(servers.api("/cards/x/rulings"), ACCEPT_JSON)
        .await
        .unwrap();
    gateway
        .get(servers.file("/a.jpg"), ACCEPT_IMAGE)
        .await
        .unwrap();
    for (server, accept) in [(&servers.api, ACCEPT_JSON), (&servers.files, ACCEPT_IMAGE)] {
        let received = server.received_requests().await.unwrap();
        let headers = &received[0].headers;
        assert_eq!(headers.get("user-agent").unwrap(), USER_AGENT);
        assert_eq!(headers.get("accept").unwrap(), accept);
    }
}

#[tokio::test]
async fn a_429_pauses_everything_longer_each_time_until_a_success() {
    let servers = Servers::start(true).await;
    let gateway = servers.gateway();
    let pause = LIMITS.rate_limited_pause;
    let rate_limited = |result: Result<reqwest::Response, ScryfallError>| match result {
        Err(ScryfallError::RateLimited { retry_after }) => retry_after,
        other => panic!("expected a 429, got {other:?}"),
    };
    let first = rate_limited(gateway.get(servers.api("/cards/search"), ACCEPT_JSON).await);
    assert_eq!(first, pause);
    let before = Instant::now();
    gateway
        .get(servers.file("/a.jpg"), ACCEPT_IMAGE)
        .await
        .unwrap();
    assert!(before.elapsed() >= pause * 9 / 10, "files wait as well");
    let second = rate_limited(gateway.get(servers.api("/cards/search"), ACCEPT_JSON).await);
    assert_eq!(
        second,
        pause * 2,
        "the second 429 in a row doubles the pause"
    );
    let before = Instant::now();
    gateway
        .get(servers.api("/cards/x/rulings"), ACCEPT_JSON)
        .await
        .unwrap();
    assert!(
        before.elapsed() >= pause * 2 * 9 / 10,
        "{:?}",
        before.elapsed()
    );
    let after_success = rate_limited(gateway.get(servers.api("/cards/search"), ACCEPT_JSON).await);
    assert_eq!(after_success, pause, "a success resets the pause");
}

#[tokio::test]
async fn retry_after_sets_the_pause() {
    let servers = Servers::start(true).await;
    let gateway = servers.gateway();
    let err = gateway
        .get(servers.api("/cards/named"), ACCEPT_JSON)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ScryfallError::RateLimited { retry_after } if retry_after == Duration::from_secs(1)),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_request_nobody_waits_for_any_more_is_not_sent() {
    let servers = Servers::start(false).await;
    let gateway = servers.gateway();
    gateway
        .get(servers.api("/cards/search"), ACCEPT_JSON)
        .await
        .unwrap();
    let abandoned = tokio::time::timeout(
        Duration::from_millis(50),
        gateway.get(servers.api("/cards/search?q=late"), ACCEPT_JSON),
    )
    .await;
    assert!(abandoned.is_err(), "still waiting for its turn");
    tokio::time::sleep(LIMITS.search + Duration::from_millis(100)).await;
    let received = servers.api.received_requests().await.unwrap();
    assert_eq!(received.len(), 1, "{received:?}");
}

#[tokio::test]
async fn other_hosts_are_refused_and_never_reached() {
    let servers = Servers::start(false).await;
    let elsewhere = MockServer::start().await;
    let gateway = servers.gateway();
    let url: Url = format!("{}/cards/search", elsewhere.uri()).parse().unwrap();
    let err = gateway.get(url, ACCEPT_JSON).await.unwrap_err();
    assert!(matches!(err, ScryfallError::Refused { .. }), "{err:?}");
    assert!(elsewhere.received_requests().await.unwrap().is_empty());
}
