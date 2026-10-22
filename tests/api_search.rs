//! Tests of the Scryfall API search against the fake.

mod support;

use std::time::Duration;

use scry::scryfall::client::{RateLimits, ScryfallClient};
use scry::search::CardSearch;
use scry::search::api::ApiSearch;
use support::fake_scryfall::{FakeScryfall, NOT_FOUND_DETAILS, card_named};

const NO_LIMITS: RateLimits = RateLimits {
    search: Duration::ZERO,
    other: Duration::ZERO,
    rate_limited_pause: Duration::ZERO,
};

fn dragons(count: usize) -> Vec<serde_json::Value> {
    (0..count)
        .map(|i| card_named(&format!("Dragon {i:03}")))
        .collect()
}

async fn search(server: &wiremock::MockServer) -> ApiSearch {
    ApiSearch::new(ScryfallClient::with_base_url(server.uri().parse().unwrap(), NO_LIMITS).unwrap())
}

#[tokio::test]
async fn first_page_holds_the_configured_page_and_the_total() {
    let mut cards = dragons(12);
    cards.push(card_named("Sol Ring"));
    let (server, fake) = FakeScryfall::start(cards).await;
    fake.set_page_size(5);
    let page = search(&server).await.first_page("dragon").await.unwrap();
    let names: Vec<String> = page.cards.iter().map(|c| c.display_name()).collect();
    assert_eq!(
        names,
        [
            "Dragon 000",
            "Dragon 001",
            "Dragon 002",
            "Dragon 003",
            "Dragon 004"
        ]
    );
    assert_eq!(page.total_cards, Some(12));
    assert!(page.next.is_some());
    assert!(page.warnings.is_empty());
    assert_eq!(page.notice, None);
}

#[tokio::test]
async fn warnings_are_kept_verbatim() {
    let (server, _fake) = FakeScryfall::start(dragons(2)).await;
    let page = search(&server)
        .await
        .first_page("dragon c:r")
        .await
        .unwrap();
    assert_eq!(page.cards.len(), 2);
    assert_eq!(page.next, None);
    assert_eq!(page.warnings, ["Invalid expression “c:r” was ignored."]);
}

#[tokio::test]
async fn no_match_is_an_empty_page_with_scryfalls_message() {
    let (server, _fake) = FakeScryfall::start(dragons(2)).await;
    let page = search(&server).await.first_page("goblin").await.unwrap();
    assert!(page.cards.is_empty());
    assert_eq!(page.total_cards, Some(0));
    assert_eq!(page.next, None);
    assert_eq!(page.notice.as_deref(), Some(NOT_FOUND_DETAILS));
}

#[tokio::test]
async fn next_page_follows_the_token() {
    let (server, fake) = FakeScryfall::start(dragons(7)).await;
    fake.set_page_size(5);
    let search = search(&server).await;
    let first = search.first_page("dragon").await.unwrap();
    let second = search
        .next_page(first.next.as_ref().unwrap())
        .await
        .unwrap();
    assert_eq!(second.cards.len(), 2);
    assert_eq!(second.cards[0].display_name(), "Dragon 005");
    assert_eq!(second.next, None);
}
