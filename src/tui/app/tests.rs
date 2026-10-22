//! Tests of the application state and key handling.
use super::*;
use crate::app::command::Command;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn key(code: KeyCode) -> AppEvent {
    AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_event(key(KeyCode::Char(c)));
    }
}

#[test]
fn capital_q_quits_in_normal_mode() {
    let mut app = App::new();
    type_text(&mut app, "2");
    app.handle_event(key(KeyCode::Char('q')));
    assert!(!app.should_quit());
    app.handle_event(key(KeyCode::Char('Q')));
    assert!(app.should_quit());
}

#[test]
fn insert_mode_keys_reach_the_input() {
    let mut app = App::new();
    app.enter_insert_mode();
    type_text(&mut app, "Q x");
    assert!(!app.should_quit());
    assert_eq!(app.search_bar.query(), "Q x");
    app.handle_event(key(KeyCode::Esc));
    assert_eq!(app.mode(), Mode::Normal);
    type_text(&mut app, "y");
    assert_eq!(app.search_bar.query(), "Q x");
    app.handle_event(key(KeyCode::Char('Q')));
    assert!(app.should_quit());
}

fn test_command(app: &mut App, id: &'static str, notation: &str, description: &'static str) {
    scoped_command(app, Scope::GLOBAL, id, notation, description, None);
}

fn scoped_command(
    app: &mut App,
    scope: Scope,
    id: &'static str,
    notation: &str,
    description: &'static str,
    hint: Option<&'static str>,
) {
    app.commands
        .register(Command {
            id: CommandId(id),
            description,
            scope,
            hint,
            run: |_| {},
        })
        .unwrap();
    app.keymap
        .bind(Mode::Normal, scope, notation, CommandId(id))
        .unwrap();
}

#[test]
fn hints_come_from_the_registry_of_the_scope() {
    let mut app = App::new();
    let results = Scope("test_results");
    let history = Scope("test_history");
    scoped_command(&mut app, results, "r.detail", "d", "Detail", Some("detail"));
    scoped_command(&mut app, results, "r.split", "<CR>", "Split", Some("split"));
    scoped_command(&mut app, results, "r.top", "gg", "Top", None);
    scoped_command(&mut app, history, "h.edit", "e", "Edit", Some("edit"));
    assert_eq!(app.hints_for(results), ["d detail", "⏎ split"]);
    assert_eq!(app.hints_for(history), ["e edit"]);
    assert!(app.hints_for(Scope::GLOBAL).is_empty());
}

fn app_with_leader_commands() -> App {
    let mut app = App::new();
    test_command(&mut app, "test.one", "<leader>ab", "First");
    test_command(&mut app, "test.two", "<leader>c", "Second");
    app
}

#[tokio::test(start_paused = true)]
async fn which_key_appears_after_the_delay_and_lists_the_registry() {
    let mut app = app_with_leader_commands();
    app.handle_event(key(KeyCode::Char(' ')));
    app.handle_event(AppEvent::Tick);
    assert!(app.which_key_entries().is_empty());
    let deadline = app.next_deadline().unwrap();
    tokio::time::advance(deadline - Instant::now()).await;
    app.handle_event(AppEvent::Tick);
    let listed: Vec<(String, String)> = app
        .which_key_entries()
        .into_iter()
        .map(|e| (e.keys, e.description))
        .collect();
    let expected: Vec<(String, String)> = app
        .keymap
        .bindings()
        .filter(|b| b.keys.first() == Some(&LEADER))
        .map(|b| {
            (
                b.sequence.tail(1).unwrap().to_string(),
                app.commands.get(b.command).unwrap().description.to_owned(),
            )
        })
        .collect();
    assert_eq!(listed, expected);
    for test_key in ["ab", "c"] {
        assert!(
            listed.iter().any(|(keys, _)| keys == test_key),
            "{listed:?}"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn escape_closes_which_key() {
    let mut app = app_with_leader_commands();
    app.handle_event(key(KeyCode::Char(' ')));
    tokio::time::advance(crate::tui::widgets::which_key::WHICH_KEY_DELAY).await;
    app.handle_event(AppEvent::Tick);
    assert!(!app.which_key_entries().is_empty());
    app.handle_event(key(KeyCode::Esc));
    assert!(app.which_key_entries().is_empty());
    assert_eq!(app.next_deadline(), None);
}

#[tokio::test(start_paused = true)]
async fn renders_which_key_popup() {
    let mut app = app_with_leader_commands();
    app.handle_event(key(KeyCode::Char(' ')));
    tokio::time::advance(crate::tui::widgets::which_key::WHICH_KEY_DELAY).await;
    app.handle_event(AppEvent::Tick);
    app.handle_event(AppEvent::Resize(40, 10));
    insta::assert_snapshot!(render(&app));
}

fn sized(width: u16, height: u16) -> App {
    let mut app = App::new();
    app.handle_event(AppEvent::Resize(width, height));
    app
}

fn render(app: &App) -> TestBackend {
    let mut terminal = Terminal::new(TestBackend::new(app.area.width, app.area.height)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    terminal.backend().clone()
}

#[test]
fn number_keys_jump_between_panels() {
    let mut app = sized(112, 12);
    assert_eq!(app.focus(), Panel::Main);
    type_text(&mut app, "2");
    assert_eq!(app.focus(), Panel::History);
    assert_eq!(app.focused_scope(), HISTORY_SCOPE);
    type_text(&mut app, "0");
    assert_eq!(app.focus(), Panel::History, "0 is a no-op without a split");
    type_text(&mut app, "3");
    assert_eq!(app.focus(), Panel::Main);
    type_text(&mut app, "1");
    assert_eq!(app.focus(), Panel::SearchBar);
    assert_eq!(app.mode(), Mode::Insert);
    type_text(&mut app, "2");
    assert_eq!(
        app.focus(),
        Panel::SearchBar,
        "digits are text in insert mode"
    );
    assert_eq!(app.search_bar.query(), "2");
}

#[test]
fn history_opens_as_overlay_on_narrow_terminals() {
    let mut app = sized(99, 12);
    type_text(&mut app, "2");
    assert!(app.areas().overlay.is_some());
    app.handle_event(key(KeyCode::Esc));
    assert_eq!(app.focus(), Panel::Main);
    assert!(app.areas().overlay.is_none());
    type_text(&mut app, "23");
    assert!(
        app.areas().overlay.is_none(),
        "leaving the history closes the overlay"
    );
    let mut wide = sized(100, 12);
    type_text(&mut wide, "2");
    assert!(wide.areas().overlay.is_none());
}

fn press(app: &mut App, notation: &str) {
    let keys = crate::tui::keymap::sequence::KeySequence::parse(notation)
        .unwrap()
        .resolve(LEADER);
    for k in keys {
        let modifiers = KeyModifiers::NONE;
        app.handle_event(AppEvent::Key(KeyEvent::new(k.code(), modifiers)));
    }
}

#[test]
fn main_panel_keys_follow_the_transition_table() {
    let mut app = sized(112, 12);
    press(&mut app, "<CR>");
    assert_eq!(app.main_view(), MainView::Split);
    assert_eq!(app.focus(), Panel::Main, "results keep focus");
    press(&mut app, "<CR>");
    assert_eq!(app.main_view(), MainView::Split, "no-op");
    press(&mut app, "d");
    assert_eq!(app.main_view(), MainView::FullDetail);
    press(&mut app, "d");
    assert_eq!(app.main_view(), MainView::FullDetail, "no-op");
    press(&mut app, "<CR>");
    press(&mut app, "0");
    assert_eq!(app.focus(), Panel::Detail);
    press(&mut app, "q");
    assert_eq!(app.main_view(), MainView::Results);
    assert_eq!(app.focus(), Panel::Main);
    press(&mut app, "d");
    press(&mut app, "q");
    assert_eq!(app.main_view(), MainView::Results);
    assert!(!app.should_quit());
    press(&mut app, "q");
    assert!(app.should_quit(), "q quits only from the results");
}

#[test]
fn hints_follow_the_main_view() {
    let mut app = sized(112, 12);
    assert_eq!(
        app.hints_for(app.focused_scope()),
        ["d detail", "⏎ split", "q quit"]
    );
    press(&mut app, "<CR>");
    assert_eq!(app.hints_for(app.focused_scope()), ["d detail", "q back"]);
    press(&mut app, "d");
    assert_eq!(app.hints_for(app.focused_scope()), ["⏎ split", "q back"]);
}

fn rows(prefix: &str, count: usize) -> Vec<String> {
    (0..count).map(|i| format!("{prefix} {i:02}")).collect()
}

#[test]
fn slash_finds_a_match_beyond_the_visible_rows() {
    let mut app = sized(112, 12);
    let mut rows = rows("Card", 30);
    rows[20] = "Glorybringer".into();
    app.results.list_mut().set_rows(rows);
    let viewport = app.viewport(Panel::Main);
    assert!(viewport < 20, "the match starts off screen");
    press(&mut app, "/glory<CR>");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(app.results.list().cursor(), 20);
    assert!(app.results.list().visible_range(viewport).contains(&20));
    assert_eq!(app.message(), None);
}

#[test]
fn n_and_upper_n_wrap_around() {
    let mut app = sized(112, 12);
    app.results.list_mut().set_rows(vec![
        "Dragon A".into(),
        "Sol Ring".into(),
        "Dragon B".into(),
    ]);
    press(&mut app, "/dragon<CR>");
    assert_eq!(app.results.list().cursor(), 2);
    press(&mut app, "n");
    assert_eq!(app.results.list().cursor(), 0);
    assert!(
        app.message()
            .unwrap()
            .text
            .contains("continued at the start")
    );
    press(&mut app, "N");
    assert_eq!(app.results.list().cursor(), 2);
    assert!(app.message().unwrap().text.contains("continued at the end"));
}

#[test]
fn a_miss_keeps_the_cursor_and_says_so() {
    let mut app = sized(112, 12);
    app.results.list_mut().set_rows(rows("Card", 5));
    press(&mut app, "jj/zzz<CR>");
    assert_eq!(app.results.list().cursor(), 2);
    assert_eq!(app.message().unwrap().text, "Pattern not found: zzz");
}

#[test]
fn only_the_focused_panel_is_searched() {
    let mut app = sized(112, 12);
    app.results.list_mut().set_rows(rows("Card", 5));
    app.history
        .set_rows(vec!["t:dragon".into(), "o:draw".into()]);
    press(&mut app, "/draw<CR>");
    assert_eq!(app.message().unwrap().text, "Pattern not found: draw");
    press(&mut app, "2n");
    assert_eq!(app.history.cursor(), 1);
    assert_eq!(app.results.list().cursor(), 0);
}

#[test]
fn escape_cancels_the_prompt() {
    let mut app = sized(112, 12);
    app.results.list_mut().set_rows(rows("Card", 5));
    press(&mut app, "/Card<Space>03<Esc>");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(app.results.list().cursor(), 0);
    press(&mut app, "n");
    assert_eq!(app.message().unwrap().text, "No previous search pattern");
}

#[test]
fn motions_move_the_focused_list() {
    let mut app = sized(112, 12);
    app.results.list_mut().set_rows(rows("Card", 30));
    press(&mut app, "jjj");
    assert_eq!(app.results.list().cursor(), 3);
    press(&mut app, "G");
    assert_eq!(app.results.list().cursor(), 29);
    press(&mut app, "gg");
    assert_eq!(app.results.list().cursor(), 0);
}

#[test]
fn keymaps_picker_lists_every_command_and_runs_the_choice() {
    let mut app = sized(112, 20);
    press(&mut app, "<leader>sk");
    let picker = app.picker().expect("picker is open");
    let labels = picker.matching_labels();
    assert_eq!(labels.len(), app.commands().iter().count());
    for (label, command) in labels.iter().zip(app.commands().iter()) {
        assert!(label.ends_with(command.description), "{label}");
        for binding in app.keymap().bindings_for(command.id) {
            assert!(label.contains(&binding.sequence.to_string()), "{label}");
        }
    }
    type_text(&mut app, "focus history");
    assert!(
        app.picker()
            .unwrap()
            .selected_label()
            .unwrap()
            .contains("Focus the history")
    );
    app.handle_event(key(KeyCode::Enter));
    assert!(app.picker().is_none());
    assert_eq!(app.focus(), Panel::History);
    assert_eq!(app.mode(), Mode::Normal);
}

#[test]
fn renders_keymaps_picker() {
    let mut app = sized(112, 24);
    press(&mut app, "<leader>sk");
    insta::assert_snapshot!(render(&app));
}

#[test]
fn renders_split() {
    let mut app = sized(112, 8);
    press(&mut app, "<CR>");
    insta::assert_snapshot!(render(&app));
}

#[test]
fn renders_layout_at_the_threshold() {
    insta::assert_snapshot!(render(&sized(100, 10)));
}

#[test]
fn renders_layout_below_the_threshold() {
    insta::assert_snapshot!(render(&sized(99, 10)));
}

#[test]
fn renders_history_overlay() {
    let mut app = sized(99, 10);
    type_text(&mut app, "2");
    insta::assert_snapshot!(render(&app));
}
