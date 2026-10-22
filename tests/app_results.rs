//! App-level tests of the results view, against the fake Scryfall.

mod support;

use support::fake_scryfall::{card_named, fixture};
use support::harness::Harness;

fn dragons(count: usize) -> Vec<serde_json::Value> {
    (0..count)
        .map(|i| card_named(&format!("Dragon {i:02}")))
        .collect()
}

#[tokio::test]
async fn rows_use_the_joined_face_values() {
    let cards = vec![
        fixture("transform_delver"),
        fixture("adventure_bonecrusher"),
    ];
    let mut h = Harness::start(cards, 175, 6).await;
    h.search("e").await;
    let screen = h.screen();
    let delver = screen
        .lines()
        .find(|l| l.contains("Delver of Secrets // Insectile Aberration"))
        .expect("Delver row");
    assert!(delver.contains("{U} // —"), "{delver}");
    assert!(delver.contains("Creature // Creature"), "{delver}");
    let giant = screen
        .lines()
        .find(|l| l.contains("Bonecrusher Giant // Stomp"))
        .unwrap();
    assert!(
        giant.contains("{2}{R} // {1}{R}") && giant.contains("Creature // Instant"),
        "{giant}"
    );
}

#[tokio::test]
async fn the_count_updates_per_page() {
    let mut h = Harness::start(dragons(6), 2, 3).await;
    h.press("<leader>ss");
    h.type_text("dragon");
    h.press("<CR>");
    assert!(
        h.status_line().contains("loading..."),
        "{}",
        h.status_line()
    );
    h.next_event().await;
    assert!(
        h.status_line().contains("2 of 6 loading..."),
        "{}",
        h.status_line()
    );
    h.next_event().await;
    assert!(
        h.status_line().contains("4 of 6 loading..."),
        "{}",
        h.status_line()
    );
    h.settle().await;
    assert!(
        h.status_line().contains("6 of 6 loaded"),
        "{}",
        h.status_line()
    );
}

#[tokio::test]
async fn reaching_the_end_loads_exactly_one_more_batch() {
    let mut h = Harness::start(dragons(10), 2, 2).await;
    h.search("dragon").await;
    assert_eq!(h.result_names().len(), 4);
    assert_eq!(h.fake.request_count(), 2);
    h.press("G");
    h.settle().await;
    assert_eq!(h.result_names().len(), 8);
    assert_eq!(h.fake.request_count(), 4);
    assert_eq!(h.app.results_cursor(), 3, "the cursor stays on its row");
    h.press("G");
    h.settle().await;
    assert_eq!(h.result_names().len(), 10);
    assert!(
        h.status_line().contains("10 of 10 loaded"),
        "{}",
        h.status_line()
    );
    h.press("Gjj");
    h.settle().await;
    assert_eq!(
        h.fake.request_count(),
        5,
        "nothing is sent once everything is loaded"
    );
}

#[tokio::test]
async fn end_loads_the_next_batch_like_upper_g() {
    let mut h = Harness::start(dragons(6), 2, 1).await;
    h.search("dragon").await;
    assert_eq!(h.result_names().len(), 2);
    h.press("<End>");
    h.settle().await;
    assert_eq!(h.result_names().len(), 4);
}

#[tokio::test]
async fn no_match_shows_scryfalls_notice() {
    let mut h = Harness::start(dragons(2), 175, 6).await;
    h.search("goblin").await;
    assert!(
        h.screen().contains("Your query didn’t match any cards."),
        "{}",
        h.screen()
    );
    assert!(
        h.status_line().contains("0 of 0 loaded"),
        "{}",
        h.status_line()
    );
}

#[tokio::test]
async fn warnings_are_shown() {
    let mut h = Harness::start(dragons(2), 175, 6).await;
    h.search("dragon c:r").await;
    assert!(
        h.status_line()
            .contains("Invalid expression “c:r” was ignored."),
        "{}",
        h.status_line()
    );
}

#[tokio::test]
async fn renders_results() {
    let cards = vec![
        fixture("normal_glorybringer"),
        fixture("legendary_balefang"),
        fixture("transform_delver"),
        fixture("split_fire_ice"),
        fixture("land_snow_covered_island"),
    ];
    let mut h = Harness::start(cards, 175, 6).await;
    h.search("i").await;
    insta::assert_snapshot!(h.render());
}
