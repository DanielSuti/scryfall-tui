//! Tests of the Scryfall HTTP client against the fake.

mod support;

use std::time::{Duration, Instant};

use scry::scryfall::client::{ACCEPT_IMAGE, RateLimits, ScryfallClient, USER_AGENT};
use scry::scryfall::error::ScryfallError;
use support::fake_scryfall::{
    CARD_IMAGE_FIXTURE, FakeScryfall, NOT_FOUND_DETAILS, card_named, ruling,
};

fn unlimited(pause: Duration) -> RateLimits {
    RateLimits {
        search: Duration::ZERO,
        manifest: Duration::ZERO,
        other: Duration::ZERO,
        files: Duration::ZERO,
        rate_limited_pause: pause,
    }
}

async fn client_for(fake: &FakeScryfall, limits: RateLimits) -> ScryfallClient {
    fake.client(limits)
}

#[tokio::test]
async fn every_request_sends_user_agent_and_accept() {
    let (_server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let client = client_for(&fake, unlimited(Duration::ZERO)).await;
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
    let (_server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let client = client_for(&fake, unlimited(Duration::ZERO)).await;
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
    let (_server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let client = client_for(&fake, unlimited(Duration::ZERO)).await;
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
    let (_server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    fake.set_min_interval(Duration::from_millis(200));
    let client = client_for(&fake, unlimited(pause)).await;
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
    let client = FakeScryfall::client_at("http://127.0.0.1:9", unlimited(Duration::ZERO));
    let err = client
        .get_card_list(client.search_url("x", "name", "auto"))
        .await
        .unwrap_err();
    assert!(matches!(err, ScryfallError::Transport(_)), "{err:?}");
}

#[tokio::test]
async fn rulings_are_fetched_by_card_id_with_the_headers() {
    let (_server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    fake.set_rulings(
        "id-glorybringer",
        vec![ruling(
            "2017-04-18",
            "You can’t exert a creature unless an effect allows it.",
        )],
    );
    let client = client_for(&fake, unlimited(Duration::ZERO)).await;
    let list = client
        .get_rulings(client.rulings_url("id-glorybringer"))
        .await
        .unwrap();
    assert_eq!(list.data.len(), 1);
    assert_eq!(list.data[0].published_at, "2017-04-18");
    let received = fake.rulings_received();
    assert_eq!(
        received[0].path_and_query,
        "/cards/id-glorybringer/rulings?"
    );
    assert_eq!(received[0].user_agent.as_deref(), Some(USER_AGENT));
    assert_eq!(received[0].accept.as_deref(), Some("application/json"));
    let err = client
        .get_rulings(client.rulings_url("no-such-id"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ScryfallError::Api { status: 404, .. }),
        "{err}"
    );
}

#[tokio::test]
async fn images_are_fetched_with_the_headers() {
    let (_server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let client = client_for(&fake, unlimited(Duration::ZERO)).await;
    let url = fake.image_url("id-glorybringer").parse().unwrap();
    let bytes = client.get_image(url).await.unwrap();
    assert_eq!(bytes, std::fs::read(CARD_IMAGE_FIXTURE).unwrap());
    let received = fake.images_received();
    assert_eq!(received[0].path_and_query, "/images/id-glorybringer.png?");
    assert_eq!(received[0].user_agent.as_deref(), Some(USER_AGENT));
    assert_eq!(received[0].accept.as_deref(), Some(ACCEPT_IMAGE));
}

#[tokio::test]
async fn a_failed_image_is_an_error_and_a_429_pauses_later_requests() {
    let pause = Duration::from_millis(300);
    let (_server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let client = client_for(&fake, unlimited(pause)).await;
    let missing = fake.image_url("no-such-id").parse().unwrap();
    let err = client.get_image(missing).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "unexpected Scryfall response (HTTP 404): not an image"
    );
    fake.fail_image("id-glorybringer", Some(429));
    let url: reqwest::Url = fake.image_url("id-glorybringer").parse().unwrap();
    let err = client.get_image(url.clone()).await.unwrap_err();
    assert!(
        matches!(err, ScryfallError::RateLimited { retry_after } if retry_after == pause),
        "{err:?}"
    );
    fake.fail_image("id-glorybringer", None);
    let before = Instant::now();
    client
        .get_card_list(client.search_url("glory", "name", "auto"))
        .await
        .unwrap();
    assert!(
        before.elapsed() >= Duration::from_millis(250),
        "the pause holds every request: {:?}",
        before.elapsed()
    );
    client.get_image(url).await.unwrap();
}

#[tokio::test]
async fn image_requests_are_spaced_in_their_own_bucket() {
    let (_server, fake) = FakeScryfall::start(vec![card_named("Glorybringer")]).await;
    let limits = RateLimits {
        files: Duration::from_millis(200),
        ..unlimited(Duration::ZERO)
    };
    let client = client_for(&fake, limits).await;
    let url: reqwest::Url = fake.image_url("id-glorybringer").parse().unwrap();
    let before = Instant::now();
    client.get_image(url.clone()).await.unwrap();
    client
        .get_rulings(client.rulings_url("id-glorybringer"))
        .await
        .unwrap();
    assert!(
        before.elapsed() < Duration::from_millis(200),
        "rulings don't wait"
    );
    client.get_image(url).await.unwrap();
    assert!(before.elapsed() >= Duration::from_millis(200));
}
