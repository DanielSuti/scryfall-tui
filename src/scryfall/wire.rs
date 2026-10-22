//! Scryfall's JSON objects, with only the fields `scry` reads. Unknown fields are ignored.
//! See <https://scryfall.com/docs/api/cards> and <https://scryfall.com/docs/api/lists>.

use std::collections::BTreeMap;

use serde::Deserialize;

/// A Card object.
#[derive(Debug, Clone, Deserialize)]
pub struct WireCard {
    /// Printing id.
    pub id: String,
    /// Id shared by all printings. Missing for some layouts, which put it on the faces.
    pub oracle_id: Option<String>,
    /// Layout code.
    pub layout: String,
    /// Name, with ` // ` between faces for multi-faced cards.
    pub name: String,
    /// Mana cost. Missing on cards whose faces carry their own.
    pub mana_cost: Option<String>,
    /// Type line.
    pub type_line: Option<String>,
    /// Oracle text.
    pub oracle_text: Option<String>,
    /// Flavor text.
    pub flavor_text: Option<String>,
    /// Power.
    pub power: Option<String>,
    /// Toughness.
    pub toughness: Option<String>,
    /// Loyalty.
    pub loyalty: Option<String>,
    /// Defense.
    pub defense: Option<String>,
    /// Format name to status: `legal`, `not_legal`, `restricted` or `banned`.
    #[serde(default)]
    pub legalities: BTreeMap<String, String>,
    /// Faces of a multi-faced card.
    pub card_faces: Option<Vec<WireFace>>,
}

/// A Card Face object.
#[derive(Debug, Clone, Deserialize)]
pub struct WireFace {
    /// Name.
    pub name: String,
    /// Mana cost, an empty string for a face without one.
    pub mana_cost: Option<String>,
    /// Type line.
    pub type_line: Option<String>,
    /// Oracle text.
    pub oracle_text: Option<String>,
    /// Flavor text.
    pub flavor_text: Option<String>,
    /// Power.
    pub power: Option<String>,
    /// Toughness.
    pub toughness: Option<String>,
    /// Loyalty.
    pub loyalty: Option<String>,
    /// Defense.
    pub defense: Option<String>,
}

/// A List object holding cards.
#[derive(Debug, Clone, Deserialize)]
pub struct WireCardList {
    /// The cards of this page.
    pub data: Vec<WireCard>,
    /// True when another page follows.
    pub has_more: bool,
    /// The full URI of the next page.
    pub next_page: Option<String>,
    /// Cards across all pages.
    pub total_cards: Option<u32>,
    /// Non-fatal warnings about the request.
    pub warnings: Option<Vec<String>>,
}

/// An Error object. See <https://scryfall.com/docs/api/errors>.
#[derive(Debug, Clone, Deserialize)]
pub struct WireError {
    /// HTTP status.
    pub status: u16,
    /// Machine-readable code, such as `not_found`.
    pub code: String,
    /// Human-readable explanation.
    pub details: String,
    /// Further classification of the error.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Non-fatal warnings about the request.
    pub warnings: Option<Vec<String>>,
}
