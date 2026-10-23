//! App-level tests of the config window on `<leader>c`, against the fake Scryfall.

mod support;

use scry::app::config::{Config, LoadMode};
use support::fake_scryfall::card_named;
use support::harness::Harness;

fn cards() -> Vec<serde_json::Value> {
    ["Glorybringer", "Shivan Dragon", "Dragon Mage"]
        .iter()
        .map(|n| card_named(n))
        .collect()
}

/// Starts with the default settings, no base terms, and the split open on the results.
async fn start() -> Harness {
    let mut config = Config::default();
    config.search.base = String::new();
    let mut h = Harness::start_with_config(cards(), 175, config).await;
    h.search("r").await;
    h.press("<CR>");
    h
}

fn detail_has(h: &Harness, text: &str) -> bool {
    h.app.detail_lines().iter().any(|l| l.contains(text))
}

#[tokio::test]
async fn tab_changes_a_setting_for_the_session_and_the_detail_follows() {
    let mut h = start().await;
    assert!(detail_has(&h, "Rulings: L to load"));
    h.press("<leader>c");
    assert!(h.app.config_window().is_some());
    assert!(
        h.screen().contains("Terms added to every search"),
        "{}",
        h.screen()
    );
    assert!(
        h.status_line().contains("no changes"),
        "{}",
        h.status_line()
    );
    // Down to "Rulings in the split", then back from trigger to auto.
    h.press("jjjj<S-Tab>");
    assert_eq!(h.app.config().detail.rulings.split, LoadMode::Auto);
    assert!(
        detail_has(&h, "Rulings: loading..."),
        "the split waits for the pause now: {:#?}",
        h.app.detail_lines()
    );
    assert!(h.status_line().contains("1 changed"), "{}", h.status_line());
    h.press("<Tab><Tab>");
    assert_eq!(h.app.config().detail.rulings.split, LoadMode::Never);
    assert!(!detail_has(&h, "Rulings"));
    h.press("<Esc>");
    assert!(h.app.config_window().is_none());
    assert_eq!(
        h.app.config().detail.rulings.split,
        LoadMode::Never,
        "<Esc> keeps the change"
    );
}

#[tokio::test]
async fn q_sets_back_the_changes_of_this_visit() {
    let mut h = start().await;
    h.press("<leader>cj<Tab>");
    assert_eq!(h.app.config().search.max_pages.get(), 6);
    h.press("<Esc><leader>cjjjj<Tab>");
    assert_eq!(h.app.config().detail.rulings.split, LoadMode::Never);
    h.press("q");
    assert!(h.app.config_window().is_none());
    assert_eq!(h.app.config().detail.rulings.split, LoadMode::Trigger);
    assert_eq!(
        h.app.config().search.max_pages.get(),
        6,
        "a change from an earlier visit stays"
    );
    assert!(detail_has(&h, "Rulings: L to load"));
}

#[tokio::test]
async fn other_keys_do_nothing_while_the_window_is_open() {
    let mut h = start().await;
    h.press("<leader>cQd");
    assert!(!h.app.should_quit());
    assert!(h.app.config_window().is_some());
    assert_eq!(h.app.config(), &{
        let mut config = Config::default();
        config.search.base = String::new();
        config
    });
}

#[tokio::test]
async fn a_smaller_history_drops_the_oldest_entries_and_is_saved() {
    let dir = std::env::temp_dir().join(format!("scry-app-config-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("history.jsonl");
    let lines: String = (0..25)
        .map(|i| format!("{{\"query\":\"q{i}\",\"total_cards\":1}}\n"))
        .collect();
    std::fs::write(&path, lines).unwrap();
    let mut h = start().await;
    h.use_history_file(&path);
    // Down to "History entries kept", then back from 40 to 20.
    h.press("<leader>cjj<S-Tab><Esc>");
    assert_eq!(h.app.config().history.max_entries.get(), 20);
    let saved = std::fs::read_to_string(&path).unwrap();
    assert_eq!(saved.lines().count(), 20);
    assert!(
        saved.starts_with("{\"query\":\"q0\""),
        "the newest stay: {saved}"
    );
}

#[tokio::test]
async fn renders_the_config_window() {
    let mut h = start().await;
    h.press("<leader>cjjjj<Tab>");
    insta::assert_snapshot!(h.screen());
}
