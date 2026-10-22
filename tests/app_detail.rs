//! App-level tests of the card detail, against the fake Scryfall.

mod support;

use scry::app::config::Config;
use scry::tui::main_panel::MainView;
use support::fake_scryfall::fixture;
use support::harness::Harness;

fn cards() -> Vec<serde_json::Value> {
    vec![
        fixture("normal_glorybringer"),
        fixture("legendary_balefang"),
        fixture("transform_delver"),
    ]
}

fn first_line(h: &Harness) -> String {
    h.app.detail_lines().first().cloned().unwrap_or_default()
}

#[tokio::test]
async fn in_the_split_the_detail_follows_the_results_cursor() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("e").await;
    h.press("<CR>");
    assert_eq!(h.app.main_view(), MainView::Split);
    // Scryfall sorts by name: Balefang, Delver, Glorybringer.
    assert!(first_line(&h).starts_with("Balefang"), "{}", first_line(&h));
    h.press("j");
    assert!(
        first_line(&h).starts_with("Delver of Secrets"),
        "{}",
        first_line(&h)
    );
    assert_eq!(h.app.selection().unwrap().index, 1);
}

#[tokio::test]
async fn j_and_k_step_cards_in_the_full_detail_and_stop_at_the_last() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("e").await;
    h.press("d");
    h.press("J");
    assert!(first_line(&h).starts_with("Delver of Secrets"));
    h.press("JJJ");
    assert!(
        first_line(&h).starts_with("Glorybringer"),
        "J at the last card stays put"
    );
    h.press("K");
    assert!(first_line(&h).starts_with("Delver of Secrets"));
}

#[tokio::test]
async fn an_unknown_format_is_reported_with_its_name() {
    let mut config = Config::default();
    config.detail.formats = vec!["modern".into(), "bogus".into()];
    let mut h = Harness::start_with_config(cards(), 175, config).await;
    h.search("e").await;
    let warning = h.app.format_warning().expect("a warning");
    assert!(
        warning.contains("`bogus`") && !warning.contains("`modern`"),
        "{warning}"
    );
    assert!(h.status_line().contains("`bogus`"), "{}", h.status_line());
}

#[tokio::test]
async fn slash_searches_the_detail_only_while_it_has_focus() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("glory").await;
    h.press("<CR>");
    h.press("/haste<CR>");
    assert_eq!(h.app.message().unwrap().text, "Pattern not found: haste");
    h.press("0/haste<CR>");
    assert_eq!(h.app.message(), None);
    let lines = h.app.detail_lines();
    assert!(lines.iter().any(|l| l.contains("haste")));
}

#[tokio::test]
async fn renders_split_detail() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("e").await;
    h.press("<CR>");
    insta::assert_snapshot!(h.render());
}

#[tokio::test]
async fn renders_full_detail() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("e").await;
    h.press("jjd");
    insta::assert_snapshot!(h.render());
}
