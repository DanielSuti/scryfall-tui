//! A stateful fake of the Scryfall search and rulings API and of its image servers.
//!
//! The fake holds a list of cards and answers `GET /cards/search` from it. `GET
//! /cards/<id>/rulings` answers the rulings set for that card, none by default, and a 404
//! Error object for an id none of the cards has. Every `image_uris.normal` it serves, on the
//! card and on its faces, points at `/images/` on a second server standing in for Scryfall's
//! file servers, which answers [`CARD_IMAGE_FIXTURE`] for every image, so no test reaches
//! Scryfall's image servers. [`FakeScryfall::client`] gives a client whose gateway allows only
//! these two servers. It supports a
//! documented subset of the query language: every whitespace-separated term must appear in
//! the card name, ignoring case. Parentheses around terms are dropped. A term containing `:` is ignored with a warning, as Scryfall
//! warns about expressions it cannot use.
//!
//! Sorting models two of Scryfall's orders with their measured `dir=auto` behaviour: `name`
//! (auto and `asc` give A → Z, `desc` Z → A) and `released` (auto and `desc` give newest
//! first, `asc` oldest first). Without `order` the fake sorts by name, as Scryfall does. Any
//! other `order` or `dir` value is a 400 error saying the fake does not model it.
//!
//! Pages, totals, `next_page`, no-match errors and 429 responses are all derived from the
//! fake's configured state.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use scry::scryfall::client::{RateLimits, ScryfallClient};
use scry::scryfall::gateway::{Gateway, Policy};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// The details Scryfall sends when a search matches nothing.
pub const NOT_FOUND_DETAILS: &str = "Your query didn’t match any cards. Adjust your search terms or refer to the syntax guide at https://scryfall.com/docs/reference";

/// The PNG every image request is answered with, 49×68 pixels.
pub const CARD_IMAGE_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/images/card.png"
);

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
    rulings: HashMap<String, Vec<Value>>,
    failing_rulings: Vec<String>,
    rulings_received: Vec<Received>,
    failing_images: HashMap<String, u16>,
    images_received: Vec<Received>,
    base_url: String,
    files_url: String,
}

/// The fake. Clones share state.
#[derive(Debug, Clone)]
pub struct FakeScryfall {
    state: Arc<Mutex<State>>,
    files: Arc<MockServer>,
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

/// A single-faced creature card named `name`, built from [`template_card`]. Its printing id is
/// `id-` and its oracle id `oracle-` followed by the name in lowercase with `-` for spaces,
/// such as `id-sol-ring`.
pub fn card_named(name: &str) -> Value {
    let slug = name.to_lowercase().replace(' ', "-");
    let mut card = template_card();
    card["name"] = json!(name);
    card["id"] = json!(format!("id-{slug}"));
    card["oracle_id"] = json!(format!("oracle-{slug}"));
    card
}

/// A Ruling object published on `published_at`, an ISO date, saying `comment`.
pub fn ruling(published_at: &str, comment: &str) -> Value {
    json!({
        "object": "ruling",
        "source": "wotc",
        "published_at": published_at,
        "comment": comment,
    })
}

/// A card named `name` first released on `released_at`, an ISO date such as `2024-11-15`.
pub fn card_released(name: &str, released_at: &str) -> Value {
    let mut card = card_named(name);
    card["released_at"] = json!(released_at);
    card
}

impl FakeScryfall {
    /// Starts a mock server answering from `cards`, with Scryfall's page size of 175, and the
    /// file server its image URIs point at, which lives as long as the fake.
    pub async fn start(cards: Vec<Value>) -> (MockServer, FakeScryfall) {
        let server = MockServer::start().await;
        let files = MockServer::start().await;
        let fake = FakeScryfall {
            state: Arc::new(Mutex::new(State {
                cards,
                page_size: 175,
                min_interval: None,
                fail_page: None,
                last_request: None,
                received: Vec::new(),
                rulings: HashMap::new(),
                failing_rulings: Vec::new(),
                rulings_received: Vec::new(),
                failing_images: HashMap::new(),
                images_received: Vec::new(),
                base_url: server.uri(),
                files_url: files.uri(),
            })),
            files: Arc::new(files),
        };
        Mock::given(method("GET"))
            .and(path("/cards/search"))
            .respond_with(fake.clone())
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/cards/[^/]+/rulings$"))
            .respond_with(FakeRulings(fake.clone()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/images/[^/]+\.png$"))
            .respond_with(FakeImages(fake.clone()))
            .mount(&fake.files)
            .await;
        (server, fake)
    }

    /// Returns a client for this fake with `limits`, through a gateway of its own that sends
    /// only to the fake's API and file servers.
    pub fn client(&self, limits: RateLimits) -> ScryfallClient {
        let (api, files) = {
            let state = self.state.lock().unwrap();
            (
                state.base_url.parse().unwrap(),
                state.files_url.parse().unwrap(),
            )
        };
        let gateway = Gateway::start(Policy::local(&api, &files), limits).unwrap();
        ScryfallClient::with_base_url(api, gateway)
    }

    /// Returns a client for an API at `base_url` with `limits`, such as an address nothing
    /// listens on, through a gateway that sends only there.
    pub fn client_at(base_url: &str, limits: RateLimits) -> ScryfallClient {
        let url: reqwest::Url = base_url.parse().unwrap();
        let gateway = Gateway::start(Policy::local(&url, &url), limits).unwrap();
        ScryfallClient::with_base_url(url, gateway)
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

    /// Answers the rulings request for the card with id `card_id` with `rulings`.
    pub fn set_rulings(&self, card_id: &str, rulings: Vec<Value>) {
        self.state
            .lock()
            .unwrap()
            .rulings
            .insert(card_id.to_owned(), rulings);
    }

    /// Answers the rulings request for the card with id `card_id` with a 500 Error object, or
    /// normally again when `failing` is false.
    pub fn fail_rulings(&self, card_id: &str, failing: bool) {
        let mut state = self.state.lock().unwrap();
        state.failing_rulings.retain(|id| id != card_id);
        if failing {
            state.failing_rulings.push(card_id.to_owned());
        }
    }

    /// Returns the rulings requests received so far.
    pub fn rulings_received(&self) -> Vec<Received> {
        self.state.lock().unwrap().rulings_received.clone()
    }

    /// Answers the image request for the card with id `card_id` with `status`, or normally
    /// again when `status` is `None`. A 429 carries no `Retry-After`, so the client pauses for
    /// its own `rate_limited_pause`.
    pub fn fail_image(&self, card_id: &str, status: Option<u16>) {
        let mut state = self.state.lock().unwrap();
        match status {
            Some(status) => state.failing_images.insert(card_id.to_owned(), status),
            None => state.failing_images.remove(card_id),
        };
    }

    /// Returns the image requests received so far.
    pub fn images_received(&self) -> Vec<Received> {
        self.state.lock().unwrap().images_received.clone()
    }

    /// Returns the URL the fake serves the front image of the card with id `card_id` at.
    pub fn image_url(&self, card_id: &str) -> String {
        image_url(&self.state.lock().unwrap().files_url, card_id)
    }

    /// Returns the number of search requests received so far.
    pub fn request_count(&self) -> usize {
        self.state.lock().unwrap().received.len()
    }
}

fn received(request: &Request) -> Received {
    let header = |name: &str| {
        request
            .headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    Received {
        path_and_query: format!(
            "{}?{}",
            request.url.path(),
            request.url.query().unwrap_or_default()
        ),
        user_agent: header("user-agent"),
        accept: header("accept"),
    }
}

/// The URL of image `name` on the fake's file server at `files_url`.
fn image_url(files_url: &str, name: &str) -> String {
    format!("{files_url}/images/{name}.png")
}

/// Returns `card` with its image URIs pointing at the fake's file server at `files_url`: the
/// card's at
/// `<id>.png`, and on a card with one image per face, the first face's there too and every
/// other face's at `<id>-face<n>.png`.
fn with_local_images(card: &Value, files_url: &str) -> Value {
    let mut card = card.clone();
    let id = card["id"].as_str().unwrap_or_default().to_owned();
    if card["image_uris"].is_object() {
        card["image_uris"]["normal"] = json!(image_url(files_url, &id));
    }
    if let Some(faces) = card["card_faces"].as_array_mut() {
        for (index, face) in faces.iter_mut().enumerate() {
            if face["image_uris"].is_object() {
                let name = match index {
                    0 => id.clone(),
                    n => format!("{id}-face{n}"),
                };
                face["image_uris"]["normal"] = json!(image_url(files_url, &name));
            }
        }
    }
    card
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
        state.received.push(received(request));
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
        // Parentheses group terms; the fake has no `or`, so it drops them.
        for term in query.split_whitespace() {
            let term = term.trim_matches(|c| c == '(' || c == ')');
            if term.is_empty() {
                continue;
            }
            if term.contains(':') {
                warnings.push(format!("Invalid expression “{term}” was ignored."));
            } else {
                terms.push(term.to_lowercase());
            }
        }
        let order = param("order").unwrap_or_else(|| "name".to_owned());
        let dir = param("dir").unwrap_or_else(|| "auto".to_owned());
        let text = |card: &Value, key: &str| card[key].as_str().unwrap_or_default().to_owned();
        let mut matching: Vec<&Value> = state
            .cards
            .iter()
            .filter(|card| {
                let name = text(card, "name").to_lowercase();
                terms.iter().all(|t| name.contains(t))
            })
            .collect();
        let ascending = match (order.as_str(), dir.as_str()) {
            ("name", "auto" | "asc") | ("released", "asc") => true,
            ("name", "desc") | ("released", "auto" | "desc") => false,
            (order, dir) => {
                return error(
                    400,
                    "bad_request",
                    &format!("The fake does not model order `{order}` with dir `{dir}`."),
                );
            }
        };
        let key = |card: &Value| match order.as_str() {
            "released" => (text(card, "released_at"), text(card, "name")),
            _ => (text(card, "name").to_lowercase(), String::new()),
        };
        matching.sort_by_key(|card| key(card));
        if !ascending {
            matching.reverse();
        }
        if matching.is_empty() {
            return error(404, "not_found", NOT_FOUND_DETAILS);
        }
        let start = (page - 1) * state.page_size;
        let data: Vec<Value> = matching
            .iter()
            .skip(start)
            .take(state.page_size)
            .map(|card| with_local_images(card, &state.files_url))
            .collect();
        let has_more = start + data.len() < matching.len();
        let next_page = has_more.then(|| {
            let mut url = reqwest::Url::parse(&state.base_url).unwrap();
            url.set_path("/cards/search");
            url.query_pairs_mut()
                .append_pair("q", &query)
                .append_pair("order", &order)
                .append_pair("dir", &dir)
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

/// Answers the rulings requests of a [`FakeScryfall`].
struct FakeRulings(FakeScryfall);

impl Respond for FakeRulings {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let mut state = self.0.state.lock().unwrap();
        state.rulings_received.push(received(request));
        let id = request
            .url
            .path_segments()
            .and_then(|mut s| s.nth(1))
            .unwrap_or_default();
        if !state.cards.iter().any(|card| card["id"] == id) {
            return error(
                404,
                "not_found",
                "No card found with the given ID or set code and collector number.",
            );
        }
        if state.failing_rulings.iter().any(|failing| failing == id) {
            return error(500, "internal_error", "Something went wrong on our end.");
        }
        let data = state.rulings.get(id).cloned().unwrap_or_default();
        ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "has_more": false,
            "data": data,
        }))
    }
}

/// Answers the image requests of a [`FakeScryfall`].
struct FakeImages(FakeScryfall);

impl Respond for FakeImages {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let mut state = self.0.state.lock().unwrap();
        state.images_received.push(received(request));
        let name = request
            .url
            .path_segments()
            .and_then(|mut s| s.nth(1))
            .and_then(|file| file.strip_suffix(".png"))
            .unwrap_or_default();
        let card_id = name.split("-face").next().unwrap_or_default();
        if !state.cards.iter().any(|card| card["id"] == card_id) {
            // The image servers answer an unknown image with a page that is no Error object.
            return ResponseTemplate::new(404).set_body_string("<Error>NoSuchKey</Error>");
        }
        if let Some(&status) = state.failing_images.get(card_id) {
            return ResponseTemplate::new(status).set_body_string("<Error>Failed</Error>");
        }
        ResponseTemplate::new(200)
            .insert_header("content-type", "image/png")
            .set_body_bytes(std::fs::read(CARD_IMAGE_FIXTURE).unwrap())
    }
}
