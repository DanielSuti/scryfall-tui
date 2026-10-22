# Changelog

All notable changes to this project are documented in this file, newest release first.

## [Unreleased]

### Added
- Cargo package `scry` with a `--version` flag, and lint settings requiring documented public items.
- Project rules (`CLAUDE.md`), README usage, architecture and UI layout docs, and a backlog of ideas.
- GitHub Actions workflow running rustfmt, clippy and the tests.
- Terminal UI shell: raw-mode terminal setup and restore, an async event loop, and a status line showing the mode. `Q` quits.
- Display-width truncation helper that marks cut text with `...`.
- Command registry: named commands with a description, scope, optional status-line hint and handler.
- Key notation parser for sequences such as `<leader>ss`, `gg` and `<C-d>`, matched against normalised terminal key events.
- Keymap with bindings per mode and scope, rejecting prefix conflicts, and a dispatcher that turns key presses into commands with the focused scope taking precedence.
- Normal and insert modes. Keys are dispatched through the keymap with `<Space>` as leader, insert mode sends unbound keys to the text input, and `<Esc>` returns to normal mode.
- Which-key popup listing the keys that complete a pending sequence, shown after a short delay.
- Status line with sections, messages and key hints for the focused panel, taken from the command registry and cut with `...` when they don't fit.
- Scrollable list widget with `j`/`k`, `gg`/`G` and `<C-d>`/`<C-u>` motions, and a signal when moving past the last row.
- Three-panel layout: search bar, history at a quarter of the width and main panel, with numbered titles and `1`/`2`/`3`/`0` focus jumps. Below 100 columns the history hides and `2` opens it as an overlay.
- Main panel states with `d` for the full detail, `<CR>` for the split and `q` to step back, quitting only from the results. Each state has its own key scope and hints.
- `/`, `n` and `N` search the text of the focused panel with smart case and wrap around, and the vim motions move the focused list.
- Fuzzy picker popup that filters labelled items as you type, moves with `<C-j>`/`<C-k>` and chooses with `<Enter>`.
- Keymaps picker on `<leader>sk` listing every command with its keys. Choosing one runs it.
- Request throttle that spaces request starts per bucket and pauses every bucket after a rate-limit response.
- Card domain model: faces joined with ` // `, `—` for a face without a cost, card types without supertypes or subtypes, and format legalities. An ignored test checks the supertype list against Scryfall.
- Parser from Scryfall Card objects to domain cards, tested on recorded cards of the normal, transform, modal double-faced, adventure, split and land layouts.
- Scryfall HTTP client sending the `scry/<version>` User-Agent and Accept headers through the throttle. Error objects keep Scryfall's details, and a 429 pauses all requests. A stateful fake of the search API backs the tests.
- `CardSearch` contract for search sources, and `ApiSearch` sending queries to Scryfall unchanged. A search with no match gives an empty page with Scryfall's message.
- Config file `~/.config/scry/config.toml` with `search.max_pages`, `history.max_entries` and `detail.formats`. Unknown keys and invalid values stop `scry` with an error naming the key.
- Batch loading of search results: up to `search.max_pages` pages at a time, retrying after rate-limit pauses and stopping at the last page or on a failure.
- Session cache of search results keyed by the exact query text. A cached query sends no request, a failed one is loaded again, and a paused one resumes from its next page.
- Search bar: `1` focuses it keeping the query, `<leader>ss` starts a new search, and `<Enter>` runs the query in the background and focuses the results. Running a query again uses the session cache.
- Results view with name, cost and card-type columns cut with `...`, loaded and total counts in the status line, Scryfall's warnings and no-match notice, and the next batch loading when the cursor moves past the last row.
- Card detail in the split and the full detail: every face's name, cost, type line, stats, oracle and flavor text, and the legalities of `detail.formats`. `J`/`K` step cards in the full detail, and formats Scryfall does not report are named in the status line.
- Search history model: newest first, re-run queries move to the top, capped at `history.max_entries`, saved atomically as JSON Lines.
