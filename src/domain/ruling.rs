//! Rulings: official notes on how a card works.

/// One ruling on a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ruling {
    /// When it was published, an ISO date such as `2017-04-18`.
    pub published_at: String,
    /// The ruling's text.
    pub comment: String,
}

/// What is known of one card's rulings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulingsState {
    /// They are being fetched, or will be shortly.
    Loading,
    /// They arrived. An empty list means the card has none.
    Loaded(Vec<Ruling>),
    /// Fetching them failed with this message.
    Failed(String),
}
