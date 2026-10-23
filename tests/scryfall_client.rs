//! Tests of the Scryfall HTTP client against the fake.

mod support;

use std::time::{Duration, Instant};

use scry::scryfall::client::{RateLimits, ScryfallClient, USER_AGENT};
use scry::scryfall::error::ScryfallError;
use support::fake_scryfall::{FakeScryfall, NOT_FOUND_DETAILS, card_named};

fn unthrottled(pause: Duration) -> RateLimits {
    RateLimits {
        search: Duration::ZERO,
        other: Duration::ZERO,
        rate_limited_pause: pause,
    }
}

async fn client_for(server: &wiremock::MockServer, limits: RateLimits) -> ScryfallClient {
    ScryfallClient::with_base_url(server.uri().parse().unwrap(), limits).unwrap()
}

#[tokio::test]
async fn every_request_sends_user_agent_and_accept() {
    let (server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let client = client_for(&server, unthrottled(Duration::ZERO)).await;
    for _ in 0..2 {
        client
            .get_card_list(client.search_url("glory", "name", "auto"))
            .await
            .unwrap();
    }
    let received = fake.received();
    assert_eq!(received.len(), 2);
    for request in received {
        assert_eq!(request.user_agent.as_deref(), Some(USER_AGENT));
        assert_eq!(request.accept.as_deref(), Some("application/json"));
    }
}

#[tokio::test]
async fn query_is_sent_unchanged() {
    let (server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let client = client_for(&server, unthrottled(Duration::ZERO)).await;
    let list = client
        .get_card_list(client.search_url("glory t:dragon", "name", "auto"))
        .await
        .unwrap();
    assert_eq!(list.data.len(), 1);
    assert_eq!(
        fake.received()[0].path_and_query,
        "/cards/search?q=glory+t%3Adragon&order=name&dir=auto"
    );
    assert_eq!(
        list.warnings.unwrap(),
        ["Invalid expression “t:dragon” was ignored."]
    );
}

#[tokio::test]
async fn error_details_surface_verbatim() {
    let (server, _fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let client = client_for(&server, unthrottled(Duration::ZERO)).await;
    let err = client
        .get_card_list(client.search_url("zzz", "name", "auto"))
        .await
        .unwrap_err();
    match &err {
        ScryfallError::Api { status, code, .. } => {
            assert_eq!((*status, code.as_str()), (404, "not_found"));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(err.to_string(), NOT_FOUND_DETAILS);
}

#[tokio::test]
async fn rate_limit_response_pauses_later_requests() {
    let pause = Duration::from_millis(300);
    let (server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    fake.set_min_interval(Duration::from_millis(200));
    let client = client_for(&server, unthrottled(pause)).await;
    let url = client.search_url("glory", "name", "auto");
    client.get_card_list(url.clone()).await.unwrap();
    let err = client.get_card_list(url.clone()).await.unwrap_err();
    assert!(
        matches!(err, ScryfallError::RateLimited { retry_after } if retry_after == pause),
        "{err:?}"
    );
    let before = Instant::now();
    client.get_card_list(url).await.unwrap();
    // Without the pause the retry would start at once, and the fake would refuse it.
    assert!(
        before.elapsed() >= Duration::from_millis(250),
        "{:?}",
        before.elapsed()
    );
    assert_eq!(fake.request_count(), 3);
}

#[tokio::test]
async fn network_failure_is_a_transport_error() {
    let client = ScryfallClient::with_base_url(
        "http://127.0.0.1:9".parse().unwrap(),
        unthrottled(Duration::ZERO),
    )
    .unwrap();
    let err = client
        .get_card_list(client.search_url("x", "name", "auto"))
        .await
        .unwrap_err();
    assert!(matches!(err, ScryfallError::Transport(_)), "{err:?}");
}
