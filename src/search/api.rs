//! Search through the Scryfall API.

use reqwest::Url;

use crate::domain::search_results::{PageToken, SearchPage};
use crate::scryfall::client::ScryfallClient;
use crate::scryfall::error::ScryfallError;
use crate::scryfall::parse::card_from_wire;
use crate::scryfall::wire::WireCardList;
use crate::search::{BoxFuture, CardSearch, SearchError, SearchRequest, SortField, SortOrder};

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

/// The fields whose `dir=auto` sorts descending on Scryfall. For every other field `auto`
/// sorts ascending. Scryfall documents `auto` only as "the most intuitive direction"; this
/// list was measured on 2026-10-23, and the ignored live test `every_order_reverses` checks it.
pub const AUTO_IS_DESCENDING: &[SortField] = &[
    SortField::Released,
    SortField::Rarity,
    SortField::Usd,
    SortField::Eur,
    SortField::Tix,
];

/// Returns Scryfall's `order` value for `field`. See
/// <https://scryfall.com/docs/api/cards/search>.
pub fn scryfall_order(field: SortField) -> &'static str {
    match field {
        SortField::Name => "name",
        SortField::Set => "set",
        SortField::Released => "released",
        SortField::Rarity => "rarity",
        SortField::Color => "color",
        SortField::ManaValue => "cmc",
        SortField::Power => "power",
        SortField::Toughness => "toughness",
        SortField::Usd => "usd",
        SortField::Eur => "eur",
        SortField::Tix => "tix",
        SortField::Edhrec => "edhrec",
        SortField::Penny => "penny",
        SortField::Artist => "artist",
        SortField::Review => "review",
    }
}

/// Returns Scryfall's `dir` value for `order`: `auto` for the default direction, and the
/// opposite of what `auto` does for the field when reversed.
pub fn scryfall_dir(order: SortOrder) -> &'static str {
    match (order.reversed, AUTO_IS_DESCENDING.contains(&order.field)) {
        (false, _) => "auto",
        (true, true) => "asc",
        (true, false) => "desc",
    }
}

impl CardSearch for ApiSearch {
    fn first_page<'a>(
        &'a self,
        request: &'a SearchRequest,
    ) -> BoxFuture<'a, Result<SearchPage, SearchError>> {
        let url = self.client.search_url(
            &request.effective_query(),
            scryfall_order(request.order.field),
            scryfall_dir(request.order),
        );
        Box::pin(self.fetch(url))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reversing_sends_the_opposite_of_auto() {
        for field in SortField::ALL {
            assert_eq!(scryfall_dir(SortOrder::new(field, false)), "auto");
            let expected = if AUTO_IS_DESCENDING.contains(&field) {
                "asc"
            } else {
                "desc"
            };
            assert_eq!(
                scryfall_dir(SortOrder::new(field, true)),
                expected,
                "{field:?}"
            );
        }
        assert_eq!(
            scryfall_dir(SortOrder::new(SortField::Released, true)),
            "asc"
        );
        assert_eq!(scryfall_dir(SortOrder::new(SortField::Name, true)), "desc");
    }

    #[test]
    fn every_field_has_its_own_order_value() {
        let mut values: Vec<&str> = SortField::ALL.iter().map(|f| scryfall_order(*f)).collect();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values.len(), SortField::ALL.len());
    }
}
