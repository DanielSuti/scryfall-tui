//! Checks against real Scryfall data, replayed by `DummyScryfall` from recordings, so these
//! tests run offline. Only the ignored test `record_scryfall_answers` calls the live API; run
//! it when the recordings need refreshing (see `support::dummy_scryfall`).

mod support;

use std::time::Duration;

use scry::domain::type_line::SUPERTYPES;
use scry::net::throttle::Throttle;
use scry::scryfall::client::{ACCEPT_JSON, RateLimits, ScryfallClient, USER_AGENT};
use scry::search::api::ApiSearch;
use scry::search::{CardRulings, CardSearch, SearchRequest, SortField, SortOrder};
use support::dummy_scryfall::{
    DummyScryfall, GLORYBRINGER_ID, ORDER_QUERY, RECORDINGS_DIR, Recording, recorded_requests,
    request_key, trim,
};

/// The recorder waits this long between requests: twice Scryfall's interval for searches.
const RECORDING_INTERVAL: Duration = Duration::from_secs(1);

const LOCAL: RateLimits = RateLimits {
    search: Duration::ZERO,
    other: Duration::ZERO,
    rate_limited_pause: Duration::ZERO,
};

async fn search_on(server: &wiremock::MockServer) -> ApiSearch {
    ApiSearch::new(ScryfallClient::with_base_url(server.uri().parse().unwrap(), LOCAL).unwrap())
}

#[tokio::test]
async fn every_recorded_request_has_an_answer() {
    let (_server, dummy) = DummyScryfall::start().await;
    for (name, url) in recorded_requests() {
        let key = request_key(&url);
        dummy.call(&key);
        assert!(
            dummy.unrecorded_calls().is_empty(),
            "{name}: no recording for {key}"
        );
    }
}

#[tokio::test]
async fn supertypes_match_the_recorded_catalog() {
    let (server, dummy) = DummyScryfall::start().await;
    let catalog: serde_json::Value = reqwest::get(format!("{}/catalog/supertypes", server.uri()))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut recorded: Vec<String> = catalog["data"]
        .as_array()
        .expect("a catalog")
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    recorded.sort();
    let mut ours: Vec<String> = SUPERTYPES.iter().map(|s| (*s).to_owned()).collect();
    ours.sort();
    assert_eq!(ours, recorded);
    assert!(dummy.unrecorded_calls().is_empty());
}

#[tokio::test]
async fn a_recorded_search_parses_and_no_match_gives_scryfalls_notice() {
    let (server, dummy) = DummyScryfall::start().await;
    let search = search_on(&server).await;
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
    assert!(
        dummy.unrecorded_calls().is_empty(),
        "{:?}",
        dummy.unrecorded_calls()
    );
}

#[tokio::test]
async fn recorded_rulings_parse() {
    let (server, dummy) = DummyScryfall::start().await;
    let search = search_on(&server).await;
    let rulings = search.rulings(GLORYBRINGER_ID).await.unwrap();
    assert_eq!(rulings.len(), 5);
    assert!(rulings.iter().all(|r| r.published_at == "2017-04-18"));
    assert!(
        rulings[0].comment.starts_with("You can’t exert a creature"),
        "{}",
        rulings[0].comment
    );
    assert!(
        dummy.unrecorded_calls().is_empty(),
        "{:?}",
        dummy.unrecorded_calls()
    );
}

#[tokio::test]
async fn every_order_reverses() {
    let (server, dummy) = DummyScryfall::start().await;
    let search = search_on(&server).await;
    for field in SortField::ALL {
        let mut firsts = Vec::new();
        for reversed in [false, true] {
            let request = SearchRequest::new(ORDER_QUERY, SortOrder::new(field, reversed));
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
    assert!(
        dummy.unrecorded_calls().is_empty(),
        "{:?}",
        dummy.unrecorded_calls()
    );
}

/// Sends every request of `recorded_requests` to the live API, one per
/// `RECORDING_INTERVAL`, and saves the trimmed answers in `RECORDINGS_DIR`, replacing the
/// previous recordings. It stops at the first 429 so as not to add to a rate-limit penalty.
#[tokio::test]
#[ignore = "calls the live Scryfall API to refresh the recordings"]
async fn record_scryfall_answers() {
    let requests = recorded_requests();
    let http = reqwest::Client::new();
    let throttle = Throttle::new();
    let mut recordings = Vec::new();
    for (name, url) in &requests {
        throttle.acquire("recording", RECORDING_INTERVAL).await;
        let response = http
            .get(url.clone())
            .header("User-Agent", USER_AGENT)
            .header("Accept", ACCEPT_JSON)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        assert_ne!(
            status, 429,
            "{name}: rate-limited; wait before recording again"
        );
        let body: serde_json::Value = response.json().await.unwrap();
        recordings.push((
            name.clone(),
            Recording {
                request: request_key(url),
                status,
                body: trim(body),
            },
        ));
    }
    let _ = std::fs::remove_dir_all(RECORDINGS_DIR);
    std::fs::create_dir_all(RECORDINGS_DIR).unwrap();
    for (name, recording) in recordings {
        let text = serde_json::to_string_pretty(&recording).unwrap();
        std::fs::write(format!("{RECORDINGS_DIR}/{name}.json"), text + "\n").unwrap();
    }
}
