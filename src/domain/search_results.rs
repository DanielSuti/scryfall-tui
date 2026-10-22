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
