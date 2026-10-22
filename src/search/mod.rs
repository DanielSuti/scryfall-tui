//! Card search sources behind one contract.

use std::future::Future;
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
