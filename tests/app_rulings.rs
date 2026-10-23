//! App-level tests of the rulings in the detail, against the fake Scryfall.

mod support;

use scry::app::config::{Config, LoadMode};
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

/// Starts with rulings in `split` and `full_detail` mode after a short pause, and searches for
/// every card.
async fn start(split: LoadMode, full_detail: LoadMode) -> Harness {
    let mut config = Config::default();
    config.search.base = String::new();
    config.detail.rulings.split = split;
    config.detail.rulings.full_detail = full_detail;
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
    let mut h = start(LoadMode::Auto, LoadMode::Auto).await;
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
    let mut h = start(LoadMode::Auto, LoadMode::Auto).await;
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
async fn each_view_follows_its_own_mode_and_never_hides_known_rulings() {
    let mut h = start(LoadMode::Never, LoadMode::Auto).await;
    h.press("j<CR>");
    assert!(!detail_has(&h, "Rulings"), "{:#?}", h.app.detail_lines());
    assert_eq!(h.app.next_deadline(), None);
    h.press("d");
    assert!(detail_has(&h, "Rulings: loading..."));
    h.tick_at_deadline().await;
    h.next_event().await;
    assert_eq!(rulings_requested(&h), ["/cards/id-glorybringer/rulings?"]);
    assert!(detail_has(&h, "Rulings (1)"));
    h.press("<CR>");
    assert!(
        !detail_has(&h, "Rulings"),
        "a split set to never hides rulings already fetched: {:#?}",
        h.app.detail_lines()
    );
}

#[tokio::test]
async fn a_failure_is_shown_and_retried_when_the_card_is_shown_again() {
    let mut h = start(LoadMode::Auto, LoadMode::Auto).await;
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

#[tokio::test]
async fn never_mode_fetches_nothing() {
    let mut h = start(LoadMode::Never, LoadMode::Never).await;
    h.press("<CR>");
    assert!(!detail_has(&h, "Rulings"), "{:#?}", h.app.detail_lines());
    assert_eq!(h.app.next_deadline(), None);
    h.press("d");
    assert!(!detail_has(&h, "Rulings"), "{:#?}", h.app.detail_lines());
    assert_eq!(h.app.next_deadline(), None);
    assert!(rulings_requested(&h).is_empty());
}

#[tokio::test]
async fn trigger_mode_loads_only_on_l_and_then_shows_them_again() {
    let mut h = start(LoadMode::Trigger, LoadMode::Trigger).await;
    h.press("j<CR>");
    assert!(
        detail_has(&h, "Rulings: L to load"),
        "{:#?}",
        h.app.detail_lines()
    );
    assert_eq!(h.app.next_deadline(), None);
    assert!(rulings_requested(&h).is_empty());
    h.press("L");
    assert!(detail_has(&h, "Rulings: loading..."));
    h.next_event().await;
    assert_eq!(rulings_requested(&h), ["/cards/id-glorybringer/rulings?"]);
    assert!(detail_has(&h, "Rulings (1)"));
    h.press("kj");
    assert!(
        detail_has(&h, "Rulings (1)"),
        "rulings already loaded show without L"
    );
    h.press("d");
    assert!(detail_has(&h, "Rulings (1)"), "in the full detail as well");
    assert_eq!(rulings_requested(&h).len(), 1);
}

#[tokio::test]
async fn l_fetches_at_once_during_the_pause() {
    let mut h = start(LoadMode::Auto, LoadMode::Auto).await;
    h.press("<CR>");
    assert!(
        h.app.next_deadline().is_some(),
        "waiting for the cursor to rest"
    );
    h.press("L");
    assert_eq!(h.app.next_deadline(), None);
    h.next_event().await;
    assert_eq!(rulings_requested(&h), ["/cards/id-dragon-mage/rulings?"]);
    assert!(detail_has(&h, "No rulings"));
}

#[tokio::test]
async fn l_says_when_there_is_nothing_to_load() {
    let mut h = start(LoadMode::Trigger, LoadMode::Trigger).await;
    h.press("<CR>L");
    h.next_event().await;
    h.press("L");
    assert!(
        h.status_line().contains("Nothing to load"),
        "{}",
        h.status_line()
    );
    assert_eq!(rulings_requested(&h).len(), 1);
}

#[tokio::test]
async fn l_fetches_nothing_in_a_view_set_to_never() {
    let mut h = start(LoadMode::Never, LoadMode::Trigger).await;
    h.press("<CR>L");
    assert!(
        h.status_line().contains("Nothing to load"),
        "{}",
        h.status_line()
    );
    assert!(!detail_has(&h, "Rulings"));
    assert!(rulings_requested(&h).is_empty());
    h.press("dL");
    h.next_event().await;
    assert_eq!(
        rulings_requested(&h).len(),
        1,
        "the full detail is on trigger"
    );
}
