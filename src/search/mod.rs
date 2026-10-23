//! Card search and rulings sources behind one contract each.

use std::future::Future;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::time::Duration;

use thiserror::Error;

use crate::domain::ruling::Ruling;
use crate::domain::search_results::{PageToken, SearchPage};
use crate::scryfall::error::ScryfallError;

pub mod api;

/// A boxed future, so that [`CardSearch`] can be used as a trait object.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A failed search request.
#[derive(Debug, Error)]
pub enum SearchError {
    /// The Scryfall API request failed.
    #[error(transparent)]
    Scryfall(#[from] ScryfallError),
    /// A page token was not issued by this search.
    #[error("invalid page token `{0}`")]
    InvalidPageToken(String),
}

impl SearchError {
    /// Returns how long requests are paused when the source rate-limited the request, or
    /// `None` for every other error.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            SearchError::Scryfall(ScryfallError::RateLimited { retry_after }) => Some(*retry_after),
            _ => None,
        }
    }
}

/// What a search sorts its cards by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SortField {
    /// Card name.
    #[default]
    Name,
    /// Set code and collector number.
    Set,
    /// Release date.
    Released,
    /// Rarity.
    Rarity,
    /// Color and color identity.
    Color,
    /// Mana value.
    ManaValue,
    /// Power.
    Power,
    /// Toughness.
    Toughness,
    /// Price in US dollars.
    Usd,
    /// Price in euros.
    Eur,
    /// Price in MTGO event tickets.
    Tix,
    /// EDHREC popularity rank.
    Edhrec,
    /// Penny Dreadful popularity rank.
    Penny,
    /// Artist name.
    Artist,
    /// The order set reviews use: color and mana value, with special printings last.
    Review,
}

impl SortField {
    /// Every field, in the order the sort window lists them.
    pub const ALL: [SortField; 15] = [
        SortField::Name,
        SortField::Set,
        SortField::Released,
        SortField::Rarity,
        SortField::Color,
        SortField::ManaValue,
        SortField::Power,
        SortField::Toughness,
        SortField::Usd,
        SortField::Eur,
        SortField::Tix,
        SortField::Edhrec,
        SortField::Penny,
        SortField::Artist,
        SortField::Review,
    ];

    /// Returns the field's name, its name inside a sentence, and the direction of its default
    /// and reversed orders.
    fn texts(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            SortField::Name => ("Name", "name", "A → Z", "Z → A"),
            SortField::Set => ("Set", "set", "A → Z", "Z → A"),
            SortField::Released => (
                "Release date",
                "release date",
                "newest first",
                "oldest first",
            ),
            SortField::Rarity => ("Rarity", "rarity", "mythic first", "common first"),
            SortField::Color => ("Color", "color", "WUBRG first", "colorless first"),
            SortField::ManaValue => ("Mana value", "mana value", "lowest first", "highest first"),
            SortField::Power => ("Power", "power", "lowest first", "highest first"),
            SortField::Toughness => ("Toughness", "toughness", "lowest first", "highest first"),
            SortField::Usd => ("USD price", "USD price", "highest first", "lowest first"),
            SortField::Eur => ("EUR price", "EUR price", "highest first", "lowest first"),
            SortField::Tix => ("TIX price", "TIX price", "highest first", "lowest first"),
            SortField::Edhrec => (
                "EDHREC rank",
                "EDHREC rank",
                "most played first",
                "least played first",
            ),
            SortField::Penny => (
                "Penny rank",
                "Penny rank",
                "most played first",
                "least played first",
            ),
            SortField::Artist => ("Artist", "artist", "A → Z", "Z → A"),
            SortField::Review => (
                "Set review",
                "set review",
                "review order",
                "reverse review order",
            ),
        }
    }

    /// Returns the field's name, such as `Release date`.
    pub fn label(self) -> &'static str {
        self.texts().0
    }
}

/// The order in which a search returns its cards: a field, in its default direction or
/// reversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SortOrder {
    /// What the cards are sorted by.
    pub field: SortField,
    /// True for the opposite of the field's default direction.
    pub reversed: bool,
}

impl SortOrder {
    /// Creates an order by `field`, reversed when `reversed` is true.
    pub fn new(field: SortField, reversed: bool) -> Self {
        Self { field, reversed }
    }

    /// Returns the direction, such as `newest first`.
    pub fn direction(self) -> &'static str {
        let (_, _, default, reversed) = self.field.texts();
        if self.reversed { reversed } else { default }
    }

    /// Returns a description such as `Release date: newest first`.
    pub fn description(self) -> String {
        format!("{}: {}", self.field.label(), self.direction())
    }

    /// Returns the status-line summary, such as `by release date, newest first`.
    pub fn summary(self) -> String {
        format!("by {}, {}", self.field.texts().1, self.direction())
    }
}

/// A query and the order its results come in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchRequest {
    /// The query as the user typed it.
    pub query: String,
    /// The order of the results.
    pub order: SortOrder,
    /// Terms added to every query, such as `game:paper`, or empty for none.
    pub base: String,
}

impl SearchRequest {
    /// Creates a request for `query` in `order`, without base terms.
    pub fn new(query: impl Into<String>, order: SortOrder) -> Self {
        Self {
            query: query.into(),
            order,
            base: String::new(),
        }
    }

    /// Adds `base` to the request's query.
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    /// Returns the query to send: the typed query in parentheses followed by the base terms,
    /// so an `or` in the typed query keeps its meaning. Without base terms, or with only
    /// whitespace, the typed query is returned unchanged.
    pub fn effective_query(&self) -> String {
        match self.base.trim() {
            "" => self.query.clone(),
            base => format!("({}) {base}", self.query),
        }
    }
}

/// A source of card search results, delivered one page at a time.
pub trait CardSearch: Send + Sync {
    /// Fetches the first page of results for `request`.
    fn first_page<'a>(
        &'a self,
        request: &'a SearchRequest,
    ) -> BoxFuture<'a, Result<SearchPage, SearchError>>;

    /// Fetches the page that `token` points to.
    fn next_page<'a>(
        &'a self,
        token: &'a PageToken,
    ) -> BoxFuture<'a, Result<SearchPage, SearchError>>;
}

/// A source of the rulings on a card.
pub trait CardRulings: Send + Sync {
    /// Fetches the rulings on the card with printing id `card_id`.
    fn rulings<'a>(&'a self, card_id: &'a str) -> BoxFuture<'a, Result<Vec<Ruling>, SearchError>>;
}

/// How many times one page request is retried after the source rate-limited it.
pub const MAX_RATE_LIMIT_RETRIES: usize = 2;

/// Where a batch starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchStart {
    /// The first page of a new search.
    Query(SearchRequest),
    /// The page a previous batch stopped before.
    Next(PageToken),
}

/// Progress of a batch, reported as it happens.
#[derive(Debug)]
pub enum BatchEvent {
    /// A page arrived.
    Page(SearchPage),
    /// The source rate-limited the request. It is retried once the pause has passed.
    RateLimited(Duration),
    /// A request failed. The batch stops.
    Failed(SearchError),
}

/// Loads up to `max_pages` pages from `start`, following each page's token, and reports each
/// page, rate-limit pause and failure to `on_event`. A rate-limited request is retried up to
/// [`MAX_RATE_LIMIT_RETRIES`] times; the source's own throttle does the waiting.
///
/// Returns the token of the page after the batch, or `None` when the last page was loaded or
/// a request failed.
pub async fn load_batch(
    search: &dyn CardSearch,
    start: BatchStart,
    max_pages: NonZeroU32,
    mut on_event: impl FnMut(BatchEvent) + Send,
) -> Option<PageToken> {
    let mut next = start;
    for _ in 0..max_pages.get() {
        let mut retries = 0;
        let page = loop {
            let result = match &next {
                BatchStart::Query(request) => search.first_page(request).await,
                BatchStart::Next(token) => search.next_page(token).await,
            };
            match result {
                Ok(page) => break page,
                Err(err) => match err.retry_after() {
                    Some(pause) if retries < MAX_RATE_LIMIT_RETRIES => {
                        retries += 1;
                        on_event(BatchEvent::RateLimited(pause));
                    }
                    _ => {
                        on_event(BatchEvent::Failed(err));
                        return None;
                    }
                },
            }
        };
        let token = page.next.clone();
        on_event(BatchEvent::Page(page));
        next = BatchStart::Next(token?);
    }
    match next {
        BatchStart::Next(token) => Some(token),
        BatchStart::Query(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_terms_follow_the_parenthesised_query() {
        let request = SearchRequest::new("t:goblin or t:elf", SortOrder::default());
        assert_eq!(request.effective_query(), "t:goblin or t:elf");
        let based = request.clone().with_base("game:paper -is:funny");
        assert_eq!(
            based.effective_query(),
            "(t:goblin or t:elf) game:paper -is:funny"
        );
        assert_eq!(
            request.with_base("  ").effective_query(),
            "t:goblin or t:elf"
        );
    }
}
