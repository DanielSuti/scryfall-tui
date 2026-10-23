//! App-level tests of the card image in the detail, against the fake Scryfall. A half-block
//! picker stands in for a terminal's graphics protocol, so the image is drawn into the test
//! screen as cells in the image's colors.

mod support;

use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui_image::picker::Picker;
use scry::app::config::{Config, LoadMode};
use support::fake_scryfall::card_named;
use support::harness::Harness;

/// The color of every pixel of the fake's image. Half blocks draw a solid image as spaces on
/// that color.
const IMAGE_COLOR: Color = Color::Rgb(200, 40, 30);

fn cards() -> Vec<serde_json::Value> {
    // Shown by name: Dragon Mage, Glorybringer, Shivan Dragon.
    ["Glorybringer", "Shivan Dragon", "Dragon Mage"]
        .iter()
        .map(|n| card_named(n))
        .collect()
}

/// Starts with images in `split` and `full_detail` mode after a short pause, rulings off, and
/// a terminal that draws images when `picker` is true, and searches for every card.
async fn start_with(split: LoadMode, full_detail: LoadMode, picker: bool) -> Harness {
    let mut config = Config::default();
    config.search.base = String::new();
    config.detail.rulings.split = LoadMode::Never;
    config.detail.rulings.full_detail = LoadMode::Never;
    config.detail.images.split = split;
    config.detail.images.full_detail = full_detail;
    config.detail.images.pause_ms = 20;
    let mut h = Harness::start_with_config(cards(), 175, config).await;
    if picker {
        h.app.set_image_picker(Some(Picker::halfblocks()));
    }
    h.search("r").await;
    h
}

/// Starts with images in `mode` in both views.
async fn start(mode: LoadMode) -> Harness {
    start_with(mode, mode, true).await
}

fn images_requested(h: &Harness) -> Vec<String> {
    h.fake
        .images_received()
        .into_iter()
        .map(|r| r.path_and_query)
        .collect()
}

/// Returns the rectangle the detail's text and image are drawn in.
fn detail_inner(h: &Harness) -> Rect {
    let areas = h.app.areas();
    let outer = areas.detail.unwrap_or(areas.main);
    Rect::new(outer.x + 1, outer.y + 1, outer.width - 2, outer.height - 2)
}

/// Returns true when cell (`x`, `y`) of the screen holds part of the image.
fn image_at(h: &Harness, x: u16, y: u16) -> bool {
    h.render().buffer()[(x, y)].bg == IMAGE_COLOR
}

fn screen_has_image(h: &Harness) -> bool {
    h.render()
        .buffer()
        .content()
        .iter()
        .any(|cell| cell.bg == IMAGE_COLOR)
}

/// Returns the screen as text with the image's cells drawn as `▓`.
fn screen_with_image(h: &Harness) -> String {
    let backend = h.render();
    let buffer = backend.buffer();
    buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(|cell| match cell.bg {
                    IMAGE_COLOR => "▓",
                    _ => cell.symbol(),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Waits for the image fetch and then for its encoding.
async fn deliver(h: &mut Harness) {
    h.next_event().await;
    h.next_event().await;
}

#[tokio::test]
async fn auto_mode_draws_the_image_beside_the_text_once_the_cursor_rests() {
    let mut h = start(LoadMode::Auto).await;
    h.press("<CR>");
    assert!(h.screen().contains("Loading image..."), "{}", h.screen());
    h.press("j");
    assert!(images_requested(&h).is_empty(), "nothing before the pause");
    h.tick_at_deadline().await;
    deliver(&mut h).await;
    assert_eq!(
        images_requested(&h),
        ["/images/id-glorybringer.png?"],
        "only the card the cursor rested on"
    );
    let inner = detail_inner(&h);
    // 19 columns wide at two fifths of the 49 inner columns, one column from the border.
    let image_x = inner.right() - 20;
    assert!(image_at(&h, image_x, inner.y), "{}", h.screen());
    assert!(!image_at(&h, image_x - 1, inner.y), "the gap stays blank");
    assert!(!h.screen().contains("Loading image..."));
    let narrow = h.app.detail_lines()[..13].iter();
    assert!(
        narrow.clone().all(|l| l.chars().count() <= 26),
        "{narrow:#?}"
    );
    assert_eq!(h.app.next_deadline(), None);
}

#[tokio::test]
async fn each_view_follows_its_own_mode_and_the_full_detail_draws_on_the_left() {
    let mut h = start_with(LoadMode::Never, LoadMode::Auto, true).await;
    h.press("<CR>");
    assert!(!h.screen().contains("Loading image..."));
    assert_eq!(h.app.next_deadline(), None);
    h.press("d");
    assert!(h.screen().contains("Loading image..."));
    h.tick_at_deadline().await;
    deliver(&mut h).await;
    assert_eq!(images_requested(&h), ["/images/id-dragon-mage.png?"]);
    let inner = detail_inner(&h);
    assert!(image_at(&h, inner.x + 1, inner.y), "{}", h.screen());
    assert!(
        h.screen()
            .lines()
            .nth(usize::from(inner.y))
            .unwrap()
            .contains("Dragon Mage"),
        "the text stands right of the image"
    );
    h.press("<CR>");
    assert!(
        !screen_has_image(&h) && !h.screen().contains("Loading image..."),
        "a split set to never hides an image already fetched: {}",
        h.screen()
    );
    assert!(
        h.app.detail_lines()[0].ends_with("{3}{R}{R}") && h.app.detail_lines()[0].len() > 40,
        "the text takes the full width: {:#?}",
        h.app.detail_lines()
    );
    assert_eq!(h.app.next_deadline(), None);
}

#[tokio::test]
async fn never_mode_and_terminals_without_graphics_fetch_nothing() {
    for h in [
        start(LoadMode::Never).await,
        start_with(LoadMode::Auto, LoadMode::Auto, false).await,
    ] {
        let mut h = h;
        h.press("<CR>");
        assert_eq!(h.app.next_deadline(), None);
        h.press("d");
        assert!(!h.screen().contains("Loading image..."));
        assert!(images_requested(&h).is_empty());
        assert!(
            h.app.detail_lines()[0].starts_with("Dragon Mage"),
            "{:#?}",
            h.app.detail_lines()
        );
    }
}

#[tokio::test]
async fn a_failure_is_shown_and_retried_when_the_card_is_shown_again() {
    let mut h = start(LoadMode::Auto).await;
    h.fake.fail_image("id-dragon-mage", Some(500));
    h.press("<CR>");
    h.tick_at_deadline().await;
    h.next_event().await;
    assert!(h.screen().contains("Image: unexpected"), "{}", h.screen());
    h.fake.fail_image("id-dragon-mage", None);
    h.press("q<CR>");
    h.tick_at_deadline().await;
    deliver(&mut h).await;
    assert!(screen_has_image(&h), "{}", h.screen());
    assert_eq!(images_requested(&h).len(), 2);
}

#[tokio::test]
async fn a_card_shown_again_is_not_fetched_again() {
    let mut h = start(LoadMode::Auto).await;
    h.press("<CR>");
    h.tick_at_deadline().await;
    deliver(&mut h).await;
    h.press("j");
    h.tick_at_deadline().await;
    deliver(&mut h).await;
    h.press("k");
    assert_eq!(h.app.next_deadline(), None, "nothing left to fetch");
    h.next_event().await;
    assert!(screen_has_image(&h));
    assert_eq!(images_requested(&h).len(), 2);
}

#[tokio::test]
async fn the_image_scrolls_with_the_splits_text() {
    let mut h = start(LoadMode::Auto).await;
    h.press("<CR>");
    h.tick_at_deadline().await;
    deliver(&mut h).await;
    let inner = detail_inner(&h);
    let x = inner.right() - 20;
    // The image is 13 rows high.
    assert!(image_at(&h, x, inner.y + 12));
    h.press("0j");
    assert!(image_at(&h, x, inner.y + 11));
    assert!(!image_at(&h, x, inner.y + 12), "{}", h.screen());
}

#[tokio::test]
async fn popups_hide_the_image() {
    let mut h = start(LoadMode::Auto).await;
    h.press("<CR>");
    h.tick_at_deadline().await;
    deliver(&mut h).await;
    assert!(screen_has_image(&h));
    h.press("<leader>");
    h.tick_at_deadline().await;
    assert!(
        h.screen().contains("search"),
        "the which-key popup is open: {}",
        h.screen()
    );
    assert!(!screen_has_image(&h), "{}", h.screen());
    h.press("<Esc>");
    assert!(screen_has_image(&h));
}

#[tokio::test]
async fn renders_the_split_with_an_image() {
    let mut h = start(LoadMode::Auto).await;
    h.press("<CR>");
    h.tick_at_deadline().await;
    deliver(&mut h).await;
    insta::assert_snapshot!(screen_with_image(&h));
}

#[tokio::test]
async fn trigger_mode_leaves_no_room_until_l() {
    let mut h = start(LoadMode::Trigger).await;
    h.press("<CR>");
    assert!(!h.screen().contains("Loading image..."));
    assert!(
        h.app.detail_lines()[0].len() > 40,
        "the text takes the full width: {:#?}",
        h.app.detail_lines()
    );
    assert_eq!(h.app.next_deadline(), None);
    assert!(images_requested(&h).is_empty());
    h.press("L");
    assert!(h.screen().contains("Loading image..."), "{}", h.screen());
    deliver(&mut h).await;
    assert_eq!(images_requested(&h), ["/images/id-dragon-mage.png?"]);
    assert!(screen_has_image(&h));
    h.press("jk");
    assert!(
        screen_has_image(&h),
        "an image already loaded shows without L"
    );
    assert_eq!(images_requested(&h).len(), 1);
}
