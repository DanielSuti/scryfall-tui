//! Rulings: official notes on how a card works.

/// One ruling on a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ruling {
    /// When it was published, an ISO date such as `2017-04-18`.
    pub published_at: String,
    /// The ruling's text.
    pub comment: String,
}
