//! App-level tests of `/` in a panel: matches are highlighted as they are typed.

mod support;

use ratatui::style::{Color, Modifier};
use scry::tui::keymap::mode::Mode;
use support::fake_scryfall::card_named;
use support::harness::Harness;

fn cards() -> Vec<serde_json::Value> {
    // Shown by name: Dragon Mage, Glorybringer, Shivan Dragon, Sol Ring.
    ["Glorybringer", "Shivan Dragon", "Sol Ring", "Dragon Mage"]
        .iter()
        .map(|n| card_named(n))
        .collect()
}

/// Returns the cells of the first occurrence of `text` inside the main panel.
fn cells_of(h: &Harness, text: &str) -> Vec<ratatui::buffer::Cell> {
    let backend = h.render();
    let buf = backend.buffer();
    let main = h.app.areas().main;
    for y in main.y + 1..main.bottom() - 1 {
        let row: Vec<&ratatui::buffer::Cell> = (main.x + 1..main.right() - 1)
            .map(|x| &buf[(x, y)])
            .collect();
        let symbols: Vec<&str> = row.iter().map(|c| c.symbol()).collect();
        let wanted: Vec<String> = text.chars().map(String::from).collect();
        if let Some(start) = symbols
            .windows(wanted.len())
            .position(|w| w.iter().zip(&wanted).all(|(a, b)| *a == b))
        {
            return row[start..start + wanted.len()]
                .iter()
                .map(|c| (*c).clone())
                .collect();
        }
    }
    panic!("{text:?} is not in the main panel:\n{}", h.screen());
}

fn is_lit(cells: &[ratatui::buffer::Cell]) -> bool {
    cells
        .iter()
        .all(|c| c.fg == Color::Yellow && c.modifier.contains(Modifier::UNDERLINED))
}

fn any_lit(h: &Harness) -> bool {
    let backend = h.render();
    backend
        .buffer()
        .content()
        .iter()
        .any(|c| c.fg == Color::Yellow)
}

#[tokio::test]
async fn matches_light_up_and_the_cursor_follows_while_typing() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("/");
    h.type_text("glo");
    assert_eq!(h.app.mode(), Mode::Insert, "still typing");
    assert_eq!(h.app.results_cursor(), 1, "on Glorybringer");
    assert!(is_lit(&cells_of(&h, "Glo")));
    assert!(!is_lit(&cells_of(&h, "rybringer")));
    h.type_text("x");
    assert_eq!(
        h.app.results_cursor(),
        0,
        "no match: back where the prompt opened"
    );
    assert!(!any_lit(&h));
}

#[tokio::test]
async fn every_match_lights_up_ignoring_case() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("/");
    h.type_text("dragon");
    assert!(is_lit(&cells_of(&h, "Dragon Mage")[..6]));
    assert!(is_lit(&cells_of(&h, "Shivan Dragon")[7..]));
    assert!(!is_lit(&cells_of(&h, "Sol Ring")));
}

#[tokio::test]
async fn escape_in_the_prompt_restores_the_cursor_and_the_highlight_goes() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("/");
    h.type_text("sol");
    assert_eq!(h.app.results_cursor(), 3);
    h.press("<Esc>");
    assert_eq!(h.app.mode(), Mode::Normal);
    assert_eq!(h.app.results_cursor(), 0);
    assert!(!any_lit(&h));
}

#[tokio::test]
async fn the_highlight_stays_after_enter_until_escape() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("/");
    h.type_text("dragon");
    h.press("<CR>");
    assert_eq!(h.app.mode(), Mode::Normal);
    assert_eq!(
        h.app.results_cursor(),
        2,
        "the first match after the cursor"
    );
    assert!(is_lit(&cells_of(&h, "Dragon Mage")[..6]));
    h.press("n");
    assert_eq!(h.app.results_cursor(), 0);
    assert!(any_lit(&h));
    h.press("<Esc>");
    assert!(!any_lit(&h));
    h.press("n");
    assert!(any_lit(&h), "n shows the highlight again");
}

#[tokio::test]
async fn only_the_focused_panel_lights_up() {
    let mut h = Harness::start(cards(), 175, 6).await;
    h.search("o").await;
    h.press("2/o");
    let backend = h.render();
    let buf = backend.buffer();
    let main = h.app.areas().main;
    let main_lit = (main.x..main.right())
        .flat_map(|x| (main.y..main.bottom()).map(move |y| (x, y)))
        .any(|(x, y)| buf[(x, y)].fg == Color::Yellow);
    assert!(
        !main_lit,
        "the results are not searched while the history has focus"
    );
    assert!(any_lit(&h), "the history entry `o` lights up");
}
