//! HTTP access to the Scryfall API with the required headers and rate limits.
//! See <https://scryfall.com/docs/api> and <https://scryfall.com/docs/api/rate-limits>.

use std::time::Duration;

use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, RETRY_AFTER};
use reqwest::{StatusCode, Url};
use serde::de::DeserializeOwned;

use crate::net::throttle::Throttle;
use crate::scryfall::error::ScryfallError;
use crate::scryfall::wire::{WireCardList, WireError, WireRulingList};

/// The API's base URL.
pub const BASE_URL: &str = "https://api.scryfall.com";
/// The User-Agent Scryfall requires: the application name and version.
pub const USER_AGENT: &str = concat!("scry/", env!("CARGO_PKG_VERSION"));
/// The Accept header sent with every request.
pub const ACCEPT_JSON: &str = "application/json";
/// How long one request may take before it fails.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Paths limited to 2 requests per second; all other methods allow 10 per second.
const SEARCH_BUCKET_PATHS: &[&str] = &[
    "/cards/search",
    "/cards/named",
    "/cards/random",
    "/cards/collection",
];

/// Minimum intervals between request starts and the pause after a 429 response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimits {
    /// Interval for the search, named, random and collection endpoints.
    pub search: Duration,
    /// Interval for every other endpoint.
    pub other: Duration,
    /// Pause after a 429 response without a `Retry-After` header.
    pub rate_limited_pause: Duration,
}

impl RateLimits {
    /// Scryfall's documented limits: 2 requests per second for search-like endpoints,
    /// 10 per second for the rest, and access limited for 30 seconds after a 429.
    pub const SCRYFALL: RateLimits = RateLimits {
        search: Duration::from_millis(500),
        other: Duration::from_millis(100),
        rate_limited_pause: Duration::from_secs(30),
    };
}

/// A client for the Scryfall API. Every request goes through one throttle and carries the
/// [`USER_AGENT`] and [`ACCEPT_JSON`] headers.
#[derive(Debug)]
pub struct ScryfallClient {
    http: reqwest::Client,
    base_url: Url,
    limits: RateLimits,
    throttle: Throttle,
}

impl ScryfallClient {
    /// Creates a client for the live API with Scryfall's rate limits.
    ///
    /// # Errors
    /// Returns [`ScryfallError::Transport`] when the HTTP client cannot be built.
    ///
    /// # Panics
    /// Panics if [`BASE_URL`] is not a valid URL, which is a programming error.
    pub fn new() -> Result<Self, ScryfallError> {
        Self::with_base_url(
            Url::parse(BASE_URL).expect("BASE_URL is valid"),
            RateLimits::SCRYFALL,
        )
    }

    /// Creates a client for the API at `base_url` with `limits`.
    ///
    /// # Errors
    /// Returns [`ScryfallError::Transport`] when the HTTP client cannot be built.
    pub fn with_base_url(base_url: Url, limits: RateLimits) -> Result<Self, ScryfallError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static(ACCEPT_JSON));
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            http,
            base_url,
            limits,
            throttle: Throttle::new(),
        })
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

    /// Fetches the card list at `url`, such as a search URL or a `next_page` URI.
    ///
    /// The request waits for the throttle. A 429 response pauses every later request for the
    /// `Retry-After` time, or [`RateLimits::rate_limited_pause`] when the header is missing.
    ///
    /// # Errors
    /// Returns [`ScryfallError::Api`] for a Scryfall Error object,
    /// [`ScryfallError::RateLimited`] for a 429, [`ScryfallError::Transport`] for network
    /// failures and [`ScryfallError::Decode`] for an unexpected body.
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

    async fn get<T: DeserializeOwned>(&self, url: Url) -> Result<T, ScryfallError> {
        let (bucket, interval) = if SEARCH_BUCKET_PATHS.contains(&url.path()) {
            ("search", self.limits.search)
        } else {
            ("other", self.limits.other)
        };
        self.throttle.acquire(bucket, interval).await;
        let response = self.http.get(url).send().await?;
        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map_or(self.limits.rate_limited_pause, Duration::from_secs);
            self.throttle.pause(retry_after);
            return Err(ScryfallError::RateLimited { retry_after });
        }
        let body = response.text().await?;
        if status.is_success() {
            return serde_json::from_str(&body).map_err(|err| ScryfallError::Decode {
                status: status.as_u16(),
                message: err.to_string(),
            });
        }
        match serde_json::from_str::<WireError>(&body) {
            Ok(error) => Err(ScryfallError::Api {
                status: error.status,
                code: error.code,
                details: error.details,
                warnings: error.warnings.unwrap_or_default(),
            }),
            Err(err) => Err(ScryfallError::Decode {
                status: status.as_u16(),
                message: err.to_string(),
            }),
        }
    }
}
