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

/// A source of card search results, delivered one page at a time.
pub trait CardSearch: Send + Sync {
    /// Fetches the first page of results for `query`.
    fn first_page<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchPage, SearchError>>;

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
    Query(String),
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
                BatchStart::Query(query) => search.first_page(query).await,
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
