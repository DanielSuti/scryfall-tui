//! The results list of the main panel.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::domain::card::Card;
use crate::domain::search_results::SearchResults;
use crate::tui::text::truncate;
use crate::tui::widgets::item_list::{ItemList, Motion, MotionOutcome};
use crate::tui::widgets::pane_search::{Navigable, Searchable};

/// The rows of the displayed search, with the cursor.
#[derive(Debug, Clone, Default)]
pub struct ResultsView {
    query: Option<String>,
    list: ItemList,
}

/// Returns the text of a card's row, which `/` searches.
pub fn row_text(card: &Card) -> String {
    format!(
        "{}  {}  {}",
        card.display_name(),
        card.display_cost(),
        card.display_types()
    )
}

impl ResultsView {
    /// Creates an empty view.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows `results`. The cursor moves to the top when the query differs from the one shown,
    /// and keeps its row when more pages of the same query arrive.
    pub fn show(&mut self, results: &SearchResults) {
        if self.query.as_deref() != Some(results.query()) {
            self.query = Some(results.query().to_owned());
            self.list = ItemList::new();
        }
        self.list
            .set_rows(results.cards().iter().map(row_text).collect());
    }

    /// Returns the query shown, or `None` before the first search.
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// Returns the list state.
    pub fn list(&self) -> &ItemList {
        &self.list
    }

    /// Returns the list state for updating.
    pub fn list_mut(&mut self) -> &mut ItemList {
        &mut self.list
    }

    /// Draws the visible rows into `area`.
    pub fn render(&self, area: Rect, buf: &mut Buffer, focused: bool) {
        self.list.render(area, buf, focused, |index, width| {
            Line::from(truncate(&self.list.rows()[index], width))
        });
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
