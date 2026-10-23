# Project rules for scry

These rules are specific to this repository. They take precedence over the general coding principles in `~/.claude/coding-principles.md` where the two differ.

## Workflow
- One planned step per commit. Each commit leaves `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` passing.
- Every commit adds or edits an entry in `CHANGELOG.md`, including internal changes such as refactors, tests and tooling.

## Changelog format
- Grouped by release, newest first.
- `[Unreleased]` sits on top. At release it is renamed `[vX.Y.Z] - YYYY-MM-DD`, using the ISO 8601 date.
- Tags are semver with a `v` prefix, such as `v0.1.0`.
- Subsections are `Added`, `Changed`, `Fixed` and `Removed`. Before a release has shipped, everything goes under `Added`, and an unreleased entry is edited to describe its final state.

## Keymap
- A key sequence that is a prefix of another in the same mode and scope is rejected at registration. There are no timeouts between keys.
- `+` and `_` are reserved for future lazygit-style screen modes.
- The layout and key table are owned by [docs/ui-layout.md](docs/ui-layout.md).

## Scryfall
- Tests never call the live API. Behaviour tests use the stateful fake in `tests/support/fake_scryfall.rs`. Tests that need real answers use `DummyScryfall` in `tests/support/dummy_scryfall.rs`, which replays `tests/recordings/scryfall/`. Only the ignored recorder test calls the API, and only to refresh the recordings.
- Every request goes through the request gateway (`src/scryfall/gateway.rs`), the only owner of an HTTP client: it sends the `scry/<version>` User-Agent and an Accept header, keeps to Scryfall's rate limits and refuses other hosts. New endpoints get their limit in the gateway's policy, and nothing builds its own HTTP client.
- Search queries are sent to Scryfall unchanged. Scryfall's error details and warnings are shown verbatim.
