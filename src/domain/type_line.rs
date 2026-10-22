//! Type lines: supertypes, card types and subtypes.

/// The supertypes of Magic, as listed by Scryfall's `/catalog/supertypes` on 2026-10-22.
/// The list belongs to the game rules; the ignored network test `supertypes_match_scryfall`
/// compares it with the live catalog.
pub const SUPERTYPES: &[&str] = &[
    "Basic",
    "Elite",
    "Legendary",
    "Ongoing",
    "Snow",
    "Token",
    "World",
];

/// The character that separates types from subtypes on a type line.
pub const SUBTYPE_SEPARATOR: char = '—';

/// One face's type line, such as `Legendary Creature — Demon Dragon`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLine {
    text: String,
}

impl TypeLine {
    /// Wraps the type line text as printed.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// Returns the type line as printed.
    pub fn full(&self) -> &str {
        &self.text
    }

    fn types_part(&self) -> &str {
        self.text
            .split(SUBTYPE_SEPARATOR)
            .next()
            .unwrap_or_default()
            .trim()
    }

    /// Returns the words before the em dash that are not supertypes, joined by spaces, such as
    /// `Creature` or `Artifact Land`. A word not in [`SUPERTYPES`] is kept, so a new supertype
    /// stays visible.
    pub fn card_types(&self) -> String {
        self.types_part()
            .split_whitespace()
            .filter(|word| !SUPERTYPES.contains(word))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Returns the words after the em dash, such as `["Demon", "Dragon"]`.
    pub fn subtypes(&self) -> Vec<&str> {
        self.text
            .split_once(SUBTYPE_SEPARATOR)
            .map(|(_, sub)| sub.split_whitespace().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_types_drop_supertypes_and_subtypes() {
        let line = TypeLine::new("Legendary Creature — Demon Dragon");
        assert_eq!(line.card_types(), "Creature");
        assert_eq!(line.subtypes(), ["Demon", "Dragon"]);
        assert_eq!(
            TypeLine::new("Basic Snow Land — Island").card_types(),
            "Land"
        );
        assert_eq!(TypeLine::new("Artifact Land").card_types(), "Artifact Land");
        assert_eq!(TypeLine::new("Instant").subtypes(), Vec::<&str>::new());
    }

    #[test]
    fn unknown_words_before_the_dash_stay_visible() {
        assert_eq!(
            TypeLine::new("Mythical Creature — Dragon").card_types(),
            "Mythical Creature"
        );
    }
}
