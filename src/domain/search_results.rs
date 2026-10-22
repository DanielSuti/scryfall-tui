//! Pages of search results.

use crate::domain::card::Card;

/// An opaque handle on the next page of a search, issued by the search that produced the
/// previous page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageToken(pub String);

/// One page of search results.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchPage {
    /// The cards on this page, in the order the source returned them.
    pub cards: Vec<Card>,
    /// Matching cards across all pages, or `None` when the source does not say.
    pub total_cards: Option<u32>,
    /// The next page, or `None` on the last page.
    pub next: Option<PageToken>,
    /// Non-fatal warnings about the query, shown verbatim.
    pub warnings: Vec<String>,
    /// The source's explanation when nothing matched.
    pub notice: Option<String>,
}

/// Where a search stands in loading its pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    /// A batch is loading.
    Loading,
    /// A batch finished and more pages exist.
    Paused,
    /// Every page is loaded.
    Complete,
    /// A request failed; the message explains why.
    Failed(String),
}

/// All pages loaded so far for one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResults {
    query: String,
    cards: Vec<Card>,
    total_cards: Option<u32>,
    next: Option<PageToken>,
    warnings: Vec<String>,
    notice: Option<String>,
    pages_loaded: usize,
    state: LoadState,
}

impl SearchResults {
    /// Creates empty results for `query`, with the first batch loading.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            cards: Vec::new(),
            total_cards: None,
            next: None,
            warnings: Vec::new(),
            notice: None,
            pages_loaded: 0,
            state: LoadState::Loading,
        }
    }

    /// Adds a page: its cards are appended, and its total, next token and notice replace the
    /// previous ones. Warnings are kept once each.
    pub fn apply(&mut self, page: SearchPage) {
        self.cards.extend(page.cards);
        self.total_cards = page.total_cards.or(self.total_cards);
        self.next = page.next;
        for warning in page.warnings {
            if !self.warnings.contains(&warning) {
                self.warnings.push(warning);
            }
        }
        self.notice = page.notice;
        self.pages_loaded += 1;
    }

    /// Marks the running batch as finished: paused when more pages exist, complete otherwise.
    /// A failed search stays failed.
    pub fn finish_batch(&mut self) {
        if matches!(self.state, LoadState::Loading) {
            self.state = if self.next.is_some() {
                LoadState::Paused
            } else {
                LoadState::Complete
            };
        }
    }

    /// Marks the search as failed with `message`.
    pub fn fail(&mut self, message: impl Into<String>) {
        self.state = LoadState::Failed(message.into());
    }

    /// Starts the next batch if loading is paused, and returns the token to load from.
    /// Returns `None` while a batch is loading, after the last page, or after a failure.
    pub fn resume(&mut self) -> Option<PageToken> {
        if self.state != LoadState::Paused {
            return None;
        }
        let token = self.next.clone()?;
        self.state = LoadState::Loading;
        Some(token)
    }

    /// Returns the query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the cards loaded so far, in order.
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    /// Returns the total number of matching cards, or `None` before the first page.
    pub fn total_cards(&self) -> Option<u32> {
        self.total_cards
    }

    /// Returns the warnings about the query.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Returns the source's explanation when nothing matched.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Returns the number of pages loaded.
    pub fn pages_loaded(&self) -> usize {
        self.pages_loaded
    }

    /// Returns the load state.
    pub fn state(&self) -> &LoadState {
        &self.state
    }

    /// Returns true when more pages exist than are loaded.
    pub fn has_more(&self) -> bool {
        self.next.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::card::Face;
    use crate::domain::legalities::Legalities;

    fn card(name: &str) -> Card {
        let face = Face {
            name: name.into(),
            ..Face::default()
        };
        Card::new(name, None, "normal", vec![face], Legalities::default())
    }

    fn page(names: &[&str], total: u32, next: Option<&str>) -> SearchPage {
        SearchPage {
            cards: names.iter().map(|n| card(n)).collect(),
            total_cards: Some(total),
            next: next.map(|n| PageToken(n.into())),
            warnings: vec!["w".into()],
            notice: None,
        }
    }

    #[test]
    fn pages_accumulate_and_batches_pause_or_complete() {
        let mut results = SearchResults::new("dragon");
        results.apply(page(&["A", "B"], 3, Some("p2")));
        assert_eq!(results.resume(), None, "no resume while loading");
        results.finish_batch();
        assert_eq!(results.state(), &LoadState::Paused);
        assert_eq!(results.resume(), Some(PageToken("p2".into())));
        assert_eq!(results.state(), &LoadState::Loading);
        results.apply(page(&["C"], 3, None));
        results.finish_batch();
        assert_eq!(results.state(), &LoadState::Complete);
        assert_eq!(results.resume(), None, "nothing after the last page");
        let names: Vec<String> = results.cards().iter().map(Card::display_name).collect();
        assert_eq!(names, ["A", "B", "C"]);
        assert_eq!(results.total_cards(), Some(3));
        assert_eq!(results.warnings(), ["w"]);
        assert_eq!(results.pages_loaded(), 2);
    }

    #[test]
    fn failure_is_kept() {
        let mut results = SearchResults::new("dragon");
        results.apply(page(&["A"], 3, Some("p2")));
        results.fail("boom");
        results.finish_batch();
        assert_eq!(results.state(), &LoadState::Failed("boom".into()));
        assert_eq!(results.resume(), None);
    }
}
