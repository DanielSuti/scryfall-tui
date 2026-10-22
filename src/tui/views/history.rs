//! The search history panel.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::app::history::{History, HistoryEntry};
use crate::domain::card::MISSING;
use crate::tui::text::{display_width, truncate};
use crate::tui::widgets::item_list::{ItemList, Motion, MotionOutcome};
use crate::tui::widgets::pane_search::{Navigable, Searchable};

/// The history entries with the cursor.
#[derive(Debug, Clone, Default)]
pub struct HistoryView {
    entries: Vec<HistoryEntry>,
    list: ItemList,
}

/// Formats a result count compactly: exact below 1,000, then `2.1k` and `1.2M`. An unknown
/// count is [`MISSING`].
pub fn compact_count(total: Option<u32>) -> String {
    match total {
        None => MISSING.to_owned(),
        Some(n) if n < 1_000 => n.to_string(),
        Some(n) if n < 1_000_000 => format!("{:.1}k", f64::from(n) / 1e3),
        Some(n) => format!("{:.1}M", f64::from(n) / 1e6),
    }
}

impl HistoryView {
    /// Creates an empty view.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows the entries of `history`, keeping the cursor on its row index.
    pub fn show(&mut self, history: &History) {
        self.entries = history.entries().to_vec();
        self.list
            .set_rows(self.entries.iter().map(|e| e.query.clone()).collect());
    }

    /// Moves the cursor to the newest entry.
    pub fn select_first(&mut self) {
        self.list.select(0, 1);
    }

    /// Returns the entry under the cursor.
    pub fn selected(&self) -> Option<&HistoryEntry> {
        self.entries.get(self.list.cursor())
    }

    /// Returns the index of the entry under the cursor.
    pub fn cursor(&self) -> usize {
        self.list.cursor()
    }

    /// Draws the entries into `area`: the query on the left, cut with `...` when it doesn't
    /// fit, and the count on the right.
    pub fn render(&self, area: Rect, buf: &mut Buffer, focused: bool) {
        self.list.render(area, buf, focused, |index, width| {
            let entry = &self.entries[index];
            let count = compact_count(entry.total_cards);
            let query_width = width.saturating_sub(display_width(&count) + 1);
            let query = truncate(&entry.query, query_width);
            let fill = width.saturating_sub(display_width(&query) + display_width(&count));
            Line::from(format!("{query}{}{count}", " ".repeat(fill)))
        });
    }
}

impl Searchable for HistoryView {
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

impl Navigable for HistoryView {
    fn apply_motion(&mut self, motion: Motion, viewport: usize) -> MotionOutcome {
        self.list.apply_motion(motion, viewport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_compact() {
        assert_eq!(compact_count(None), "—");
        assert_eq!(compact_count(Some(57)), "57");
        assert_eq!(compact_count(Some(999)), "999");
        assert_eq!(compact_count(Some(2_145)), "2.1k");
        assert_eq!(compact_count(Some(18_832)), "18.8k");
        assert_eq!(compact_count(Some(1_250_000)), "1.2M");
    }
}
