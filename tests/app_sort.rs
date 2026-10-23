//! App-level tests of sorting with `,` and a letter, against the fake Scryfall.

mod support;

use scry::search::{SortField, SortOrder};
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
async fn comma_opens_the_sort_window_at_once_with_paired_keys() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press(",");
    assert_eq!(h.app.which_key_title(), Some("Sort by"));
    let entries = h.app.which_key_entries();
    assert_eq!(entries.len(), SortField::ALL.len(), "one line per order");
    let lines: Vec<String> = entries
        .iter()
        .map(|e| format!("{} {}", e.keys, e.description))
        .collect();
    assert_eq!(lines[0], "n/N Name: A → Z / Z → A");
    assert!(lines.contains(&"d/D Release date: newest first / oldest first".to_owned()));
    assert!(lines.contains(&"u/U USD price: highest first / lowest first".to_owned()));
    h.press("<Esc>");
    assert!(h.app.which_key_entries().is_empty());
    assert_eq!(
        h.app.sort_order(),
        SortOrder::default(),
        "cancelling changes nothing"
    );
}

#[tokio::test]
async fn comma_works_in_every_panel_in_normal_mode() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press("2,");
    assert_eq!(h.app.which_key_title(), Some("Sort by"), "in the history");
    h.press("d");
    h.settle().await;
    assert_eq!(
        h.app.sort_order(),
        SortOrder::new(SortField::Released, false)
    );
    assert_eq!(h.result_names(), ["New Dragon", "Mid Dragon", "Old Dragon"]);
    h.press("1<Esc>,D");
    h.settle().await;
    assert_eq!(
        h.result_names(),
        ["Old Dragon", "Mid Dragon", "New Dragon"],
        "in the search bar"
    );
    h.press("3d,n");
    h.settle().await;
    assert_eq!(
        h.result_names(),
        ["Mid Dragon", "New Dragon", "Old Dragon"],
        "in the full detail"
    );
}

#[tokio::test]
async fn comma_is_text_in_insert_mode() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.press("<leader>ss");
    h.type_text("a,d");
    assert_eq!(h.app.search_query(), "a,d");
    assert!(h.app.which_key_entries().is_empty());
    assert_eq!(h.app.sort_order(), SortOrder::default());
}

#[tokio::test]
async fn d_sorts_newest_first_and_upper_d_oldest_first() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    assert!(
        last_request(&h).ends_with("order=name&dir=auto"),
        "{}",
        last_request(&h)
    );
    assert_eq!(h.result_names(), ["Mid Dragon", "New Dragon", "Old Dragon"]);
    h.press(",d");
    h.settle().await;
    assert!(
        last_request(&h).ends_with("order=released&dir=auto"),
        "{}",
        last_request(&h)
    );
    assert_eq!(h.result_names(), ["New Dragon", "Mid Dragon", "Old Dragon"]);
    assert!(
        h.status_line().contains("by release date, newest first"),
        "{}",
        h.status_line()
    );
    h.press(",D");
    h.settle().await;
    assert!(
        last_request(&h).ends_with("order=released&dir=asc"),
        "{}",
        last_request(&h)
    );
    assert_eq!(h.result_names(), ["Old Dragon", "Mid Dragon", "New Dragon"]);
    assert!(
        h.status_line().contains("oldest first"),
        "{}",
        h.status_line()
    );
}

#[tokio::test]
async fn upper_n_sorts_names_in_reverse_and_n_comes_back_from_the_cache() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press(",N");
    h.settle().await;
    assert!(
        last_request(&h).ends_with("order=name&dir=desc"),
        "{}",
        last_request(&h)
    );
    assert_eq!(h.result_names(), ["Old Dragon", "New Dragon", "Mid Dragon"]);
    h.press(",n");
    h.settle().await;
    assert_eq!(h.fake.request_count(), 2, "name A → Z is cached");
    assert_eq!(h.result_names(), ["Mid Dragon", "New Dragon", "Old Dragon"]);
}

#[tokio::test]
async fn sort_keys_work_in_the_split() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press("<CR>,d");
    h.settle().await;
    assert_eq!(h.result_names(), ["New Dragon", "Mid Dragon", "Old Dragon"]);
}

#[tokio::test]
async fn the_order_stays_for_later_searches_and_history_keeps_queries_only() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press(",d");
    h.settle().await;
    h.search("old").await;
    assert!(
        last_request(&h).ends_with("order=released&dir=auto"),
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
async fn renders_sort_window() {
    let mut h = Harness::start(dragons(), 175, 6).await;
    h.search("dragon").await;
    h.press(",");
    insta::assert_snapshot!(h.render());
}
