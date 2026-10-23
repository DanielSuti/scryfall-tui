//! Conversion from Scryfall's wire objects to domain values.

use thiserror::Error;

use crate::domain::card::{Card, Face};
use crate::domain::legalities::{Legalities, UnknownLegality};
use crate::domain::ruling::Ruling;
use crate::domain::type_line::TypeLine;
use crate::scryfall::wire::{WireCard, WireFace, WireRuling};

/// Why a Scryfall card could not be converted.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The text is not a valid Card object.
    #[error("invalid Scryfall card JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// A legality status is unknown.
    #[error("card {id}: {source}")]
    Legality {
        /// The card's printing id.
        id: String,
        /// The unknown status.
        source: UnknownLegality,
    },
}

/// Parses one Card object from JSON text, such as an API response or a bulk-data line.
///
/// # Errors
/// Returns [`ParseError`] for invalid JSON or an unknown legality status.
pub fn parse_card(json: &str) -> Result<Card, ParseError> {
    card_from_wire(serde_json::from_str(json)?)
}

/// Converts a Card object. A multi-faced card takes every face from `card_faces`; any other
/// card becomes one face from its top-level fields. An empty mana cost becomes `None`. The
/// image is the card's `normal` image, or the first face's on cards with one per face.
///
/// # Errors
/// Returns [`ParseError::Legality`] for an unknown legality status.
pub fn card_from_wire(wire: WireCard) -> Result<Card, ParseError> {
    let legalities = wire
        .legalities
        .iter()
        .map(|(format, status)| {
            status
                .parse()
                .map(|legality| (format.clone(), legality))
                .map_err(|source| ParseError::Legality {
                    id: wire.id.clone(),
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let image_url = wire.image_uris.and_then(|uris| uris.normal).or_else(|| {
        wire.card_faces
            .as_ref()?
            .first()?
            .image_uris
            .as_ref()?
            .normal
            .clone()
    });
    let faces = match wire.card_faces {
        Some(faces) if !faces.is_empty() => faces.into_iter().map(face_from_wire).collect(),
        _ => vec![Face {
            name: wire.name,
            mana_cost: non_empty(wire.mana_cost),
            type_line: wire.type_line.map(TypeLine::new),
            oracle_text: wire.oracle_text,
            flavor_text: wire.flavor_text,
            power: wire.power,
            toughness: wire.toughness,
            loyalty: wire.loyalty,
            defense: wire.defense,
        }],
    };
    Ok(Card::new(
        wire.id,
        wire.oracle_id,
        wire.layout,
        faces,
        Legalities::new(legalities),
    )
    .with_image_url(image_url))
}

fn face_from_wire(face: WireFace) -> Face {
    Face {
        name: face.name,
        mana_cost: non_empty(face.mana_cost),
        type_line: face.type_line.map(TypeLine::new),
        oracle_text: face.oracle_text,
        flavor_text: face.flavor_text,
        power: face.power,
        toughness: face.toughness,
        loyalty: face.loyalty,
        defense: face.defense,
    }
}

/// Converts a Ruling object.
pub fn ruling_from_wire(wire: WireRuling) -> Ruling {
    Ruling {
        published_at: wire.published_at,
        comment: wire.comment,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::legalities::Legality;

    fn fixture(name: &str) -> Card {
        let path = format!(
            "{}/tests/fixtures/scryfall/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        parse_card(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn display(card: &Card) -> (String, String, String) {
        (
            card.display_name(),
            card.display_cost(),
            card.display_types(),
        )
    }

    #[test]
    fn single_faced_cards() {
        let glorybringer = fixture("normal_glorybringer");
        assert_eq!(
            display(&glorybringer),
            ("Glorybringer".into(), "{3}{R}{R}".into(), "Creature".into())
        );
        let face = &glorybringer.faces()[0];
        assert_eq!(face.stats().as_deref(), Some("4/4"));
        assert!(
            face.oracle_text
                .as_deref()
                .unwrap()
                .starts_with("Flying, haste")
        );
        assert!(face.flavor_text.is_some());
        assert_eq!(
            glorybringer.legalities().get("commander"),
            Some(Legality::Legal)
        );
        assert_eq!(
            glorybringer.legalities().get("standard"),
            Some(Legality::NotLegal)
        );
        assert!(glorybringer.oracle_id().is_some());

        let balefang = fixture("legendary_balefang");
        assert_eq!(balefang.display_types(), "Creature");
        let island = fixture("land_snow_covered_island");
        assert_eq!(
            display(&island),
            ("Snow-Covered Island".into(), "—".into(), "Land".into())
        );
        assert_eq!(island.faces()[0].mana_cost, None);
        assert_eq!(island.faces()[0].stats(), None);
    }

    #[test]
    fn multi_faced_cards_take_every_face() {
        let cases = [
            (
                "transform_delver",
                "Delver of Secrets // Insectile Aberration",
                "{U} // —",
                "Creature // Creature",
            ),
            (
                "modal_dfc_shatterskull",
                "Shatterskull Smashing // Shatterskull, the Hammer Pass",
                "{X}{R}{R} // —",
                "Sorcery // Land",
            ),
            (
                "adventure_bonecrusher",
                "Bonecrusher Giant // Stomp",
                "{2}{R} // {1}{R}",
                "Creature // Instant",
            ),
            (
                "split_fire_ice",
                "Fire // Ice",
                "{1}{R} // {1}{U}",
                "Instant // Instant",
            ),
        ];
        for (name, expected_name, cost, types) in cases {
            let card = fixture(name);
            assert_eq!(card.faces().len(), 2, "{name}");
            assert_eq!(
                display(&card),
                (expected_name.into(), cost.into(), types.into())
            );
        }
        let delver = fixture("transform_delver");
        assert_eq!(delver.faces()[1].stats().as_deref(), Some("3/2"));
        assert_eq!(delver.layout(), "transform");
    }

    #[test]
    fn the_image_is_the_cards_or_its_front_faces() {
        let glorybringer = fixture("normal_glorybringer");
        let url = glorybringer.image_url().unwrap();
        assert!(
            url.starts_with("https://cards.scryfall.io/normal/front/"),
            "{url}"
        );
        let delver = fixture("transform_delver");
        let url = delver.image_url().unwrap();
        assert!(
            url.starts_with("https://cards.scryfall.io/normal/front/"),
            "{url}"
        );
        let json = r#"{"id":"x","layout":"token","name":"Spirit"}"#;
        assert_eq!(parse_card(json).unwrap().image_url(), None);
    }

    #[test]
    fn unknown_legality_is_an_error() {
        let json = r#"{"id":"x","layout":"normal","name":"X","legalities":{"modern":"maybe"}}"#;
        let err = parse_card(json).unwrap_err();
        assert!(
            err.to_string().contains("unknown legality status `maybe`"),
            "{err}"
        );
    }

    #[test]
    fn missing_optional_fields_are_unknown() {
        let json = r#"{"id":"x","layout":"token","name":"Spirit"}"#;
        let card = parse_card(json).unwrap();
        assert_eq!(card.oracle_id(), None);
        assert_eq!(card.faces()[0].type_line, None);
        assert_eq!(card.display_types(), "—");
        assert_eq!(card.legalities().formats().count(), 0);
    }
}
