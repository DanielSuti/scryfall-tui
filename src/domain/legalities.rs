//! Format legalities.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use thiserror::Error;

/// A card's status in one format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legality {
    /// Allowed.
    Legal,
    /// Not allowed, for example because the card was never in the format's sets.
    NotLegal,
    /// Allowed as a single copy.
    Restricted,
    /// Forbidden by the format's ban list.
    Banned,
}

/// A legality status string that is not one of the known values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown legality status `{0}`")]
pub struct UnknownLegality(pub String);

impl FromStr for Legality {
    type Err = UnknownLegality;

    /// Parses `legal`, `not_legal`, `restricted` or `banned`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "legal" => Ok(Legality::Legal),
            "not_legal" => Ok(Legality::NotLegal),
            "restricted" => Ok(Legality::Restricted),
            "banned" => Ok(Legality::Banned),
            other => Err(UnknownLegality(other.to_owned())),
        }
    }
}

impl fmt::Display for Legality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Legality::Legal => "legal",
            Legality::NotLegal => "not legal",
            Legality::Restricted => "restricted",
            Legality::Banned => "banned",
        })
    }
}

/// A card's legality in every format its source reports.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Legalities(BTreeMap<String, Legality>);

impl Legalities {
    /// Creates legalities from format names and statuses.
    pub fn new(entries: impl IntoIterator<Item = (String, Legality)>) -> Self {
        Self(entries.into_iter().collect())
    }

    /// Returns the status in `format`, or `None` when the format is not reported.
    pub fn get(&self, format: &str) -> Option<Legality> {
        self.0.get(format).copied()
    }

    /// Iterates over the reported format names in alphabetical order.
    pub fn formats(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_statuses_parse_and_unknown_ones_fail() {
        assert_eq!("not_legal".parse(), Ok(Legality::NotLegal));
        assert_eq!("banned".parse(), Ok(Legality::Banned));
        assert_eq!(
            "suspended".parse::<Legality>(),
            Err(UnknownLegality("suspended".into()))
        );
        assert_eq!(Legality::NotLegal.to_string(), "not legal");
    }
}
