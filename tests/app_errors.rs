//! App-level tests of failures, against the fake Scryfall.

mod support;

use std::time::Duration;

use support::fake_scryfall::card_named;
use support::harness::Harness;

fn dragons(count: usize) -> Vec<serde_json::Value> {
    (0..count)
        .map(|i| card_named(&format!("Dragon {i:02}")))
        .collect()
}

#[tokio::test]
async fn a_failure_partway_through_a_batch_keeps_the_loaded_cards() {
    let mut h = Harness::start(dragons(6), 2, 3).await;
    h.fake.fail_on_page(2);
    h.search("dragon").await;
    assert_eq!(h.result_names().len(), 2);
    let status = h.status_line();
    assert!(status.contains("2 of 6 loaded, failed"), "{status}");
    assert!(
        status.contains("Something went wrong on our end."),
        "{status}"
    );
    h.press("j");
    assert!(
        h.status_line().contains("Something went wrong on our end."),
        "the reason stays visible: {}",
        h.status_line()
    );
    h.press("G");
    h.settle().await;
    assert_eq!(
        h.fake.request_count(),
        2,
        "a failed search does not load further pages"
    );
}

#[tokio::test]
async fn running_a_failed_query_again_loads_it_from_the_start() {
    let mut h = Harness::start(dragons(6), 2, 3).await;
    h.fake.fail_on_page(2);
    h.search("dragon").await;
    h.fake.fail_on_page(99);
    h.search("dragon").await;
    assert_eq!(h.result_names().len(), 6);
    assert!(
        h.status_line().contains("6 of 6 loaded"),
        "{}",
        h.status_line()
    );
}

#[tokio::test]
async fn an_unreachable_scryfall_is_reported() {
    let mut h = Harness::start_unreachable().await;
    h.search("dragon").await;
    let status = h.status_line();
    assert!(status.contains("cannot reach Scryfall"), "{status}");
    assert!(h.result_names().is_empty());
}

#[tokio::test]
async fn a_rate_limit_pause_is_shown_and_the_search_continues() {
    let mut h = Harness::start(dragons(4), 2, 2).await;
    h.fake.set_min_interval(Duration::from_millis(30));
    h.press("<leader>ss");
    h.type_text("dragon");
    h.press("<CR>");
    let mut saw_pause = false;
    while h.app.is_loading() {
        h.next_event().await;
        saw_pause |= h
            .status_line()
            .contains("Scryfall rate limit reached, retrying in 1 s");
    }
    assert!(saw_pause);
    assert_eq!(h.result_names().len(), 4);
}
