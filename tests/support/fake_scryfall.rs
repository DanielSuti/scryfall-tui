//! A stateful fake of the Scryfall search API.
//!
//! The fake holds a list of cards and answers `GET /cards/search` from it. It supports a
//! documented subset of the query language: every whitespace-separated term must appear in
//! the card name, ignoring case. A term containing `:` is ignored with a warning, as Scryfall
//! warns about expressions it cannot use. Like Scryfall, results are sorted by name unless
//! `order=released` asks for newest first; another `order` value is a 400 error. Pages, totals,
//! `next_page`, no-match errors and 429 responses are all derived from the fake's configured
//! state.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// The details Scryfall sends when a search matches nothing.
pub const NOT_FOUND_DETAILS: &str = "Your query didn’t match any cards. Adjust your search terms or refer to the syntax guide at https://scryfall.com/docs/reference";

/// A request the fake received.
#[derive(Debug, Clone)]
pub struct Received {
    /// Path and query.
    pub path_and_query: String,
    /// The User-Agent header, if any.
    pub user_agent: Option<String>,
    /// The Accept header, if any.
    pub accept: Option<String>,
}

#[derive(Debug)]
struct State {
    cards: Vec<Value>,
    page_size: usize,
    min_interval: Option<Duration>,
    fail_page: Option<usize>,
    last_request: Option<Instant>,
    received: Vec<Received>,
    base_url: String,
}

/// The fake. Clones share state.
#[derive(Debug, Clone)]
pub struct FakeScryfall {
    state: Arc<Mutex<State>>,
}

/// The recorded Card object `tests/fixtures/scryfall/<name>.json`.
pub fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/scryfall/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A real Card object recorded from Scryfall, used as the template for generated cards.
pub fn template_card() -> Value {
    fixture("normal_glorybringer")
}

/// A single-faced creature card named `name`, built from [`template_card`].
pub fn card_named(name: &str) -> Value {
    let mut card = template_card();
    card["name"] = json!(name);
    card["id"] = json!(format!("id-{}", name.to_lowercase().replace(' ', "-")));
    card
}

/// A card named `name` first released on `released_at`, an ISO date such as `2024-11-15`.
pub fn card_released(name: &str, released_at: &str) -> Value {
    let mut card = card_named(name);
    card["released_at"] = json!(released_at);
    card
}

impl FakeScryfall {
    /// Starts a mock server answering from `cards`, with Scryfall's page size of 175.
    pub async fn start(cards: Vec<Value>) -> (MockServer, FakeScryfall) {
        let server = MockServer::start().await;
        let fake = FakeScryfall {
            state: Arc::new(Mutex::new(State {
                cards,
                page_size: 175,
                min_interval: None,
                fail_page: None,
                last_request: None,
                received: Vec::new(),
                base_url: server.uri(),
            })),
        };
        Mock::given(method("GET"))
            .and(path("/cards/search"))
            .respond_with(fake.clone())
            .mount(&server)
            .await;
        (server, fake)
    }

    /// Sets the number of cards per page.
    pub fn set_page_size(&self, page_size: usize) {
        self.state.lock().unwrap().page_size = page_size;
    }

    /// Answers 429 when a request arrives less than `interval` after the previous one.
    pub fn set_min_interval(&self, interval: Duration) {
        self.state.lock().unwrap().min_interval = Some(interval);
    }

    /// Answers page `page` (1-based) with a 500 Error object.
    pub fn fail_on_page(&self, page: usize) {
        self.state.lock().unwrap().fail_page = Some(page);
    }

    /// Returns the requests received so far.
    pub fn received(&self) -> Vec<Received> {
        self.state.lock().unwrap().received.clone()
    }

    /// Returns the number of search requests received so far.
    pub fn request_count(&self) -> usize {
        self.state.lock().unwrap().received.len()
    }
}

fn error(status: u16, code: &str, details: &str) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(json!({
        "object": "error",
        "status": status,
        "code": code,
        "details": details,
    }))
}

impl Respond for FakeScryfall {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let mut state = self.state.lock().unwrap();
        let header = |name: &str| {
            request
                .headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        state.received.push(Received {
            path_and_query: format!(
                "{}?{}",
                request.url.path(),
                request.url.query().unwrap_or_default()
            ),
            user_agent: header("user-agent"),
            accept: header("accept"),
        });
        let now = Instant::now();
        let too_fast = matches!(
            (state.min_interval, state.last_request),
            (Some(min), Some(last)) if now - last < min
        );
        state.last_request = Some(now);
        if too_fast {
            return error(
                429,
                "too_many_requests",
                "You are sending requests too quickly.",
            );
        }

        let param = |key: &str| {
            request
                .url
                .query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
        };
        let query = param("q").unwrap_or_default();
        let page: usize = param("page").and_then(|p| p.parse().ok()).unwrap_or(1);
        if state.fail_page == Some(page) {
            return error(500, "internal_error", "Something went wrong on our end.");
        }

        let mut warnings = Vec::new();
        let mut terms = Vec::new();
        for term in query.split_whitespace() {
            if term.contains(':') {
                warnings.push(format!("Invalid expression “{term}” was ignored."));
            } else {
                terms.push(term.to_lowercase());
            }
        }
        let order = param("order").unwrap_or_else(|| "name".to_owned());
        let text = |card: &Value, key: &str| card[key].as_str().unwrap_or_default().to_owned();
        let mut matching: Vec<&Value> = state
            .cards
            .iter()
            .filter(|card| {
                let name = text(card, "name").to_lowercase();
                terms.iter().all(|t| name.contains(t))
            })
            .collect();
        match order.as_str() {
            "name" => matching.sort_by_key(|card| text(card, "name").to_lowercase()),
            "released" => matching.sort_by(|a, b| {
                text(b, "released_at")
                    .cmp(&text(a, "released_at"))
                    .then_with(|| text(a, "name").cmp(&text(b, "name")))
            }),
            other => {
                return error(400, "bad_request", &format!("Unknown order `{other}`."));
            }
        }
        if matching.is_empty() {
            return error(404, "not_found", NOT_FOUND_DETAILS);
        }
        let start = (page - 1) * state.page_size;
        let data: Vec<&Value> = matching
            .iter()
            .skip(start)
            .take(state.page_size)
            .copied()
            .collect();
        let has_more = start + data.len() < matching.len();
        let next_page = has_more.then(|| {
            let mut url = reqwest::Url::parse(&state.base_url).unwrap();
            url.set_path("/cards/search");
            url.query_pairs_mut()
                .append_pair("q", &query)
                .append_pair("order", &order)
                .append_pair("page", &(page + 1).to_string());
            url.to_string()
        });
        let mut body = json!({
            "object": "list",
            "total_cards": matching.len(),
            "has_more": has_more,
            "next_page": next_page,
            "data": data,
        });
        if !warnings.is_empty() {
            body["warnings"] = json!(warnings);
        }
        ResponseTemplate::new(200).set_body_json(body)
    }
}
