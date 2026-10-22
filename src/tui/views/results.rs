//! The results list of the main panel.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::domain::card::Card;
use crate::domain::search_results::{LoadState, SearchResults};
use crate::search::SearchRequest;
use crate::tui::text::{display_width, truncate};
use crate::tui::widgets::item_list::{ItemList, Motion, MotionOutcome};
use crate::tui::widgets::pane_search::{Navigable, Searchable};

/// Below this many columns the type column is left out, as in the split.
pub const TYPES_COLUMN_MIN_WIDTH: usize = 60;
/// Columns between two cells.
const GAP: &str = "  ";

/// The cells of one row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    name: String,
    cost: String,
    types: String,
}

impl Row {
    fn of(card: &Card) -> Self {
        Self {
            name: card.display_name(),
            cost: card.display_cost(),
            types: card.display_types(),
        }
    }

    fn text(&self) -> String {
        format!("{}{GAP}{}{GAP}{}", self.name, self.cost, self.types)
    }
}

/// The rows of the displayed search, with the cursor.
#[derive(Debug, Clone, Default)]
pub struct ResultsView {
    request: Option<SearchRequest>,
    rows: Vec<Row>,
    list: ItemList,
    notice: Option<String>,
}

impl ResultsView {
    /// Creates an empty view.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows `results`, the results of `request`. The cursor moves to the top when the
    /// request differs from the one shown, and keeps its row when more pages of the same
    /// request arrive.
    pub fn show(&mut self, request: &SearchRequest, results: &SearchResults) {
        if self.request.as_ref() != Some(request) {
            self.request = Some(request.clone());
            self.list = ItemList::new();
        }
        self.rows = results.cards().iter().map(Row::of).collect();
        self.list
            .set_rows(self.rows.iter().map(Row::text).collect());
        self.notice = results.notice().map(str::to_owned);
    }

    /// Returns the request shown, or `None` before the first search.
    pub fn request(&self) -> Option<&SearchRequest> {
        self.request.as_ref()
    }

    /// Returns the list state.
    pub fn list(&self) -> &ItemList {
        &self.list
    }

    /// Returns the list state for updating.
    pub fn list_mut(&mut self) -> &mut ItemList {
        &mut self.list
    }

    /// Draws the visible rows into `area`, in columns of name, cost and card types. Cells that
    /// don't fit are cut with `...`. When nothing matched, the source's notice is drawn instead.
    pub fn render(&self, area: Rect, buf: &mut Buffer, focused: bool) {
        if self.rows.is_empty() {
            if let Some(notice) = &self.notice {
                let width = usize::from(area.width).saturating_sub(1);
                let line = Line::from(format!(" {}", truncate(notice, width)));
                buf.set_line(area.x, area.y, &line, area.width);
            }
            return;
        }
        let width = usize::from(area.width).saturating_sub(1);
        let widths = self.column_widths(width);
        self.list.render(area, buf, focused, |index, _| {
            let row = &self.rows[index];
            let mut cells = vec![
                pad(&truncate(&row.name, widths.name), widths.name),
                pad(&truncate(&row.cost, widths.cost), widths.cost),
            ];
            if widths.types > 0 {
                cells.push(truncate(&row.types, widths.types));
            }
            Line::from(cells.join(GAP))
        });
    }

    fn column_widths(&self, width: usize) -> Widths {
        let widest = |cell: fn(&Row) -> &str| {
            self.rows
                .iter()
                .map(|row| display_width(cell(row)))
                .max()
                .unwrap_or(0)
        };
        let gap = display_width(GAP);
        if width < TYPES_COLUMN_MIN_WIDTH {
            let cost = widest(|r| &r.cost).min(width / 3);
            let name = width.saturating_sub(cost + gap);
            return Widths {
                name,
                cost,
                types: 0,
            };
        }
        let cost = widest(|r| &r.cost).min(width / 4);
        let types = widest(|r| &r.types).min(width / 4);
        let name = width.saturating_sub(cost + types + 2 * gap);
        Widths { name, cost, types }
    }
}

struct Widths {
    name: usize,
    cost: usize,
    types: usize,
}

fn pad(text: &str, width: usize) -> String {
    let fill = width.saturating_sub(display_width(text));
    format!("{text}{}", " ".repeat(fill))
}

/// Formats `count` with thousands separators, such as `1,050`.
pub fn format_count(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Describes how much of `results` is loaded, such as `1,050 of 3,783 loaded`.
pub fn load_summary(results: &SearchResults) -> String {
    let loaded = format_count(results.cards().len());
    let total = results
        .total_cards()
        .map_or_else(|| "?".to_owned(), |t| format_count(t as usize));
    match results.state() {
        LoadState::Loading if results.pages_loaded() == 0 => "loading...".to_owned(),
        LoadState::Loading => format!("{loaded} of {total} loading..."),
        LoadState::Paused | LoadState::Complete => format!("{loaded} of {total} loaded"),
        LoadState::Failed(_) => format!("{loaded} of {total} loaded, failed"),
    }
}

impl Searchable for ResultsView {
    fn search_lines(&self) -> &[String] {
        self.list.search_lines()
    }

    fn current_line(&self) -> usize {
        self.list.current_line()
    }

    fn jump_to(&mut self, index: usize, viewport: usize) {
        self.list.jump_to(index, viewport);
    }
}

impl Navigable for ResultsView {
    fn apply_motion(&mut self, motion: Motion, viewport: usize) -> MotionOutcome {
        self.list.apply_motion(motion, viewport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1050), "1,050");
        assert_eq!(format_count(18832), "18,832");
        assert_eq!(format_count(1_234_567), "1,234,567");
    }
}
