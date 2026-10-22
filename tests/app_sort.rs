//! App-level tests of sorting the results, against the fake Scryfall.

mod support;

use scry::search::SortOrder;
use scry::tui::keymap::mode::Mode;
use support::fake_scryfall::card_released;
use support::harness::Harness;

fn dragons() -> Vec<serde_json::Value> {
    vec![
        card_released("Old Dragon", "2001-01-01"),
        card_released("New Dragon", "2026-10-19"),
        card_released("Mid Dragon", "2015-05-05"),
    ]
}

fn last_request(h: &Harness) -> String {
    h.fake.received().last().unwrap().path_and_query.clone()
}

#[tokio::test]
async fn s_on_the_results_opens_the_sort_picker_on_the_current_order() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press("s");
    let picker = h.app.picker().expect("the sort picker is open");
    assert_eq!(
        picker.labels(),
        ["Name (A → Z)", "Release date (newest first)"]
    );
    assert_eq!(picker.selected_label(), Some("Name (A → Z)"));
    assert_eq!(h.app.mode(), Mode::Insert);
    h.press("<Esc>");
    assert!(h.app.picker().is_none());
    assert_eq!(
        h.app.sort_order(),
        SortOrder::Name,
        "cancelling changes nothing"
    );
}

#[tokio::test]
async fn s_outside_the_results_does_nothing() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press("2s");
    assert!(h.app.picker().is_none());
}

#[tokio::test]
async fn release_date_order_asks_scryfall_and_shows_newest_first() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    assert_eq!(h.result_names(), ["Mid Dragon", "New Dragon", "Old Dragon"]);
    assert!(
        last_request(&h).ends_with("order=name"),
        "{}",
        last_request(&h)
    );
    h.press("s<Down><CR>");
    h.settle().await;
    assert_eq!(h.fake.request_count(), 2);
    assert!(
        last_request(&h).ends_with("order=released"),
        "{}",
        last_request(&h)
    );
    assert_eq!(h.result_names(), ["New Dragon", "Mid Dragon", "Old Dragon"]);
    assert!(
        h.status_line().contains("by release date"),
        "{}",
        h.status_line()
    );
    assert_eq!(h.app.mode(), Mode::Normal);
}

#[tokio::test]
async fn switching_back_comes_from_the_cache() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press("s<Down><CR>");
    h.settle().await;
    h.press("s<Up><CR>");
    h.settle().await;
    assert_eq!(h.fake.request_count(), 2, "name order is cached");
    assert_eq!(h.result_names(), ["Mid Dragon", "New Dragon", "Old Dragon"]);
    assert!(h.status_line().contains("by name"), "{}", h.status_line());
}

#[tokio::test]
async fn the_order_stays_for_later_searches_and_history_keeps_queries_only() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press("s<Down><CR>");
    h.settle().await;
    h.search("old").await;
    assert!(
        last_request(&h).ends_with("order=released"),
        "{}",
        last_request(&h)
    );
    let queries: Vec<&str> = h
        .app
        .history()
        .entries()
        .iter()
        .map(|e| e.query.as_str())
        .collect();
    assert_eq!(queries, ["old", "dragon"]);
}

#[tokio::test]
async fn renders_sort_picker() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press("s");
    insta::assert_snapshot!(h.render());
}
