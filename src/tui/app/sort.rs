//! Sorting the results with `,` followed by a letter, as in yazi: the letter picks the field,
//! and its uppercase form sorts in reverse. The keys work in normal mode in every panel.

use crate::app::command::{Command, CommandId, Scope};
use crate::search::{SortField, SortOrder};
use crate::tui::app::App;
use crate::tui::app::registrar::{Registrar, RegistrationError};
use crate::tui::keymap::mode::Mode;

/// The prefix of the sort keys.
pub const SORT_PREFIX: &str = ",";
/// The title of the window listing the sort keys.
pub const SORT_TITLE: &str = "Sort by";

/// The letter of each field, and the ids of its default and reversed commands. The uppercase
/// letter runs the reversed one.
const SORT_KEYS: [(SortField, char, &str, &str); 15] = [
    (SortField::Name, 'n', "sort.name", "sort.name.reverse"),
    (SortField::Set, 's', "sort.set", "sort.set.reverse"),
    (
        SortField::Released,
        'd',
        "sort.released",
        "sort.released.reverse",
    ),
    (SortField::Rarity, 'r', "sort.rarity", "sort.rarity.reverse"),
    (SortField::Color, 'c', "sort.color", "sort.color.reverse"),
    (
        SortField::ManaValue,
        'm',
        "sort.mana_value",
        "sort.mana_value.reverse",
    ),
    (SortField::Power, 'p', "sort.power", "sort.power.reverse"),
    (
        SortField::Toughness,
        't',
        "sort.toughness",
        "sort.toughness.reverse",
    ),
    (SortField::Usd, 'u', "sort.usd", "sort.usd.reverse"),
    (SortField::Eur, 'e', "sort.eur", "sort.eur.reverse"),
    (SortField::Tix, 'x', "sort.tix", "sort.tix.reverse"),
    (SortField::Edhrec, 'h', "sort.edhrec", "sort.edhrec.reverse"),
    (SortField::Penny, 'y', "sort.penny", "sort.penny.reverse"),
    (SortField::Artist, 'a', "sort.artist", "sort.artist.reverse"),
    (SortField::Review, 'v', "sort.review", "sort.review.reverse"),
];

pub(super) fn register(reg: &mut Registrar<'_>) -> Result<(), RegistrationError> {
    for (field, letter, id, reverse_id) in SORT_KEYS {
        for (reversed, id, key) in [
            (false, id, letter),
            (true, reverse_id, letter.to_ascii_uppercase()),
        ] {
            let order = SortOrder::new(field, reversed);
            let command = Command::new(
                CommandId(id),
                order.description(),
                Scope::GLOBAL,
                None,
                move |app: &mut App| app.apply_sort(order),
            );
            let notation = format!("{SORT_PREFIX}{key}");
            reg.add(Mode::Normal, &[notation.as_str()], command)?;
        }
    }
    reg.describe_prefix(Mode::Normal, Scope::GLOBAL, SORT_PREFIX, SORT_TITLE, true)
}

impl App {
    /// Returns the order used for searches in this session.
    pub fn sort_order(&self) -> SortOrder {
        self.sort_order
    }

    /// Makes `order` the session's sort order and shows the displayed query in it, from the
    /// cache when it holds those results.
    pub(super) fn apply_sort(&mut self, order: SortOrder) {
        if order == self.sort_order {
            return;
        }
        self.sort_order = order;
        if let Some(query) = self.results.request().map(|r| r.query.clone()) {
            self.run_query(&query);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_field_has_a_distinct_letter() {
        let mut letters: Vec<char> = SORT_KEYS.iter().map(|(_, c, _, _)| *c).collect();
        letters.sort_unstable();
        letters.dedup();
        assert_eq!(letters.len(), SortField::ALL.len());
        let fields: Vec<SortField> = SORT_KEYS.iter().map(|(f, _, _, _)| *f).collect();
        assert_eq!(fields, SortField::ALL);
    }
}
