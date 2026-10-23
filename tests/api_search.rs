//! Tests of the Scryfall API search against the fake.

mod support;

use std::time::Duration;

use scry::scryfall::client::{RateLimits, ScryfallClient};
use scry::search::api::ApiSearch;
use scry::search::{CardImages, CardSearch, SearchError, SearchRequest, SortField, SortOrder};
use support::fake_scryfall::{
    CARD_IMAGE_FIXTURE, FakeScryfall, NOT_FOUND_DETAILS, card_named, card_released, fixture,
};

const NO_LIMITS: RateLimits = RateLimits {
    search: Duration::ZERO,
    other: Duration::ZERO,
    images: Duration::ZERO,
    rate_limited_pause: Duration::ZERO,
};

fn by_name(query: &str) -> SearchRequest {
    SearchRequest::new(query, SortOrder::default())
}

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
    let page = search(&server)
        .await
        .first_page(&by_name("dragon"))
        .await
        .unwrap();
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
async fn the_order_is_sent_to_scryfall() {
    let cards = vec![
        card_released("Young Dragon", "2026-10-19"),
        card_released("Ancient Dragon", "2001-01-01"),
    ];
    let (server, fake) = FakeScryfall::start(cards).await;
    let search = search(&server).await;
    let names = |page: scry::domain::search_results::SearchPage| -> Vec<String> {
        page.cards.iter().map(|c| c.display_name()).collect()
    };
    let mut sorted = Vec::new();
    for (field, reversed) in [
        (SortField::Name, false),
        (SortField::Name, true),
        (SortField::Released, false),
        (SortField::Released, true),
    ] {
        let request = SearchRequest::new("dragon", SortOrder::new(field, reversed));
        sorted.push(names(search.first_page(&request).await.unwrap()));
    }
    assert_eq!(
        sorted,
        [
            ["Ancient Dragon", "Young Dragon"],
            ["Young Dragon", "Ancient Dragon"],
            ["Young Dragon", "Ancient Dragon"],
            ["Ancient Dragon", "Young Dragon"],
        ]
    );
    let paths: Vec<String> = fake
        .received()
        .into_iter()
        .map(|r| r.path_and_query)
        .collect();
    assert_eq!(
        paths,
        [
            "/cards/search?q=dragon&order=name&dir=auto",
            "/cards/search?q=dragon&order=name&dir=desc",
            "/cards/search?q=dragon&order=released&dir=auto",
            "/cards/search?q=dragon&order=released&dir=asc",
        ]
    );
}

#[tokio::test]
async fn warnings_are_kept_verbatim() {
    let (server, _fake) = FakeScryfall::start(dragons(2)).await;
    let page = search(&server)
        .await
        .first_page(&by_name("dragon c:r"))
        .await
        .unwrap();
    assert_eq!(page.cards.len(), 2);
    assert_eq!(page.next, None);
    assert_eq!(page.warnings, ["Invalid expression “c:r” was ignored."]);
}

#[tokio::test]
async fn no_match_is_an_empty_page_with_scryfalls_message() {
    let (server, _fake) = FakeScryfall::start(dragons(2)).await;
    let page = search(&server)
        .await
        .first_page(&by_name("goblin"))
        .await
        .unwrap();
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
    let first = search.first_page(&by_name("dragon")).await.unwrap();
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
        let (first, next) = run_batch(&search, BatchStart::Query(by_name("dragon")), 2).await;
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
            BatchStart::Query(by_name("dragon")),
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
        let (log, _) = run_batch(&search, BatchStart::Query(by_name("dragon")), 2).await;
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
            BatchStart::Query(by_name("dragon")),
            3,
        )
        .await;
        assert_eq!(log.len(), 2);
        assert_eq!(log[1], "failed: Something went wrong on our end.");
        assert_eq!(next, None);
    }
}

#[tokio::test]
async fn found_cards_carry_their_image_which_the_search_fetches() {
    let (server, fake) = FakeScryfall::start(vec![
        card_named("Glorybringer"),
        fixture("transform_delver"),
    ])
    .await;
    let search = search(&server).await;
    let page = search.first_page(&by_name("")).await.unwrap();
    let urls: Vec<&str> = page.cards.iter().filter_map(|c| c.image_url()).collect();
    let delver_id = page.cards[0].id().to_owned();
    assert_eq!(
        urls,
        [
            fake.image_url(&delver_id),
            fake.image_url("id-glorybringer")
        ],
        "the front face's image on a transform card"
    );
    let bytes = search.image(urls[1]).await.unwrap();
    assert_eq!(bytes, std::fs::read(CARD_IMAGE_FIXTURE).unwrap());
    let err = search.image("not a url").await.unwrap_err();
    assert!(matches!(err, SearchError::InvalidImageUrl(_)), "{err}");
}
