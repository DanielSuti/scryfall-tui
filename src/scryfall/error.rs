//! Errors of the Scryfall client.

use std::time::Duration;

use thiserror::Error;

use crate::scryfall::parse::ParseError;

/// A failed Scryfall request.
#[derive(Debug, Error)]
pub enum ScryfallError {
    /// The request did not reach Scryfall or no response arrived.
    #[error("cannot reach Scryfall: {0}")]
    Transport(#[from] reqwest::Error),
    /// Scryfall answered with an Error object. `details` is Scryfall's own explanation.
    #[error("{details}")]
    Api {
        /// HTTP status.
        status: u16,
        /// Scryfall's machine-readable code, such as `not_found`.
        code: String,
        /// Scryfall's human-readable explanation, shown verbatim.
        details: String,
        /// Non-fatal warnings Scryfall attached.
        warnings: Vec<String>,
    },
    /// Scryfall answered 429 Too Many Requests. The client has paused all requests for
    /// `retry_after`.
    #[error("Scryfall rate limit reached, pausing requests for {} s", retry_after.as_secs())]
    RateLimited {
        /// How long requests are paused.
        retry_after: Duration,
    },
    /// The response body is not the JSON object Scryfall documents.
    #[error("unexpected Scryfall response (HTTP {status}): {message}")]
    Decode {
        /// HTTP status.
        status: u16,
        /// What failed to decode.
        message: String,
    },
    /// The request was for a host that isn't Scryfall's, so it was not sent.
    #[error("refusing to send a request to {host}, which is not a Scryfall server")]
    Refused {
        /// The host asked for.
        host: String,
    },
    /// The request gateway no longer runs, so nothing can be sent.
    #[error("the request gateway has stopped")]
    GatewayStopped,
    /// A card in the response could not be converted.
    #[error(transparent)]
    Parse(#[from] ParseError),
}
