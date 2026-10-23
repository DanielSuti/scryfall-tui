# ScryfallTUI
A TUI interface for the Magic The Gathering database https://scryfall.com/

## Usage

```sh
cargo run --release
```

`scry` opens a terminal UI. Press `<Space>ss`, type a [Scryfall query](https://scryfall.com/docs/syntax) and press `<Enter>`. Move with `j`/`k` or the arrow keys, and press `,` on the results to choose how to sort them. Press `<Space>sk` to list every key, and `Q` to quit.

The layout and all keys are described in [docs/ui-layout.md](docs/ui-layout.md).

## Configuration

`scry` reads `~/.config/scry/config.toml` (the XDG config directory) if it exists. Unknown keys are an error.

```toml
[search]
max_pages = 6      # pages of 175 cards loaded per batch

[history]
max_entries = 100

[detail]
formats = ["standard", "pioneer", "modern", "commander", "legacy", "historic", "alchemy", "brawl", "timeless"]
```

Search history is kept in `~/.local/state/scry/history.jsonl`.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

No test calls the live Scryfall API. Tests that need real Scryfall answers replay recordings from `tests/recordings/scryfall/`. To refresh the recordings, for example after Scryfall changes its data, run the one test that calls the API. It sends about 33 requests, one per second:

```sh
cargo test --test scryfall_recorded -- --ignored record_scryfall_answers
```

If Scryfall answers 429, the recorder stops. Wait a few minutes before running it again.
