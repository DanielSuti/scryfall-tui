# ScryfallTUI
A TUI interface for the Magic The Gathering database https://scryfall.com/

`scry` searches Scryfall from the terminal. It is keyboard-driven and modal, in the style of LazyVim, with a layout borrowed from lazygit and tig. Results, card details and your search history are on one screen, and every key is discoverable from inside the app.

`scry` is unofficial and not affiliated with Scryfall. It needs an internet connection: every search is answered by the [Scryfall API](https://scryfall.com/docs/api).

## Install

You need Rust 1.88 or newer. Install it with [rustup](https://rustup.rs) if `cargo --version` doesn't work.

```sh
git clone https://github.com/DanielSuti/scryfall-tui.git
cd scryfall-tui
cargo install --path .
```

This builds `scry` and puts it in `~/.cargo/bin`. Make sure that directory is on your `PATH`. To update later, run `git pull` and then `cargo install --path . --force`.

To try it without installing, run `cargo run --release` from the repository.

**Terminal.** Use a UTF-8 terminal with a font that has box-drawing characters. Below 100 columns the history panel is hidden; press `2` to open it as an overlay.

## Quick start

1. Run `scry`.
2. Press `<Space>ss`, type a query such as `t:dragon c:r` and press `<Enter>`.
3. Move through the results with `j`/`k` or the arrow keys.
4. Press `<Enter>` to see the card's details next to the results, or `d` to see them full width. `q` goes back.
5. Press `,` to choose how to sort, and then a letter, such as `d` for newest first.
6. Press `Q` to quit.

Press `<Space>sk` at any time to search the list of every key. Press `<Space>` and wait a moment to see the keys that can follow it.

## The screen

```
╭ [1] Search ──────────────────────────────────────────────────────────────╮
│ t:dragon c:r mv=5                                                        │
╰──────────────────────────────────────────────────────────────────────────╯
╭ [2] History ─────╮┏ [3] Results ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
│t:dragon c:r... 67│┃▌Glorybringer                 {3}{R}{R}   Creature   ┃
│ o:draw       3.8k│┃ Stormbreath Dragon           {3}{R}{R}   Creature   ┃
╰──────────────────╯┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
 NORMAL │ [3] Results │ 67 of 67 loaded │ by name, A → Z   d detail · ⏎ split
```

- **[1] Search** holds the query.
- **[2] History** lists past queries with their result counts.
- **[3] Results** lists the matching cards: name, mana cost and card types. It is replaced by the card's details after `d`, or shares the space with them after `<Enter>`. The details show the card's text, with the flavor text in italics, its legality in your formats and its rulings, and in a terminal that can draw images, the card's image.
- **The status line** shows the mode, the focused panel, how many cards are loaded, the sort order, messages, and the most useful keys for the focused panel.

The focused panel has a heavy green border, and its selected line is bold on a blue background (marked `▌` in the drawing above). Without focus, the results mark the selected card with `▌`, and the history shows no selection. The number in each title is the key that focuses it. The full layout, with mockups of every state, is in [docs/ui-layout.md](docs/ui-layout.md).

## Keys

`scry` has two modes. In **normal** mode keys run commands. In **insert** mode you type into the search bar or a prompt, and `<Esc>` returns to normal mode. `<Space>` is the leader key.

**Anywhere, in normal mode**

| Key | Action |
|---|---|
| `<Space>ss` | New search: focus the search bar with an empty query |
| `1` | Focus the search bar and keep the query |
| `2` / `3` / `0` | Focus the history, the results, or the detail of the split |
| `j`/`k`, `↓`/`↑` | Next or previous row |
| `gg`/`G`, `Home`/`End` | First or last row |
| `<C-d>`/`<C-u>`, `PageDown`/`PageUp` | Half a page down or up |
| `/`, then `n`/`N` | Search the focused panel's text, then the next or previous match. Matches light up as you type, and the current line shows its position, such as `[2/9]` |
| `<Esc>` | Clear the search highlight |
| `,` then a letter | Sort the results (see below) |
| `s` | Jump to any text on screen (see below) |
| `<Space>sk` | List every key and command |
| `<Space>c` | Change settings for the session (see [Configuration](#configuration)) |
| `Q` | Quit |

**Search bar, in insert mode:** `<Enter>` runs the search and focuses the results, and `<Esc>` goes back to normal mode.

**Results**

| Key | Action |
|---|---|
| `<Enter>` | Show the card's details next to the results |
| `d` | Show the card's details full width |
| `q` | Close the details, or quit when only the results are shown |
| `L` | With the details shown: load the card's rulings and image now |

**Full detail:** `J`/`K` show the next or previous card, `<Enter>` splits, `L` loads the rulings and image now, and `q` goes back.

**History**

| Key | Action |
|---|---|
| `<Enter>` | Run the search again, instantly when it already ran this session |
| `e` | Copy the query into the search bar to edit it |
| `dd` | Delete the entry |

## Searching

Type any query in [Scryfall's search syntax](https://scryfall.com/docs/syntax), such as `t:dragon c:r mv=5`, `o:"draw a card" f:commander` or `!"Sol Ring"`. The query goes to Scryfall unchanged, so everything the website understands works here. Scryfall's warnings about a query, and its message when nothing matches, appear in the status line and the results panel.

**Base filter.** `scry` adds `search.base` from the config to every search, `game:paper -is:funny` by default. The query you type is wrapped in parentheses first, so `t:goblin or t:elf` is sent as `(t:goblin or t:elf) game:paper -is:funny`. The history and the search bar keep the query as you typed it. Set `base = ""` to search without it.

**Loading.** Results arrive in pages of 175 cards. `scry` loads `search.max_pages` pages at a time, 6 by default, so 1,050 cards. Moving past the last loaded row loads the next batch. The status line shows the count, such as `1,050 of 3,783 loaded`.

**Cache.** Results stay cached for the session. Running a query again, from the search bar or the history, shows it at once without asking Scryfall. Restart `scry` to fetch fresh results and prices.

**Rate limits.** Scryfall allows 2 searches per second. `scry` spaces its requests to stay within that. If Scryfall still answers that the limit is reached, `scry` pauses all requests for the time Scryfall asks and says so in the status line.

## Flash jumps

`s` in normal mode, in any panel, starts a flash jump, as in LazyVim's flash.nvim. All panel text is dimmed and the status line shows `⚡`. Type a few characters: every place on screen showing them lights up, in the search bar, the history, the results and the detail. Each gets a one-letter label on the character after it. Type a label to jump there: the focus moves to that panel and its cursor to that line. A jump into the search bar puts the text cursor on the match in insert mode; a jump into the detail scrolls that line to the top.

- **`<Enter>`** jumps to the match nearest the cursor, drawn in bright yellow.
- **Colors** come from your terminal's palette, so flash follows its theme, in the hues LazyVim uses: grey text, blue matches, a bright yellow nearest match and bright red labels. Matches and labels show dark text on their color, as in LazyVim, so they stay readable on light palette colors.
- **Letters that would continue a match** are never labels, so typing on narrows the matches.
- **Labels** go to the nearest matches first, `asdfghjkl...` and then uppercase. Matches beyond the labels get one once you type more.
- **`<BS>`** deletes a character, and **`<Esc>`** closes flash without moving.
- Matching ignores case unless you type an uppercase letter.

## Sorting

Press `,` in any panel to open the "Sort by" window in the middle of the screen, then press a letter. The uppercase letter sorts the other way round. `<Esc>` closes the window without sorting.

| Key | Order | Lowercase | Uppercase |
|---|---|---|---|
| `n` | Name | A → Z | Z → A |
| `s` | Set | A → Z | Z → A |
| `d` | Release date | newest first | oldest first |
| `r` | Rarity | mythic first | common first |
| `c` | Color | WUBRG first | colorless first |
| `m` | Mana value | lowest first | highest first |
| `p` | Power | lowest first | highest first |
| `t` | Toughness | lowest first | highest first |
| `u` | USD price | highest first | lowest first |
| `e` | EUR price | highest first | lowest first |
| `x` | TIX price | highest first | lowest first |
| `h` | EDHREC rank | most played first | least played first |
| `y` | Penny rank | most played first | least played first |
| `a` | Artist | A → Z | Z → A |
| `v` | Set review | review order | reverse review order |

Scryfall does the sorting, so all matches are in order, not just the loaded ones. The order applies to every search until you change it or restart `scry`. Reversed price orders list cards without a price first.

## History

Every search goes to the top of the history, with its result count. Running a query again moves it back to the top instead of adding a duplicate. The oldest entries drop off beyond `history.max_entries`. The history is saved after every change and survives restarts.

## Configuration

`scry` reads `~/.config/scry/config.toml` if it exists. Every key is optional; these are the defaults, with each comment on its own line so a value can be edited without touching it:

```toml
[search]
# terms added to every search; "" for none
base = "game:paper -is:funny"
# pages of 175 cards loaded per batch, at least 1
max_pages = 4

[history]
# entries kept, at least 1
max_entries = 40

[detail]
# formats whose legality the detail shows, in this order; [] hides the legalities
formats = ["standard", "pioneer", "modern", "commander", "legacy", "historic", "alchemy", "brawl", "timeless"]

[detail.rulings]
# in the split: "auto", "trigger" (on L) or "never"
split = "trigger"
# in the full detail: "auto", "trigger" or "never"
full_detail = "auto"
# "auto": how long the cursor rests on a card first, in milliseconds; 0 for at once
pause_ms = 50

[detail.images]
# in the split: "auto", "trigger" (on L) or "never"
split = "trigger"
# in the full detail: "auto", "trigger" or "never"
full_detail = "auto"
# "auto": how long the cursor rests on a card first, in milliseconds; 0 for at once
pause_ms = 50
```

Rulings take one extra Scryfall request per card, kept for the session. In `auto` mode the cursor must rest on a card for `pause_ms` first, so scrolling through the split or stepping with `J`/`K` doesn't fetch every card on the way. A view set to `trigger` fetches them only when you press `L`, which also skips the pause in `auto`, and shows rulings already fetched without it. A view set to `never` neither fetches them nor shows them, even when the other view already did. The same goes for images, and a split on `trigger` leaves no room for the image until `L`.

Card images take one request to Scryfall's image servers per card, and are shown only in a terminal with a graphics protocol: kitty and Ghostty (kitty's protocol), foot and Windows Terminal (sixel), iTerm2 and WezTerm (iTerm2's protocol). Inside tmux, set `allow-passthrough on`. At start-up `scry` asks the terminal which protocol it supports, which can take up to two seconds in a terminal that doesn't answer; `never` in both views skips that question. Terminals without one, such as Alacritty, show no image.

`<Space>c` opens the config window, listing these settings with their values. `j`/`k` move, `<Tab>` and `<S-Tab>` step through preset values, and `i` or `a` lets you type a value, as in vim, until `<Esc>`. Each change applies at once. `<Enter>` closes the window and saves the changed settings to the config file, creating it from the defaults above if needed and keeping your comments otherwise. `<Esc>` closes it keeping the changes for this session only, and `q` sets back what changed since it opened.

An unknown key or an invalid value stops `scry` at start-up, with an error naming the key and its line. A format name that Scryfall doesn't report is named in the status line.

## Files

| File | Contents |
|---|---|
| `~/.config/scry/config.toml` | Your configuration. `scry` never writes it. |
| `~/.local/state/scry/history.jsonl` | The search history, one query per line. Delete it to clear the history. |

These follow the XDG base directories, so `XDG_CONFIG_HOME` and `XDG_STATE_HOME` move them.

## Troubleshooting

- **The history panel is missing.** The terminal is narrower than 100 columns. Widen it, or press `2` to open the history as an overlay.
- **"Scryfall rate limit reached".** `scry` waits and retries on its own. If it keeps happening, slow down for a minute.
- **"cannot reach Scryfall".** Check the internet connection. Running the query again retries it.
- **`scry` exits with a config error.** Fix the key the message names, or remove it to use the default.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The code is organised as described in [docs/architecture.md](docs/architecture.md), and the project rules are in [CLAUDE.md](CLAUDE.md). Ideas for later releases are in [docs/backlog.md](docs/backlog.md).

No test calls the live Scryfall API. Behaviour tests use a local fake of the search API. Tests that need real Scryfall answers replay recordings from `tests/recordings/scryfall/`. To refresh the recordings, for example after Scryfall changes its data, run the one test that calls the API. It sends about 33 requests, one per second:

```sh
cargo test --test scryfall_recorded -- --ignored record_scryfall_answers
```

If Scryfall answers 429, the recorder stops. Wait a few minutes before running it again.

## License

The code of `scry` is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you say otherwise, any contribution you submit for inclusion in `scry` is dual-licensed in the same way, without any further terms.

This license covers the code only. Card names, text and images, and the card data `scry` shows, belong to their owners and are not covered by it. The same goes for the Scryfall data saved under `tests/fixtures/scryfall/` and `tests/recordings/`. `scry` fetches card data and images from [Scryfall](https://scryfall.com), which provides them under its [terms of use](https://scryfall.com/docs/api), and is not affiliated with or endorsed by Scryfall.

`scry` is unofficial Fan Content permitted under the [Fan Content Policy](https://company.wizards.com/en/legal/fancontentpolicy). Not approved/endorsed by Wizards. Portions of the materials used are property of Wizards of the Coast. ©Wizards of the Coast LLC.
