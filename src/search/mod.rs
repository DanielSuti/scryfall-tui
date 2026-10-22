//! Card search sources behind one contract.

use std::future::Future;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::time::Duration;

use thiserror::Error;

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

/// The order in which a search returns its cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SortOrder {
    /// By name, A to Z.
    #[default]
    Name,
    /// By release date, newest first.
    Released,
}

impl SortOrder {
    /// Every order, in the order a picker lists them.
    pub const ALL: [SortOrder; 2] = [SortOrder::Name, SortOrder::Released];

    /// Returns the name shown in the sort picker.
    pub fn label(self) -> &'static str {
        match self {
            SortOrder::Name => "Name (A → Z)",
            SortOrder::Released => "Release date (newest first)",
        }
    }

    /// Returns the short description shown in the status line, such as `by name`.
    pub fn summary(self) -> &'static str {
        match self {
            SortOrder::Name => "by name",
            SortOrder::Released => "by release date",
        }
    }
}

/// A query and the order its results come in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchRequest {
    /// The query, sent to the source unchanged.
    pub query: String,
    /// The order of the results.
    pub order: SortOrder,
}

impl SearchRequest {
    /// Creates a request for `query` in `order`.
    pub fn new(query: impl Into<String>, order: SortOrder) -> Self {
        Self {
            query: query.into(),
            order,
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
