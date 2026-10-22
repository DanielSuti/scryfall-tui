//! App-level tests of searching, against the fake Scryfall.

mod support;

use scry::tui::keymap::mode::Mode;
use scry::tui::layout::Panel;
use support::fake_scryfall::card_named;
use support::harness::Harness;

fn cards() -> Vec<serde_json::Value> {
    ["Glorybringer", "Shivan Dragon", "Sol Ring", "Dragon Mage"]
        .iter()
        .map(|n| card_named(n))
        .collect()
}

#[tokio::test]
async fn one_keeps_the_query_and_leader_ss_clears_it() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.press("<leader>ss");
    h.type_text("glory");
    h.press("<Esc>");
    assert_eq!(h.app.mode(), Mode::Normal);
    h.press("1");
    assert_eq!(
        (h.app.focus(), h.app.mode()),
        (Panel::SearchBar, Mode::Insert)
    );
    assert_eq!(h.app.search_query(), "glory");
    h.press("<Esc><leader>ss");
    assert_eq!(
        (h.app.focus(), h.app.mode()),
        (Panel::SearchBar, Mode::Insert)
    );
    assert_eq!(h.app.search_query(), "");
}

#[tokio::test]
async fn enter_runs_one_search_and_focuses_the_results() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    assert_eq!(h.fake.request_count(), 1);
    assert_eq!((h.app.focus(), h.app.mode()), (Panel::Main, Mode::Normal));
    assert_eq!(h.result_names(), ["Shivan Dragon", "Dragon Mage"]);
}

#[tokio::test]
async fn an_empty_query_sends_nothing() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.press("<leader>ss");
    h.type_text("   ");
    h.press("<CR>");
    assert_eq!(h.app.mode(), Mode::Insert);
    assert_eq!(h.app.message().unwrap().text, "Type a query first");
    assert_eq!(h.fake.request_count(), 0);
}

#[tokio::test]
async fn running_a_cached_query_again_sends_no_request() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    h.search("glory").await;
    h.search("dragon").await;
    assert_eq!(h.fake.request_count(), 2);
    assert_eq!(h.result_names(), ["Shivan Dragon", "Dragon Mage"]);
}
