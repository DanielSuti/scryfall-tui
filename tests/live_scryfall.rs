//! Tests against the live Scryfall API. They are ignored by default; run them with
//! `cargo test -- --ignored`.

use scry::domain::type_line::SUPERTYPES;
use scry::scryfall::client::USER_AGENT;

#[tokio::test]
#[ignore = "calls the live Scryfall API"]
async fn supertypes_match_scryfall() {
    let catalog: serde_json::Value = reqwest::Client::new()
        .get("https://api.scryfall.com/catalog/supertypes")
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut live: Vec<String> = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    live.sort();
    let mut ours: Vec<String> = SUPERTYPES.iter().map(|s| (*s).to_owned()).collect();
    ours.sort();
    assert_eq!(ours, live);
}
