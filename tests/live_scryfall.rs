//! Tests against the live Scryfall API. They are ignored by default; run them with
//! `cargo test -- --ignored`.

use scry::domain::type_line::SUPERTYPES;

const USER_AGENT: &str = concat!("scry/", env!("CARGO_PKG_VERSION"));

#[tokio::test]
#[ignore = "calls the live Scryfall API"]
async fn supertypes_match_scryfall() {
    #[derive(serde::Deserialize)]
    struct Catalog {
        data: Vec<String>,
    }
    let catalog: Catalog = reqwest::Client::new()
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
    let mut live = catalog.data;
    live.sort();
    let mut ours: Vec<String> = SUPERTYPES.iter().map(|s| (*s).to_owned()).collect();
    ours.sort();
    assert_eq!(ours, live);
}
