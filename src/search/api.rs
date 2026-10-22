//! Search through the Scryfall API.

use reqwest::Url;

use crate::domain::search_results::{PageToken, SearchPage};
use crate::scryfall::client::ScryfallClient;
use crate::scryfall::error::ScryfallError;
use crate::scryfall::parse::card_from_wire;
use crate::scryfall::wire::WireCardList;
use crate::search::{BoxFuture, CardSearch, SearchError};

/// Scryfall's error code for a search that matches nothing.
const NOT_FOUND: &str = "not_found";

/// Searches with Scryfall's full query syntax by sending the query to the API unchanged.
#[derive(Debug)]
pub struct ApiSearch {
    client: ScryfallClient,
}

impl ApiSearch {
    /// Creates a search over `client`.
    pub fn new(client: ScryfallClient) -> Self {
        Self { client }
    }

    async fn fetch(&self, url: Url) -> Result<SearchPage, SearchError> {
        match self.client.get_card_list(url).await {
            Ok(list) => page_from_wire(list),
            Err(ScryfallError::Api {
                code,
                details,
                warnings,
                ..
            }) if code == NOT_FOUND => Ok(SearchPage {
                total_cards: Some(0),
                warnings,
                notice: Some(details),
                ..SearchPage::default()
            }),
            Err(err) => Err(err.into()),
        }
    }
}

fn page_from_wire(list: WireCardList) -> Result<SearchPage, SearchError> {
    let cards = list
        .data
        .into_iter()
        .map(card_from_wire)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ScryfallError::from)?;
    Ok(SearchPage {
        cards,
        total_cards: list.total_cards,
        next: list.next_page.filter(|_| list.has_more).map(PageToken),
        warnings: list.warnings.unwrap_or_default(),
        notice: None,
    })
}

impl CardSearch for ApiSearch {
    fn first_page<'a>(&'a self, query: &'a str) -> BoxFuture<'a, Result<SearchPage, SearchError>> {
        Box::pin(self.fetch(self.client.search_url(query)))
    }

    fn next_page<'a>(
        &'a self,
        token: &'a PageToken,
    ) -> BoxFuture<'a, Result<SearchPage, SearchError>> {
        Box::pin(async move {
            let url =
                Url::parse(&token.0).map_err(|_| SearchError::InvalidPageToken(token.0.clone()))?;
            self.fetch(url).await
        })
    }
}
