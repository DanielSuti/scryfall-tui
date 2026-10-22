//! The card that commands act on.

use crate::domain::card::Card;

/// The card under the results cursor, with where it came from. Commands that act on "the
/// current card", such as the detail view, read it from here.
#[derive(Debug, Clone, Copy)]
pub struct Selection<'a> {
    /// The selected card.
    pub card: &'a Card,
    /// Its position in the results.
    pub index: usize,
    /// The query whose results it belongs to.
    pub query: &'a str,
}
