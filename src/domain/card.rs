//! Cards and their faces.

use crate::domain::legalities::Legalities;
use crate::domain::type_line::TypeLine;

/// Joins the values of the faces of one card.
pub const FACE_SEPARATOR: &str = " // ";
/// Shown for a face without a mana cost or type line.
pub const MISSING: &str = "—";
/// The width and height in pixels of the image at [`Card::image_url`], so the image's space
/// can be laid out before it arrives.
pub const CARD_IMAGE_PIXELS: (u32, u32) = (488, 680);

/// One face of a card. A single-faced card has one face.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Face {
    /// The face's name.
    pub name: String,
    /// The mana cost, such as `{3}{R}{R}`, or `None` when the face has none.
    pub mana_cost: Option<String>,
    /// The type line, or `None` when the source reports none.
    pub type_line: Option<TypeLine>,
    /// The Oracle text.
    pub oracle_text: Option<String>,
    /// The flavor text.
    pub flavor_text: Option<String>,
    /// Power, a string because it can be `*` or `1+*`.
    pub power: Option<String>,
    /// Toughness, a string for the same reason.
    pub toughness: Option<String>,
    /// Starting loyalty.
    pub loyalty: Option<String>,
    /// Defense of a battle.
    pub defense: Option<String>,
}

impl Face {
    /// Returns the cost for display: the mana cost, or [`MISSING`] when there is none.
    pub fn display_cost(&self) -> &str {
        match self.mana_cost.as_deref() {
            Some(cost) if !cost.is_empty() => cost,
            _ => MISSING,
        }
    }

    /// Returns the card types without supertypes and subtypes, or [`MISSING`].
    pub fn display_types(&self) -> String {
        self.type_line
            .as_ref()
            .map(TypeLine::card_types)
            .filter(|types| !types.is_empty())
            .unwrap_or_else(|| MISSING.to_owned())
    }

    /// Returns `power/toughness`, `Loyalty n` or `Defense n`, or `None` when the face has none.
    pub fn stats(&self) -> Option<String> {
        match (&self.power, &self.toughness, &self.loyalty, &self.defense) {
            (Some(p), Some(t), _, _) => Some(format!("{p}/{t}")),
            (_, _, Some(l), _) => Some(format!("Loyalty {l}")),
            (_, _, _, Some(d)) => Some(format!("Defense {d}")),
            _ => None,
        }
    }
}

/// A card printing with one or more faces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    id: String,
    oracle_id: Option<String>,
    layout: String,
    faces: Vec<Face>,
    legalities: Legalities,
    image_url: Option<String>,
}

impl Card {
    /// Creates a card. `id` identifies the printing, `oracle_id` the card across printings.
    ///
    /// # Panics
    /// Panics if `faces` is empty: every card has at least one face.
    pub fn new(
        id: impl Into<String>,
        oracle_id: Option<String>,
        layout: impl Into<String>,
        faces: Vec<Face>,
        legalities: Legalities,
    ) -> Self {
        assert!(!faces.is_empty(), "a card has at least one face");
        Self {
            id: id.into(),
            oracle_id,
            layout: layout.into(),
            faces,
            legalities,
            image_url: None,
        }
    }

    /// Sets the URL of the front face's image, [`CARD_IMAGE_PIXELS`] in size.
    #[must_use]
    pub fn with_image_url(mut self, url: Option<String>) -> Self {
        self.image_url = url;
        self
    }

    /// Returns the printing id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the id shared by all printings, or `None` when the source reports none.
    pub fn oracle_id(&self) -> Option<&str> {
        self.oracle_id.as_deref()
    }

    /// Returns the layout code, such as `normal` or `transform`.
    pub fn layout(&self) -> &str {
        &self.layout
    }

    /// Returns the faces, front first.
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// Returns the legalities.
    pub fn legalities(&self) -> &Legalities {
        &self.legalities
    }

    /// Returns the URL of the front face's image, or `None` when the source has none.
    pub fn image_url(&self) -> Option<&str> {
        self.image_url.as_deref()
    }

    fn join(&self, value: impl Fn(&Face) -> String) -> String {
        self.faces
            .iter()
            .map(value)
            .collect::<Vec<_>>()
            .join(FACE_SEPARATOR)
    }

    /// Returns every face's name joined with [`FACE_SEPARATOR`].
    pub fn display_name(&self) -> String {
        self.join(|face| face.name.clone())
    }

    /// Returns every face's cost joined with [`FACE_SEPARATOR`], with [`MISSING`] for a face
    /// without a cost.
    pub fn display_cost(&self) -> String {
        self.join(|face| face.display_cost().to_owned())
    }

    /// Returns every face's card types joined with [`FACE_SEPARATOR`].
    pub fn display_types(&self) -> String {
        self.join(Face::display_types)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(name: &str, cost: Option<&str>, type_line: &str) -> Face {
        Face {
            name: name.into(),
            mana_cost: cost.map(Into::into),
            type_line: Some(TypeLine::new(type_line)),
            ..Face::default()
        }
    }

    #[test]
    fn faces_are_joined_with_missing_costs_marked() {
        let delver = Card::new(
            "id",
            None,
            "transform",
            vec![
                face("Delver of Secrets", Some("{U}"), "Creature — Human Wizard"),
                face("Insectile Aberration", Some(""), "Creature — Human Insect"),
            ],
            Legalities::default(),
        );
        assert_eq!(
            delver.display_name(),
            "Delver of Secrets // Insectile Aberration"
        );
        assert_eq!(delver.display_cost(), "{U} // —");
        assert_eq!(delver.display_types(), "Creature // Creature");
    }

    #[test]
    fn single_face_values_are_not_joined() {
        let card = Card::new(
            "id",
            None,
            "normal",
            vec![face(
                "Balefang the Unslayable",
                Some("{3}{R}{R}"),
                "Legendary Creature — Demon Dragon",
            )],
            Legalities::default(),
        );
        assert_eq!(card.display_name(), "Balefang the Unslayable");
        assert_eq!(card.display_types(), "Creature");
        let forest = Card::new(
            "id",
            None,
            "normal",
            vec![face("Forest", None, "Basic Land — Forest")],
            Legalities::default(),
        );
        assert_eq!(forest.display_cost(), "—");
    }

    #[test]
    fn stats_prefer_power_and_toughness() {
        let mut f = face("x", None, "Creature");
        assert_eq!(f.stats(), None);
        f.loyalty = Some("3".into());
        assert_eq!(f.stats().as_deref(), Some("Loyalty 3"));
        f.power = Some("*".into());
        f.toughness = Some("1+*".into());
        assert_eq!(f.stats().as_deref(), Some("*/1+*"));
    }
}
