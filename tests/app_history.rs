//! App-level tests of the history panel, against the fake Scryfall.

mod support;

use std::path::PathBuf;

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

fn history_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("scry-app-history-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("history.jsonl")
}

fn queries(h: &Harness) -> Vec<String> {
    h.app
        .history()
        .entries()
        .iter()
        .map(|e| e.query.clone())
        .collect()
}

#[tokio::test]
async fn searches_enter_the_history_with_their_count() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    h.search("sol").await;
    assert_eq!(queries(&h), ["sol", "dragon"]);
    assert_eq!(h.app.history().entries()[1].total_cards, Some(2));
    let screen = h.screen();
    let row = screen
        .lines()
        .find(|l| l.starts_with("│dragon"))
        .expect("history row");
    let row = row.split('│').nth(1).unwrap();
    assert!(row.trim_end().ends_with('2'), "{row:?}");
}

#[tokio::test]
async fn a_long_query_shows_an_ellipsis_before_its_count() {
    let mut h = Harness::start(cards(), 175, 6).await;
    // The fake ignores the terms with a colon, so two dragons match.
    h.search("dragon c:red t:legendary-creature").await;
    let screen = h.screen();
    let row = screen
        .lines()
        .skip(3)
        .find(|l| l.contains("dragon c:"))
        .expect("history row");
    let cell = row.split('│').nth(1).unwrap();
    assert!(cell.contains("... 2"), "{cell:?}");
}

#[tokio::test]
async fn enter_on_a_cached_entry_sends_no_request() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    h.search("sol").await;
    let requests = h.fake.request_count();
    h.press("2j<CR>");
    h.settle().await;
    assert_eq!(h.fake.request_count(), requests);
    assert_eq!(h.app.focus(), Panel::Main);
    assert_eq!(h.result_names(), ["Dragon Mage", "Shivan Dragon"]);
    assert_eq!(h.app.search_query(), "dragon");
    assert_eq!(
        queries(&h),
        ["dragon", "sol"],
        "re-running moves it to the top"
    );
    assert_eq!(
        h.app.history_cursor(),
        0,
        "the cursor follows the entry to the top"
    );
}

#[tokio::test]
async fn e_copies_the_query_into_the_search_bar() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    h.search("sol").await;
    h.press("2je");
    assert_eq!(
        (h.app.focus(), h.app.mode()),
        (Panel::SearchBar, Mode::Insert)
    );
    assert_eq!(h.app.search_query(), "dragon");
    h.type_text(" mage");
    h.press("<CR>");
    h.settle().await;
    assert_eq!(h.result_names(), ["Dragon Mage"]);
    assert_eq!(queries(&h), ["dragon mage", "sol", "dragon"]);
}

#[tokio::test]
async fn dd_deletes_the_entry_and_saves() {
    let path = history_file("delete");
    let mut h = Harness::start(cards(), 175, 6).await;
    h.use_history_file(&path);
    h.search("dragon").await;
    h.search("sol").await;
    h.press("2dd");
    assert_eq!(queries(&h), ["dragon"]);
    let saved = std::fs::read_to_string(&path).unwrap();
    assert_eq!(saved, "{\"query\":\"dragon\",\"total_cards\":2}\n");
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[tokio::test]
async fn the_history_survives_a_restart() {
    let path = history_file("restart");
    let mut first = Harness::start(cards(), 175, 6).await;
    first.use_history_file(&path);
    first.search("dragon").await;
    first.search("sol").await;
    drop(first);
    let mut second = Harness::start(cards(), 175, 6).await;
    second.use_history_file(&path);
    assert_eq!(queries(&second), ["sol", "dragon"]);
    assert_eq!(second.app.history().entries()[1].total_cards, Some(2));
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[tokio::test]
async fn renders_history() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    h.search("o:\"a very long oracle text query\" sol").await;
    h.press("2");
    insta::assert_snapshot!(h.render());
}

#[tokio::test]
async fn the_history_shows_its_selection_only_with_focus() {
    use ratatui::style::Color;
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    h.search("sol").await;
    let backend = h.render();
    let buf = backend.buffer();
    let history = h.app.areas().history.unwrap();
    let (x, y) = (history.x + 1, history.y + 1);
    let row = |buf: &ratatui::buffer::Buffer| -> String {
        (x..history.right() - 1)
            .map(|x| buf[(x, y)].symbol())
            .collect()
    };
    assert!(
        row(buf).starts_with("sol"),
        "no marker column: {:?}",
        row(buf)
    );
    assert!(
        !h.screen().lines().any(|l| l.starts_with("│▌")),
        "{}",
        h.screen()
    );
    assert_eq!(buf[(x, y)].bg, Color::Reset, "no highlight without focus");
    h.press("2");
    let backend = h.render();
    assert_eq!(
        backend.buffer()[(x, y)].bg,
        Color::Blue,
        "highlighted with focus"
    );
    assert!(row(backend.buffer()).starts_with("sol"));
}
