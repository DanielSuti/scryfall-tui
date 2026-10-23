//! App-level tests of the rulings in the detail, against the fake Scryfall.

mod support;

use scry::app::config::{Config, RulingsMode};
use support::fake_scryfall::{card_named, ruling};
use support::harness::Harness;

const EXERT: &str = "You can’t exert a creature unless an effect allows you to do so.";

fn cards() -> Vec<serde_json::Value> {
    // Shown by name: Dragon Mage, Glorybringer, Shivan Dragon.
    ["Glorybringer", "Shivan Dragon", "Dragon Mage"]
        .iter()
        .map(|n| card_named(n))
        .collect()
}

/// Starts with `mode` and a short pause, and searches for every card.
async fn start(mode: RulingsMode) -> Harness {
    let mut config = Config::default();
    config.search.base = String::new();
    config.detail.rulings.mode = mode;
    config.detail.rulings.pause_ms = 20;
    let mut h = Harness::start_with_config(cards(), 175, config).await;
    h.fake
        .set_rulings("id-glorybringer", vec![ruling("2017-04-18", EXERT)]);
    h.search("r").await;
    h
}

fn detail_has(h: &Harness, text: &str) -> bool {
    h.app.detail_lines().iter().any(|l| l.contains(text))
}

fn rulings_requested(h: &Harness) -> Vec<String> {
    h.fake
        .rulings_received()
        .into_iter()
        .map(|r| r.path_and_query)
        .collect()
}

#[tokio::test]
async fn auto_mode_fetches_once_the_cursor_rests() {
    let mut h = start(RulingsMode::Auto).await;
    h.press("<CR>");
    assert!(detail_has(&h, "Rulings: loading..."));
    h.press("j");
    assert!(rulings_requested(&h).is_empty(), "nothing before the pause");
    h.tick_at_deadline().await;
    h.next_event().await;
    assert_eq!(
        rulings_requested(&h),
        ["/cards/id-glorybringer/rulings?"],
        "only the card the cursor rested on"
    );
    assert!(detail_has(&h, "Rulings (1)"), "{:#?}", h.app.detail_lines());
    assert!(detail_has(&h, "2017-04-18  You can’t exert a creature"));
    assert_eq!(h.app.next_deadline(), None);
}

#[tokio::test]
async fn known_rulings_are_not_fetched_again() {
    let mut h = start(RulingsMode::Auto).await;
    h.press("<CR>");
    h.tick_at_deadline().await;
    h.next_event().await;
    assert!(detail_has(&h, "No rulings"), "{:#?}", h.app.detail_lines());
    h.press("j");
    h.tick_at_deadline().await;
    h.next_event().await;
    h.press("k");
    assert!(detail_has(&h, "No rulings"));
    assert_eq!(h.app.next_deadline(), None, "nothing left to fetch");
    assert_eq!(rulings_requested(&h).len(), 2);
}

#[tokio::test]
async fn full_detail_mode_fetches_only_in_the_full_detail_and_at_once() {
    let mut h = start(RulingsMode::FullDetail).await;
    h.press("j<CR>");
    assert!(!detail_has(&h, "Rulings"), "{:#?}", h.app.detail_lines());
    assert_eq!(h.app.next_deadline(), None);
    h.press("d");
    assert!(detail_has(&h, "Rulings: loading..."));
    h.next_event().await;
    assert_eq!(rulings_requested(&h), ["/cards/id-glorybringer/rulings?"]);
    assert!(detail_has(&h, "Rulings (1)"));
    h.press("q");
    assert!(
        detail_has(&h, "Rulings (1)"),
        "the split shows rulings already fetched"
    );
}

#[tokio::test]
async fn a_failure_is_shown_and_retried_when_the_card_is_shown_again() {
    let mut h = start(RulingsMode::Auto).await;
    h.fake.fail_rulings("id-dragon-mage", true);
    h.press("<CR>");
    h.tick_at_deadline().await;
    h.next_event().await;
    assert!(
        detail_has(&h, "Rulings: Something went wrong on our end."),
        "{:#?}",
        h.app.detail_lines()
    );
    h.fake.fail_rulings("id-dragon-mage", false);
    h.press("q<CR>");
    h.tick_at_deadline().await;
    h.next_event().await;
    assert!(detail_has(&h, "No rulings"), "{:#?}", h.app.detail_lines());
    assert_eq!(rulings_requested(&h).len(), 2);
}
