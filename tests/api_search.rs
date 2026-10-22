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

mod batches {
    use super::*;
    use scry::search::{BatchEvent, BatchStart, load_batch};
    use std::num::NonZeroU32;

    fn pages(n: u32) -> NonZeroU32 {
        NonZeroU32::new(n).unwrap()
    }

    async fn run_batch(
        search: &ApiSearch,
        start: BatchStart,
        max_pages: u32,
    ) -> (Vec<String>, Option<scry::domain::search_results::PageToken>) {
        let mut log = Vec::new();
        let next = load_batch(search, start, pages(max_pages), |event| {
            log.push(match event {
                BatchEvent::Page(page) => format!(
                    "page {}",
                    page.cards
                        .iter()
                        .map(|c| c.display_name())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                BatchEvent::RateLimited(_) => "rate limited".to_owned(),
                BatchEvent::Failed(err) => format!("failed: {err}"),
            });
        })
        .await;
        (log, next)
    }

    #[tokio::test]
    async fn batches_deliver_every_card_once_in_order() {
        let (server, fake) = FakeScryfall::start(dragons(12)).await;
        fake.set_page_size(5);
        let search = search(&server).await;
        let (first, next) = run_batch(&search, BatchStart::Query("dragon".into()), 2).await;
        assert_eq!(first.len(), 2);
        assert_eq!(
            fake.request_count(),
            2,
            "a batch is exactly max_pages pages"
        );
        let (second, after) = run_batch(&search, BatchStart::Next(next.unwrap()), 2).await;
        assert_eq!(second, ["page Dragon 010,Dragon 011"]);
        assert_eq!(after, None);
        let all: Vec<String> = first
            .iter()
            .chain(&second)
            .flat_map(|p| {
                p.trim_start_matches("page ")
                    .split(',')
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        let expected: Vec<String> = (0..12).map(|i| format!("Dragon {i:03}")).collect();
        assert_eq!(all, expected);
    }

    #[tokio::test]
    async fn loading_stops_at_the_last_page() {
        let (server, fake) = FakeScryfall::start(dragons(12)).await;
        fake.set_page_size(5);
        let (log, next) = run_batch(
            &search(&server).await,
            BatchStart::Query("dragon".into()),
            6,
        )
        .await;
        assert_eq!(log.len(), 3);
        assert_eq!(next, None);
        assert_eq!(fake.request_count(), 3);
    }

    #[tokio::test]
    async fn rate_limited_pages_are_retried_after_the_pause() {
        let (server, fake) = FakeScryfall::start(dragons(4)).await;
        fake.set_page_size(2);
        fake.set_min_interval(Duration::from_millis(150));
        let limits = RateLimits {
            rate_limited_pause: Duration::from_millis(200),
            ..NO_LIMITS
        };
        let search = ApiSearch::new(
            ScryfallClient::with_base_url(server.uri().parse().unwrap(), limits).unwrap(),
        );
        let (log, _) = run_batch(&search, BatchStart::Query("dragon".into()), 2).await;
        assert_eq!(
            log,
            [
                "page Dragon 000,Dragon 001",
                "rate limited",
                "page Dragon 002,Dragon 003"
            ]
        );
    }

    #[tokio::test]
    async fn a_failure_stops_the_batch() {
        let (server, fake) = FakeScryfall::start(dragons(6)).await;
        fake.set_page_size(2);
        fake.fail_on_page(2);
        let (log, next) = run_batch(
            &search(&server).await,
            BatchStart::Query("dragon".into()),
            3,
        )
        .await;
        assert_eq!(log.len(), 2);
        assert_eq!(log[1], "failed: Something went wrong on our end.");
        assert_eq!(next, None);
    }
}
