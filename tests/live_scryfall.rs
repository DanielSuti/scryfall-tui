//! Tests against the live Scryfall API. They are ignored by default; run them with
//! `cargo test -- --ignored`.

use scry::domain::type_line::SUPERTYPES;
use scry::scryfall::client::USER_AGENT;

/// Each test has its own client and throttle, so the tests take turns: together they would
/// exceed Scryfall's 2 searches per second and get rate-limited.
static LIVE_API: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
#[ignore = "calls the live Scryfall API"]
async fn supertypes_match_scryfall() {
    let _turn = LIVE_API.lock().await;
    let catalog: serde_json::Value = reqwest::Client::new()
        .get("https://api.scryfall.com/catalog/supertypes")
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut live: Vec<String> = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    live.sort();
    let mut ours: Vec<String> = SUPERTYPES.iter().map(|s| (*s).to_owned()).collect();
    ours.sort();
    assert_eq!(ours, live);
}

#[tokio::test]
#[ignore = "calls the live Scryfall API"]
async fn a_real_search_returns_parsed_cards() {
    let _turn = LIVE_API.lock().await;
    use scry::scryfall::client::ScryfallClient;
    use scry::search::api::ApiSearch;
    use scry::search::{CardSearch, SearchRequest, SortOrder};

    let search = ApiSearch::new(ScryfallClient::new().unwrap());
    let page = search
        .first_page(&SearchRequest::new(
            "!\"Glorybringer\"",
            SortOrder::default(),
        ))
        .await
        .unwrap();
    assert_eq!(page.cards.len(), 1);
    let card = &page.cards[0];
    assert_eq!(card.display_name(), "Glorybringer");
    assert_eq!(card.display_cost(), "{3}{R}{R}");
    assert_eq!(card.display_types(), "Creature");
    let none = search
        .first_page(&SearchRequest::new("zzzqqqnotacard", SortOrder::default()))
        .await
        .unwrap();
    assert!(none.cards.is_empty() && none.notice.is_some());
}

#[tokio::test]
#[ignore = "calls the live Scryfall API"]
async fn every_order_reverses() {
    let _turn = LIVE_API.lock().await;
    use scry::scryfall::client::ScryfallClient;
    use scry::search::api::ApiSearch;
    use scry::search::{CardSearch, SearchRequest, SortField, SortOrder};

    let search = ApiSearch::new(ScryfallClient::new().unwrap());
    for field in SortField::ALL {
        let mut firsts = Vec::new();
        for reversed in [false, true] {
            let request = SearchRequest::new("t:dragon", SortOrder::new(field, reversed));
            let page = search.first_page(&request).await.unwrap();
            let ids: Vec<String> = page
                .cards
                .iter()
                .take(5)
                .map(|c| c.id().to_owned())
                .collect();
            firsts.push(ids);
        }
        assert_ne!(
            firsts[0], firsts[1],
            "{field:?}: reversing changes the first cards"
        );
    }
}
