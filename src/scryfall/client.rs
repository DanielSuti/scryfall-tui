//! Access to the Scryfall API with the required headers, through the request gateway.
//! See <https://scryfall.com/docs/api>.

use reqwest::{StatusCode, Url};
use serde::de::DeserializeOwned;

use crate::scryfall::error::ScryfallError;
use crate::scryfall::gateway::Gateway;
use crate::scryfall::wire::{WireCardList, WireError, WireRulingList};

pub use crate::scryfall::gateway::{BASE_URL, REQUEST_TIMEOUT, RateLimits, USER_AGENT};

/// The Accept header sent with every API request.
pub const ACCEPT_JSON: &str = "application/json";
/// The Accept header sent with every image request.
pub const ACCEPT_IMAGE: &str = "image/*";

/// The URLs of the API at one base URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiUrls {
    base_url: Url,
}

impl ApiUrls {
    /// The URLs of the API at `base_url`.
    pub fn new(base_url: Url) -> Self {
        Self { base_url }
    }

    /// Returns the URL of the first search page for `query`, sorted by Scryfall's `order`
    /// value, such as `name` or `released`, in the `dir` direction: `auto`, `asc` or `desc`.
    /// The query is sent unchanged.
    pub fn search_url(&self, query: &str, order: &str, dir: &str) -> Url {
        let mut url = self.base_url.clone();
        url.set_path("/cards/search");
        url.query_pairs_mut()
            .clear()
            .append_pair("q", query)
            .append_pair("order", order)
            .append_pair("dir", dir);
        url
    }

    /// Returns the URL of the rulings on the card with printing id `card_id`.
    ///
    /// # Panics
    /// Panics if the base URL cannot have a path, which no `http` URL lacks.
    pub fn rulings_url(&self, card_id: &str) -> Url {
        let mut url = self.base_url.clone();
        url.set_query(None);
        url.path_segments_mut()
            .expect("an http URL has path segments")
            .clear()
            .extend(["cards", card_id, "rulings"]);
        url
    }
}

/// A client for the Scryfall API and its card images. Every request goes through the
/// [`Gateway`], which carries the [`USER_AGENT`] header and keeps to Scryfall's rate limits;
/// the client asks for [`ACCEPT_JSON`] or, for images, [`ACCEPT_IMAGE`].
#[derive(Debug, Clone)]
pub struct ScryfallClient {
    urls: ApiUrls,
    gateway: Gateway,
}

impl ScryfallClient {
    /// Creates a client for the live API, through the session's one [`Gateway::live`].
    ///
    /// # Errors
    /// Returns [`ScryfallError::Transport`] when the gateway's HTTP client cannot be built.
    ///
    /// # Panics
    /// Panics if [`BASE_URL`] is not a valid URL, which is a programming error, or outside a
    /// Tokio runtime.
    pub fn new() -> Result<Self, ScryfallError> {
        let base_url = Url::parse(BASE_URL).expect("BASE_URL is valid");
        Ok(Self::with_base_url(base_url, Gateway::live()?))
    }

    /// Creates a client for the API at `base_url` sending through `gateway`, such as a test
    /// server with a gateway of its own.
    pub fn with_base_url(base_url: Url, gateway: Gateway) -> Self {
        Self {
            urls: ApiUrls::new(base_url),
            gateway,
        }
    }

    /// See [`ApiUrls::search_url`].
    pub fn search_url(&self, query: &str, order: &str, dir: &str) -> Url {
        self.urls.search_url(query, order, dir)
    }

    /// See [`ApiUrls::rulings_url`].
    pub fn rulings_url(&self, card_id: &str) -> Url {
        self.urls.rulings_url(card_id)
    }

    /// Fetches the card list at `url`, such as a search URL or a `next_page` URI, once the
    /// gateway lets it go.
    ///
    /// # Errors
    /// Returns [`ScryfallError::Api`] for a Scryfall Error object,
    /// [`ScryfallError::RateLimited`] for a 429, [`ScryfallError::Transport`] for network
    /// failures, [`ScryfallError::Decode`] for an unexpected body, and the gateway's
    /// [`ScryfallError::Refused`] or [`ScryfallError::GatewayStopped`].
    pub async fn get_card_list(&self, url: Url) -> Result<WireCardList, ScryfallError> {
        self.get(url).await
    }

    /// Fetches the rulings list at `url`, a [`ScryfallClient::rulings_url`]. It waits and
    /// fails as [`ScryfallClient::get_card_list`] does.
    ///
    /// # Errors
    /// See [`ScryfallClient::get_card_list`].
    pub async fn get_rulings(&self, url: Url) -> Result<WireRulingList, ScryfallError> {
        self.get(url).await
    }

    /// Fetches the image at `url`, an absolute image URI from a Card object, and returns its
    /// bytes. It waits as [`ScryfallClient::get_card_list`] does.
    ///
    /// # Errors
    /// Returns [`ScryfallError::Api`] or [`ScryfallError::Decode`] for a failed status, and
    /// the other errors as [`ScryfallClient::get_card_list`] does.
    pub async fn get_image(&self, url: Url) -> Result<Vec<u8>, ScryfallError> {
        let response = self.gateway.get(url, ACCEPT_IMAGE).await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response.bytes().await?.to_vec());
        }
        // The image servers answer most failures without an Error object.
        Err(match error_from_body(status, &response.text().await?) {
            ScryfallError::Decode { status, .. } => ScryfallError::Decode {
                status,
                message: "not an image".to_owned(),
            },
            err => err,
        })
    }

    async fn get<T: DeserializeOwned>(&self, url: Url) -> Result<T, ScryfallError> {
        let response = self.gateway.get(url, ACCEPT_JSON).await?;
        let status = response.status();
        let body = response.text().await?;
        if status.is_success() {
            return serde_json::from_str(&body).map_err(|err| ScryfallError::Decode {
                status: status.as_u16(),
                message: err.to_string(),
            });
        }
        Err(error_from_body(status, &body))
    }
}

/// Returns the error for a failed `status` with `body`: Scryfall's Error object when the body
/// is one, or [`ScryfallError::Decode`].
fn error_from_body(status: StatusCode, body: &str) -> ScryfallError {
    match serde_json::from_str::<WireError>(body) {
        Ok(error) => ScryfallError::Api {
            status: error.status,
            code: error.code,
            details: error.details,
            warnings: error.warnings.unwrap_or_default(),
        },
        Err(err) => ScryfallError::Decode {
            status: status.as_u16(),
            message: err.to_string(),
        },
    }
}
