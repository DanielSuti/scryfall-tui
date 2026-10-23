//! App-level tests of flash jumps (`s`), against the fake Scryfall.

mod support;

use ratatui::style::{Color, Modifier};
use scry::tui::keymap::mode::Mode;
use scry::tui::layout::Panel;
use support::fake_scryfall::{card_named, fixture};
use support::harness::Harness;

fn cards() -> Vec<serde_json::Value> {
    // Shown by name: Dragon Mage, Glorybringer, Shivan Dragon, Sol Ring.
    ["Glorybringer", "Shivan Dragon", "Sol Ring", "Dragon Mage"]
        .iter()
        .map(|n| card_named(n))
        .collect()
}

/// Opens flash, types `pattern`, and returns the label of the only match in `panel`.
fn label_in(h: &mut Harness, pattern: &str, panel: Panel) -> char {
    h.press("s");
    h.type_text(pattern);
    let flash = h.app.flash().expect("flash is open");
    let in_panel: Vec<_> = flash
        .matches()
        .iter()
        .filter(|m| m.panel == panel)
        .collect();
    assert_eq!(in_panel.len(), 1, "{:?}", flash.matches());
    in_panel[0].label.expect("a label")
}

fn press_label(h: &mut Harness, label: char) {
    h.type_text(&label.to_string());
}

#[tokio::test]
async fn a_label_jumps_to_its_result() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    let label = label_in(&mut h, "shiv", Panel::Main);
    press_label(&mut h, label);
    assert!(h.app.flash().is_none());
    assert_eq!((h.app.focus(), h.app.mode()), (Panel::Main, Mode::Normal));
    assert_eq!(h.app.results_cursor(), 2);
}

#[tokio::test]
async fn a_jump_can_move_to_another_panel() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    h.search("sol").await;
    let label = label_in(&mut h, "drag", Panel::History);
    press_label(&mut h, label);
    assert_eq!(h.app.focus(), Panel::History);
    assert_eq!(h.app.history_cursor(), 1, "the `dragon` entry");
}

#[tokio::test]
async fn enter_jumps_to_the_nearest_match() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("G");
    h.press("s");
    h.type_text("dragon");
    h.press("<CR>");
    assert_eq!(
        h.app.results_cursor(),
        2,
        "Shivan Dragon is nearer to Sol Ring than Dragon Mage"
    );
}

#[tokio::test]
async fn a_jump_into_the_search_bar_puts_the_cursor_on_the_match() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("dragon").await;
    let label = label_in(&mut h, "agon", Panel::SearchBar);
    press_label(&mut h, label);
    assert_eq!(
        (h.app.focus(), h.app.mode()),
        (Panel::SearchBar, Mode::Insert)
    );
    h.type_text("X");
    assert_eq!(h.app.search_query(), "drXagon");
}

#[tokio::test]
async fn a_jump_into_the_detail_scrolls_the_line_to_the_top() {
    let mut h = Harness::start(vec![fixture("normal_glorybringer")], 175, 6).await;
    h.search("glory").await;
    h.press("<CR>");
    let label = label_in(&mut h, "haste", Panel::Detail);
    press_label(&mut h, label);
    assert_eq!(h.app.focus(), Panel::Detail);
    let detail = h.app.areas().detail.unwrap();
    let top = h
        .screen()
        .lines()
        .nth(usize::from(detail.y + 1))
        .unwrap()
        .to_owned();
    assert!(top.contains("Flying, haste"), "{top}");
}

#[tokio::test]
async fn a_letter_that_continues_a_match_narrows_instead_of_jumping() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("s");
    h.type_text("dr");
    let flash = h.app.flash().unwrap();
    assert!(
        flash.matches().iter().all(|m| m.label != Some('a')),
        "`a` follows `dr`"
    );
    h.type_text("a");
    let flash = h.app.flash().expect("still open");
    assert_eq!(flash.pattern(), "dra");
    assert_eq!(h.app.results_cursor(), 0);
}

#[tokio::test]
async fn escape_and_backspace_close_flash_without_moving() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("s");
    h.type_text("sol");
    h.press("<Esc>");
    assert!(h.app.flash().is_none());
    assert_eq!((h.app.focus(), h.app.results_cursor()), (Panel::Main, 0));
    h.press("s");
    h.type_text("x");
    h.press("<BS>");
    assert_eq!(h.app.flash().unwrap().pattern(), "");
    h.press("<BS>");
    assert!(h.app.flash().is_none());
}

#[tokio::test]
async fn s_is_text_in_insert_mode() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.press("<leader>ss");
    h.type_text("sos");
    assert!(h.app.flash().is_none());
    assert_eq!(h.app.search_query(), "sos");
}

#[tokio::test]
async fn flash_dims_the_panels_and_colors_matches_and_labels() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("G");
    h.press("s");
    h.type_text("dragon");
    let flash = h.app.flash().unwrap().clone();
    let backend = h.render();
    let buf = backend.buffer();
    let nearest = &flash.matches()[0];
    let other = &flash.matches()[1];
    // Matches, the nearest match and labels are reversed: dark text on their color.
    let near = &buf[(nearest.columns.0, nearest.y)];
    assert_eq!((near.fg, near.bg), (Color::LightYellow, Color::Reset));
    assert!(near.modifier.contains(Modifier::REVERSED));
    let far = &buf[(other.columns.0, other.y)];
    assert_eq!((far.fg, far.bg), (Color::Blue, Color::Reset));
    assert!(far.modifier.contains(Modifier::REVERSED));
    let label = buf[(other.columns.1 + 1, other.y)].clone();
    assert_eq!(label.symbol(), other.label.unwrap().to_string());
    assert_eq!((label.fg, label.bg), (Color::LightRed, Color::Reset));
    assert!(label.modifier.contains(Modifier::REVERSED));
    assert!(label.modifier.contains(Modifier::BOLD));
    let main = h.app.areas().main;
    let sol = h
        .screen()
        .lines()
        .position(|l| l.contains("Sol Ring"))
        .unwrap();
    let sol_cell = &buf[(main.x + 3, u16::try_from(sol).unwrap())];
    assert_eq!(
        sol_cell.fg,
        Color::DarkGray,
        "text without a match is dimmed"
    );
    let status = h.status_line();
    let prompt = status.strip_prefix("⚡").expect("the flash prompt");
    assert_eq!(prompt.trim(), "dragon", "{status}");
    insta::assert_snapshot!(backend);
}
