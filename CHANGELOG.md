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
- Request schedule that spaces request starts per bucket, a request counting in several buckets at once, and pauses every bucket after a rate-limit response.
- Card domain model: faces joined with ` // `, `—` for a face without a cost, card types without supertypes or subtypes, and format legalities. An ignored test checks the supertype list against Scryfall.
- Parser from Scryfall Card objects to domain cards, tested on recorded cards of the normal, transform, modal double-faced, adventure, split and land layouts.
- Scryfall HTTP client sending the `scry/<version>` User-Agent and Accept headers through the request gateway. Error objects keep Scryfall's details, and a 429 pauses all requests. A stateful fake of the search API backs the tests.
- Request gateway: one actor per session that sends every request to Scryfall, searches, rulings and images alike, and the recorder's as well. It sorts each URL into Scryfall's limits (2 per second for search-like endpoints, 10 per minute for the manifest, 10 per second for the whole API, and the file servers on their own), starts each request only once the interval since the last one under the same limits has passed, and keeps one queue per limit so a waiting search doesn't hold back an image. It refuses hosts that aren't Scryfall's, pauses everything after a 429 and twice as long after each further 429 in a row, up to 8 times, and drops requests whose caller stopped waiting before their turn.
- `CardSearch` contract for search sources, and `ApiSearch` sending queries to Scryfall unchanged. A search with no match gives an empty page with Scryfall's message.
- `CardRulings` contract for a card's rulings, which `ApiSearch` fetches from Scryfall's `/cards/<id>/rulings` through the same gateway as the search. The fake answers rulings requests, and a recording of Glorybringer's rulings backs a parse test.
- `CardImages` contract for card images. Cards carry the URL of their front face's `normal` image, and `ApiSearch` fetches it from Scryfall's image servers with an `image/*` Accept header, through the same gateway under the file servers' limit. The fake serves every image from a second local server standing in for the file servers.
- At start-up `scry` asks the terminal for a graphics protocol, as Yazi does: kitty's (kitty, Ghostty), sixel (foot, Windows Terminal) or iTerm2's (iTerm2, WezTerm), and the cell size in pixels, also through tmux. Without one no image is drawn. `detail.images` set to `never` in both views skips the question.
- Config file `~/.config/scry/config.toml` with `search.max_pages`, `history.max_entries`, `detail.formats` and `detail.rulings` and `detail.images` (each with `split` and `full_detail`, `auto` or `never` for that view, and `pause_ms`). Unknown keys and invalid values stop `scry` with an error naming the key.
- Batch loading of search results: up to `search.max_pages` pages at a time, retrying after rate-limit pauses and stopping at the last page or on a failure.
- Session cache of search results keyed by the exact query text. A cached query sends no request, a failed one is loaded again, and a paused one resumes from its next page.
- Search bar: `1` focuses it keeping the query, `<leader>ss` starts a new search, and `<Enter>` runs the query in the background and focuses the results. Running a query again uses the session cache.
- Results view with name, cost and card-type columns cut with `...`, loaded and total counts in the status line, Scryfall's warnings and no-match notice, and the next batch loading when the cursor moves past the last row.
- Card detail in the split and the full detail: every face's name, cost, type line, stats, oracle text and italic flavor text, the legalities of `detail.formats`, and the card's rulings, each dated, or whether they are loading, failed or none exist. Rulings are fetched per `detail.rulings`, which sets the split and the full detail apart, and kept for the session by oracle id; a failed fetch is retried when the card is shown again. `J`/`K` step cards in the full detail, and formats Scryfall does not report are named in the status line.
- One load schedule decides when the detail fetches data beyond the card, following the view's mode and `pause_ms`, so rulings and later detail data share the same rules.
- Card images in the detail, in terminals with a graphics protocol: at the top right of the split, with the text wrapping beside it and then below it and the image scrolling with the text, and on the left of the full detail at the panel's height, as on scryfall.com. Images are fetched per `detail.images`, view by view, the last 50 kept for the session, and scaled to their box off the main thread. The box says when the image is loading or why it failed, and a failed fetch is retried when the card is shown again. Popups hide the image, as Yazi hides its preview. Without room for readable text beside it, the detail has no image.
- Search history model: newest first, re-run queries move to the top, capped at `history.max_entries`, saved atomically as JSON Lines.
- History panel listing past queries with their result counts, cutting long queries with `...`. `<Enter>` runs an entry from the cache and moves it to the top with the cursor, `e` edits it in the search bar and `dd` deletes it. The history is saved in `~/.local/state/scry/history.jsonl`.
- Failed searches keep their loaded cards and show the reason in the status line until the query is run again. Network failures and rate-limit pauses are shown there as well.
- Ignored test running one real search against the Scryfall API.
- Arrow, Home/End and PageUp/PageDown keys move like `j`/`k`, `gg`/`G` and `<C-d>`/`<C-u>`.
- `,` on the results opens a "Sort by" window, and a second key sorts by any of Scryfall's 15 orders, uppercase for reverse. Scryfall sorts all matches, and the order stays for the session.
- Command handlers can capture values and descriptions can be built at run time, so one loop can register a family of related commands.
- Which-key popups can carry a title and appear at once for chosen prefixes, and show a lowercase key and its uppercase twin on one line.
- Tests that need real Scryfall data replay answers recorded once from the API, so no test calls Scryfall. An ignored recorder test refreshes the recordings.
- The `,` sort keys work in normal mode in every panel, not only on the results.
- `search.base` in the config adds Scryfall terms to every search, `game:paper -is:funny` by default. The search bar shows them, and the history keeps the query as typed. The README covers installing and using `scry` in full.
- The focused panel's border and title are green and bold, as in lazygit. All interface styles come from one theme.
- The selected line of the focused panel is bold on a blue background across its width, as in lazygit, instead of the `▌` marker.
- The history shows no selection while it doesn't have focus, and its entries use the full width.
- Backlog of readability ideas taken from tig, yazi and lazygit.
- `/` highlights the matches in the focused panel as you type and moves the cursor to the first one, as yazi does. `<Esc>` in the prompt cancels the search and restores the cursor, and `<Esc>` in normal mode clears the highlight.
- A line matching the `/` search shows its position among the matching lines, such as `[2/9]`, at its right end, as yazi does. The label is a magenta badge so it stands out on the selected line.
- Flash matching on the drawn screen: occurrences of typed text, labelled nearest first, skipping letters that would continue a match.
- `s` starts a flash jump, as in LazyVim: typed characters light up wherever they show on screen, each match gets a label, and typing the label moves the focus and cursor there, across panels. Its colors come from the terminal's palette in the hues LazyVim uses, with dark text on each highlight so it stays readable.
- Every color comes from the terminal's 16-color palette or its default colors, so the interface follows the terminal theme. Flash uses LazyVim's colors for its groups.
- Flash matches, the nearest match and labels show dark text on palette blue, bright yellow and bright red, as LazyVim does, so they stay readable on themes with light palette colors.
