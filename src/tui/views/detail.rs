//! The detail of one card: every face's text, then the chosen formats' legalities.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::domain::card::{Card, Face};
use crate::tui::text::{display_width, truncate, wrap};
use crate::tui::theme::Theme;
use crate::tui::widgets::item_list::{Motion, MotionOutcome};
use crate::tui::widgets::pane_search::{
    Navigable, PaneSearch, Searchable, draw_position, highlight,
};

/// From this width on the legalities are laid out in two columns.
pub const LEGALITY_TWO_COLUMN_MIN_WIDTH: usize = 50;
/// Columns reserved for a format name in the legalities.
const FORMAT_WIDTH: usize = 10;
/// Columns reserved for a status in the legalities.
const STATUS_WIDTH: usize = 10;

/// Formats `card` into lines of at most `width` columns: for each face its name and cost,
/// its full type line and stats, its oracle text and its flavor text, with faces separated
/// by a `//` line; then the legality of each of `formats` the card reports, in that order.
pub fn format_card(card: &Card, formats: &[String], width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, face) in card.faces().iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
            lines.push("//".to_owned());
            lines.push(String::new());
        }
        format_face(face, width, &mut lines);
    }
    let legalities: Vec<String> = formats
        .iter()
        .filter_map(|format| {
            let status = card.legalities().get(format)?;
            Some(format!(
                "{:FORMAT_WIDTH$} {:STATUS_WIDTH$}",
                truncate(format, FORMAT_WIDTH),
                status.to_string()
            ))
        })
        .collect();
    if !legalities.is_empty() {
        lines.push(String::new());
        if width >= LEGALITY_TWO_COLUMN_MIN_WIDTH {
            let rows = legalities.len().div_ceil(2);
            for row in 0..rows {
                let left = &legalities[row];
                match legalities.get(row + rows) {
                    Some(right) => lines.push(format!("{left}   {right}").trim_end().to_owned()),
                    None => lines.push(left.trim_end().to_owned()),
                }
            }
        } else {
            lines.extend(legalities.iter().map(|l| l.trim_end().to_owned()));
        }
    }
    lines
        .into_iter()
        .map(|line| truncate(&line, width))
        .collect()
}

fn format_face(face: &Face, width: usize, lines: &mut Vec<String>) {
    lines.push(spread(&face.name, face.display_cost(), width));
    let type_line = face.type_line.as_ref().map_or("—", |t| t.full());
    lines.push(spread(type_line, &face.stats().unwrap_or_default(), width));
    if let Some(text) = &face.oracle_text {
        lines.push(String::new());
        lines.extend(wrap(text, width));
    }
    if let Some(flavor) = &face.flavor_text {
        lines.push(String::new());
        lines.extend(wrap(flavor, width));
    }
}

/// Puts `left` at the start and `right` at the end of a line of `width` columns. When both
/// don't fit, `left` is cut with `...`.
fn spread(left: &str, right: &str, width: usize) -> String {
    if right.is_empty() {
        return truncate(left, width);
    }
    let right_width = display_width(right);
    let left = truncate(left, width.saturating_sub(right_width + 1));
    let gap = width
        .saturating_sub(display_width(&left) + right_width)
        .max(1);
    format!("{left}{}{right}", " ".repeat(gap))
}

/// The detail lines of the selected card and the scroll position.
#[derive(Debug, Clone, Default)]
pub struct DetailView {
    card_id: Option<String>,
    lines: Vec<String>,
    scroll: usize,
}

impl DetailView {
    /// Creates an empty view.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows `card` formatted for `width` columns. The scroll goes back to the top when the
    /// card changes.
    pub fn show(&mut self, card: Option<&Card>, formats: &[String], width: usize) {
        let id = card.map(|c| c.id().to_owned());
        if id != self.card_id {
            self.scroll = 0;
            self.card_id = id;
        }
        self.lines = card.map_or_else(Vec::new, |c| format_card(c, formats, width));
        self.scroll = self.scroll.min(self.lines.len().saturating_sub(1));
    }

    /// Returns the formatted lines.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Returns the index of the top visible line.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Draws the lines from the scroll position into `area`, each indented by one column, with
    /// the matches of `search` in `theme.find_keyword`. The top line, the current one, shows its
    /// position among the matching lines at its right end.
    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        search: Option<&PaneSearch<'_>>,
    ) {
        if self.lines.is_empty() {
            buf.set_line(
                area.x + 1,
                area.y,
                &Line::from("No card selected"),
                area.width,
            );
            return;
        }
        for (row, line) in self.lines.iter().skip(self.scroll).enumerate() {
            let Ok(row) = u16::try_from(row) else { break };
            if row >= area.height {
                break;
            }
            let mut text = Line::from(line.clone());
            if let Some(search) = search {
                text = highlight(text, search.pattern, theme.find_keyword);
            }
            buf.set_line(
                area.x + 1,
                area.y + row,
                &text,
                area.width.saturating_sub(1),
            );
            if let (0, Some(search)) = (row, search) {
                let line_area = Rect {
                    y: area.y,
                    height: 1,
                    ..area
                };
                draw_position(buf, line_area, search, theme.find_position);
            }
        }
    }
}

impl Searchable for DetailView {
    fn search_lines(&self) -> &[String] {
        &self.lines
    }

    fn current_line(&self) -> usize {
        self.scroll
    }

    fn jump_to(&mut self, index: usize, _viewport: usize) {
        self.scroll = index.min(self.lines.len().saturating_sub(1));
    }
}

impl Navigable for DetailView {
    fn apply_motion(&mut self, motion: Motion, viewport: usize) -> MotionOutcome {
        let last = self.lines.len().saturating_sub(viewport.max(1));
        let half = (viewport / 2).max(1);
        self.scroll = match motion {
            Motion::Down => (self.scroll + 1).min(last),
            Motion::Up => self.scroll.saturating_sub(1),
            Motion::Top => 0,
            Motion::Bottom => last,
            Motion::HalfPageDown => (self.scroll + half).min(last),
            Motion::HalfPageUp => self.scroll.saturating_sub(half),
        };
        MotionOutcome::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scryfall::parse::parse_card;

    fn fixture(name: &str) -> Card {
        let path = format!(
            "{}/tests/fixtures/scryfall/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        parse_card(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn formats(names: &[&str]) -> Vec<String> {
        names.iter().map(|f| (*f).to_owned()).collect()
    }

    #[test]
    fn single_face_shows_text_flavor_and_legalities() {
        let card = fixture("normal_glorybringer");
        let lines = format_card(&card, &formats(&["standard", "commander"]), 40);
        assert_eq!(
            lines[0],
            format!("Glorybringer{}{{3}}{{R}}{{R}}", " ".repeat(19))
        );
        assert_eq!(lines[1], format!("Creature — Dragon{}4/4", " ".repeat(20)));
        assert_eq!(lines[3], "Flying, haste");
        let flavor = card.faces()[0].flavor_text.clone().unwrap();
        let first_flavor_word = flavor.split_whitespace().next().unwrap();
        assert!(
            lines.iter().any(|l| l.starts_with(first_flavor_word)),
            "{lines:#?}"
        );
        assert_eq!(
            &lines[lines.len() - 2..],
            ["standard   not legal", "commander  legal"]
        );
        assert!(lines.iter().all(|l| display_width(l) <= 40));
    }

    #[test]
    fn faces_are_separated_and_each_has_its_own_block() {
        let card = fixture("transform_delver");
        let lines = format_card(&card, &[], 40);
        let separator = lines.iter().position(|l| l == "//").unwrap();
        assert!(lines[0].starts_with("Delver of Secrets") && lines[0].ends_with("{U}"));
        assert!(lines[1].starts_with("Creature — Human Wizard") && lines[1].ends_with("1/1"));
        let back = &lines[separator + 2];
        assert!(
            back.starts_with("Insectile Aberration") && back.ends_with("—"),
            "{back}"
        );
        assert!(lines[separator + 3].ends_with("3/2"));
    }

    #[test]
    fn split_and_adventure_cards_show_both_halves() {
        for (name, halves) in [
            ("split_fire_ice", ["Fire", "Ice"]),
            ("adventure_bonecrusher", ["Bonecrusher Giant", "Stomp"]),
        ] {
            let lines = format_card(&fixture(name), &[], 60);
            for half in halves {
                assert!(
                    lines.iter().any(|l| l.starts_with(half)),
                    "{name}: {lines:#?}"
                );
            }
        }
    }

    #[test]
    fn legalities_use_two_columns_when_wide() {
        let card = fixture("normal_glorybringer");
        let chosen = formats(&["standard", "pioneer", "modern"]);
        let wide = format_card(&card, &chosen, 60);
        assert_eq!(
            &wide[wide.len() - 2..],
            [
                "standard   not legal    modern     legal",
                "pioneer    legal"
            ]
        );
        let unknown = format_card(&card, &formats(&["bogus"]), 60);
        assert!(!unknown.iter().any(|l| l.contains("bogus")));
    }

    #[test]
    fn scrolling_stays_within_the_text() {
        let mut view = DetailView::new();
        let card = fixture("normal_glorybringer");
        view.show(Some(&card), &[], 20);
        let viewport = 3;
        view.apply_motion(Motion::Bottom, viewport);
        assert_eq!(view.scroll(), view.lines().len() - viewport);
        view.apply_motion(Motion::Down, viewport);
        assert_eq!(view.scroll(), view.lines().len() - viewport);
        view.apply_motion(Motion::Top, viewport);
        assert_eq!(view.scroll(), 0);
    }
}
