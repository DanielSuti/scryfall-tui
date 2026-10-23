//! A generic fuzzy picker: type to filter, move with `<C-j>`/`<C-k>`, choose with `<CR>`.

use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Clear};

use crate::tui::text::{display_width, truncate};
use crate::tui::theme::Theme;
use crate::tui::widgets::item_list::{ItemList, Motion};
use crate::tui::widgets::query_input::QueryInput;

/// The result of a key press in the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerOutcome<T> {
    /// The picker stays open.
    Open,
    /// The user chose this item.
    Selected(T),
    /// The user closed the picker without choosing.
    Cancelled,
}

/// An item label together with its position in the item list.
struct Labeled<'a>(usize, &'a str);

impl AsRef<str> for Labeled<'_> {
    fn as_ref(&self) -> &str {
        self.1
    }
}

/// A popup listing labelled items, filtered by fuzzy matching against the typed query.
pub struct Picker<T> {
    title: String,
    items: Vec<(String, T)>,
    input: QueryInput,
    matcher: Matcher,
    matches: Vec<usize>,
    list: ItemList,
}

impl<T> fmt::Debug for Picker<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Picker")
            .field("title", &self.title)
            .field("query", &self.input.value())
            .field("matches", &self.matches.len())
            .finish_non_exhaustive()
    }
}

/// Number of rows the popup moves by for `<C-j>`/`<C-k>`; the list itself scrolls as needed.
const PICKER_VIEWPORT: usize = 20;
/// The popup is at most this percentage of the screen's width.
pub const MAX_WIDTH_PERCENT: u16 = 60;
/// The popup is at most this percentage of the screen's height.
pub const MAX_HEIGHT_PERCENT: u16 = 70;
/// The popup is at least this many columns wide, so the query has room.
const MIN_WIDTH: usize = 24;

impl<T: Clone> Picker<T> {
    /// Creates a picker titled `title` over `items`, each a label and a value.
    pub fn new(title: impl Into<String>, items: Vec<(String, T)>) -> Self {
        let mut picker = Self {
            title: title.into(),
            items,
            input: QueryInput::new(),
            matcher: Matcher::new(Config::DEFAULT),
            matches: Vec::new(),
            list: ItemList::new(),
        };
        picker.refilter();
        picker
    }

    /// Returns the typed query.
    pub fn query(&self) -> &str {
        self.input.value()
    }

    /// Returns the labels of the matching items, best match first. An empty query matches
    /// every item, in the original order.
    pub fn matching_labels(&self) -> Vec<&str> {
        self.matches
            .iter()
            .map(|i| self.items[*i].0.as_str())
            .collect()
    }

    /// Returns the label under the cursor, or `None` when nothing matches.
    pub fn selected_label(&self) -> Option<&str> {
        let index = *self.matches.get(self.list.cursor())?;
        Some(self.items[index].0.as_str())
    }

    fn refilter(&mut self) {
        let pattern = Pattern::parse(self.query(), CaseMatching::Smart, Normalization::Smart);
        let labeled = self
            .items
            .iter()
            .enumerate()
            .map(|(i, (label, _))| Labeled(i, label.as_str()));
        self.matches = pattern
            .match_list(labeled, &mut self.matcher)
            .into_iter()
            .map(|(Labeled(i, _), _)| i)
            .collect();
        let rows = self
            .matching_labels()
            .into_iter()
            .map(str::to_owned)
            .collect();
        self.list.set_rows(rows);
        self.list.select(0, PICKER_VIEWPORT);
    }

    /// Applies a key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> PickerOutcome<T> {
        if key.kind == KeyEventKind::Release {
            return PickerOutcome::Open;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => PickerOutcome::Cancelled,
            KeyCode::Enter => match self.matches.get(self.list.cursor()) {
                Some(index) => PickerOutcome::Selected(self.items[*index].1.clone()),
                None => PickerOutcome::Open,
            },
            KeyCode::Char('j') if ctrl => self.motion(Motion::Down),
            KeyCode::Char('k') if ctrl => self.motion(Motion::Up),
            KeyCode::Down => self.motion(Motion::Down),
            KeyCode::Up => self.motion(Motion::Up),
            _ => {
                if self.input.handle_key(key) {
                    self.refilter();
                }
                PickerOutcome::Open
            }
        }
    }

    fn motion(&mut self, motion: Motion) -> PickerOutcome<T> {
        self.list.apply(motion, PICKER_VIEWPORT);
        PickerOutcome::Open
    }

    /// Draws the picker centred in `area`, sized to its items: as wide as the longest label
    /// and as tall as the item list, up to [`MAX_WIDTH_PERCENT`] and [`MAX_HEIGHT_PERCENT`]
    /// of `area`. The border is drawn in `theme.focus_border`, since the picker holds the keys.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let label_width = self
            .items
            .iter()
            .map(|(label, _)| display_width(label))
            .chain([display_width(&self.title) + 4, MIN_WIDTH])
            .max()
            .unwrap_or(MIN_WIDTH);
        // Borders, the cursor marker and a space on each side.
        let wanted_width = u16::try_from(label_width + 5).unwrap_or(u16::MAX);
        // Borders and the query line.
        let wanted_height = u16::try_from(self.items.len() + 3).unwrap_or(u16::MAX);
        let width = wanted_width.min(area.width * MAX_WIDTH_PERCENT / 100);
        let height = wanted_height.min(area.height * MAX_HEIGHT_PERCENT / 100);
        let [popup] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(area);
        let [popup] = Layout::vertical([Constraint::Length(height)])
            .flex(Flex::Center)
            .areas(popup);
        frame.render_widget(Clear, popup);
        let block = Block::bordered()
            .border_type(BorderType::Thick)
            .border_style(theme.focus_border)
            .title_style(theme.focus_border)
            .title(format!(" {} ", self.title))
            .title_bottom(format!(" {}/{} ", self.matches.len(), self.items.len()));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let [query, list] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
        frame.render_widget(ratatui::widgets::Paragraph::new("> "), query);
        let input = Rect {
            x: query.x + 2,
            width: query.width.saturating_sub(2),
            ..query
        };
        self.input.render(frame, input, true);
        self.list
            .render(list, frame.buffer_mut(), true, |index, width| {
                Line::from(truncate(&self.list.rows()[index], width))
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picker() -> Picker<u32> {
        Picker::new(
            "Test",
            vec![
                ("Quit scry".into(), 1),
                ("Focus the search bar".into(), 2),
                ("Search the focused panel".into(), 3),
                ("Next match".into(), 4),
            ],
        )
    }

    fn press(
        picker: &mut Picker<u32>,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> PickerOutcome<u32> {
        picker.handle_key(KeyEvent::new(code, modifiers))
    }

    fn type_text(picker: &mut Picker<u32>, text: &str) {
        for c in text.chars() {
            press(picker, KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    #[test]
    fn empty_query_shows_every_item_in_order() {
        let picker = picker();
        assert_eq!(
            picker.matching_labels(),
            [
                "Quit scry",
                "Focus the search bar",
                "Search the focused panel",
                "Next match"
            ]
        );
    }

    #[test]
    fn ranking_follows_the_matcher_scores() {
        let mut picker = picker();
        type_text(&mut picker, "search");
        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse("search", CaseMatching::Smart, Normalization::Smart);
        let labels = [
            "Quit scry",
            "Focus the search bar",
            "Search the focused panel",
            "Next match",
        ];
        let expected: Vec<&str> = pattern
            .match_list(labels, &mut matcher)
            .into_iter()
            .map(|(label, _)| label)
            .collect();
        assert_eq!(picker.matching_labels(), expected);
        assert_eq!(expected.len(), 2);
        assert!(!picker.matching_labels().contains(&"Quit scry"));
    }

    #[test]
    fn enter_returns_the_item_under_the_cursor() {
        let mut picker = picker();
        press(&mut picker, KeyCode::Char('j'), KeyModifiers::CONTROL);
        press(&mut picker, KeyCode::Char('j'), KeyModifiers::CONTROL);
        press(&mut picker, KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(picker.selected_label(), Some("Focus the search bar"));
        assert_eq!(
            press(&mut picker, KeyCode::Enter, KeyModifiers::NONE),
            PickerOutcome::Selected(2)
        );
    }

    #[test]
    fn escape_returns_nothing_and_no_match_keeps_it_open() {
        let mut picker = picker();
        type_text(&mut picker, "zzzz");
        assert!(picker.matching_labels().is_empty());
        assert_eq!(
            press(&mut picker, KeyCode::Enter, KeyModifiers::NONE),
            PickerOutcome::Open
        );
        assert_eq!(
            press(&mut picker, KeyCode::Esc, KeyModifiers::NONE),
            PickerOutcome::Cancelled
        );
    }
}
