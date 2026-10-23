//! `DummyScryfall`: a local server that answers with responses recorded once from the live
//! Scryfall API, so tests that need real Scryfall data never call the API.
//!
//! Every request a test may send is listed in [`recorded_requests`]. The ignored test
//! `record_scryfall_answers` in `tests/scryfall_recorded.rs` sends each of them to the live
//! API once and saves the answers in [`RECORDINGS_DIR`]. Every other run replays the saved
//! answers. A request without a recording gets a 501 Error object naming the request, so a
//! missing recording fails loudly instead of reaching the network.
//!
//! To keep the recordings small, a card list keeps its first [`RECORDED_CARDS`] cards, and each
//! card keeps only [`KEPT_CARD_FIELDS`]. Totals, `has_more` and `next_page` are kept as
//! recorded; `next_page` points at the live API and must not be followed in tests.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use reqwest::Url;
use scry::scryfall::client::{BASE_URL, RateLimits, ScryfallClient};
use scry::search::api::{scryfall_dir, scryfall_order};
use scry::search::{SortField, SortOrder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// Where the recorded answers live, one JSON file per request.
pub const RECORDINGS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/recordings/scryfall");
/// Cards kept from each recorded card list.
pub const RECORDED_CARDS: usize = 10;
/// The card fields kept in recordings: those `scry` parses, plus the ones the sort orders use.
pub const KEPT_CARD_FIELDS: &[&str] = &[
    "object",
    "id",
    "oracle_id",
    "layout",
    "name",
    "mana_cost",
    "cmc",
    "type_line",
    "oracle_text",
    "flavor_text",
    "power",
    "toughness",
    "loyalty",
    "defense",
    "colors",
    "legalities",
    "card_faces",
    "set",
    "collector_number",
    "rarity",
    "released_at",
    "prices",
    "edhrec_rank",
    "penny_rank",
    "artist",
];
/// The query whose recordings check every sort order. Its matches differ in every field
/// Scryfall sorts by, so each order and its reverse start with different cards.
pub const ORDER_QUERY: &str = "t:dragon";
/// The printing id of the Glorybringer card in `search_glorybringer`, whose rulings are
/// recorded.
pub const GLORYBRINGER_ID: &str = "06f90d62-6d21-47b1-a427-eb25a42f4dcb";
/// The command that refreshes the recordings, quoted in error messages.
pub const RECORD_COMMAND: &str =
    "cargo test --test scryfall_recorded -- --ignored record_scryfall_answers";

/// One recorded answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    /// The request path and query, exactly as `scry` sends it.
    pub request: String,
    /// The HTTP status Scryfall answered with.
    pub status: u16,
    /// The answer, trimmed as described in the module documentation.
    pub body: Value,
}

/// Returns the path and query of `url`, the key recordings are stored under.
pub fn request_key(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    }
}

/// Every request the recorded tests send, with the file name its answer is saved under.
pub fn recorded_requests() -> Vec<(String, Url)> {
    let client =
        ScryfallClient::with_base_url(Url::parse(BASE_URL).unwrap(), RateLimits::SCRYFALL).unwrap();
    let mut requests = vec![
        (
            "catalog_supertypes".to_owned(),
            Url::parse(&format!("{BASE_URL}/catalog/supertypes")).unwrap(),
        ),
        (
            "search_glorybringer".to_owned(),
            client.search_url("!\"Glorybringer\"", "name", "auto"),
        ),
        (
            "search_no_match".to_owned(),
            client.search_url("zzzqqqnotacard", "name", "auto"),
        ),
        (
            "rulings_glorybringer".to_owned(),
            client.rulings_url(GLORYBRINGER_ID),
        ),
    ];
    for field in SortField::ALL {
        for reversed in [false, true] {
            let order = SortOrder::new(field, reversed);
            let (order, dir) = (scryfall_order(field), scryfall_dir(order));
            requests.push((
                format!("order_{order}_{dir}"),
                client.search_url(ORDER_QUERY, order, dir),
            ));
        }
    }
    requests
}

/// Trims a recorded answer as described in the module documentation.
pub fn trim(mut body: Value) -> Value {
    if let Some(cards) = body.get_mut("data").and_then(Value::as_array_mut)
        && cards.first().is_some_and(|c| c["object"] == "card")
    {
        cards.truncate(RECORDED_CARDS);
        for card in cards.iter_mut() {
            if let Some(fields) = card.as_object_mut() {
                fields.retain(|key, _| KEPT_CARD_FIELDS.contains(&key.as_str()));
            }
        }
    }
    body
}

/// The replaying server. Clones share their list of requests.
#[derive(Debug, Clone)]
pub struct DummyScryfall {
    answers: Arc<HashMap<String, Recording>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl DummyScryfall {
    /// Loads every recording and starts a server answering from them.
    ///
    /// # Panics
    /// Panics when no recording exists, with the command that creates them.
    pub async fn start() -> (MockServer, DummyScryfall) {
        let mut answers = HashMap::new();
        for entry in std::fs::read_dir(RECORDINGS_DIR).unwrap_or_else(|_| {
            panic!("no recordings in {RECORDINGS_DIR}; create them with `{RECORD_COMMAND}`")
        }) {
            let path = entry.unwrap().path();
            let recording: Recording =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            answers.insert(recording.request.clone(), recording);
        }
        let dummy = DummyScryfall {
            answers: Arc::new(answers),
            calls: Arc::new(Mutex::new(Vec::new())),
        };
        let server = MockServer::start().await;
        Mock::given(any())
            .respond_with(dummy.clone())
            .mount(&server)
            .await;
        (server, dummy)
    }

    /// Returns the recorded answer to `request`, a path and query: the recorded status and
    /// body, or a 501 Error object when nothing was recorded for it.
    pub fn call(&self, request: &str) -> ResponseTemplate {
        self.calls.lock().unwrap().push(request.to_owned());
        match self.answers.get(request) {
            Some(recording) => ResponseTemplate::new(recording.status).set_body_json(&recording.body),
            None => ResponseTemplate::new(501).set_body_json(json!({
                "object": "error",
                "status": 501,
                "code": "not_recorded",
                "details": format!("no recorded answer for {request}; record it with `{RECORD_COMMAND}`"),
            })),
        }
    }

    /// Returns the requests received so far, in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    /// Returns the received requests that had no recording.
    pub fn unrecorded_calls(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter(|c| !self.answers.contains_key(c))
            .collect()
    }
}

impl Respond for DummyScryfall {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        self.call(&request_key(&request.url))
    }
}
