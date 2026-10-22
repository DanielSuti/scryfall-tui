//! Everything specific to the Scryfall API: endpoints, headers, rate limits, the JSON wire
//! format and its conversion to domain values.

pub mod client;
pub mod error;
pub mod parse;
pub mod wire;
