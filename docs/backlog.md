# Backlog

Ideas and planned work outside the current release. Nothing here is scheduled until it is moved into a plan.

## Ideas
- Lazygit-style screen modes on `+` and `_`. These keys are reserved.
- Proper double-faced card support for each layout: transform, modal double-faced, adventure, prepared and others, including the back face's image.
- Prices, set, collector number, rarity and artist in the detail view.
- A vim-style `:` command line.
- `?` for backward search, as in vim, tig and yazi.
- An `f` filter of the results list, as in yazi.
- A "what changed since the last update" report.
- Manifest-based incremental updates.
- Offline rulings in the detail.
- `default_cards` printings for collecting and image previews.
- Name autocomplete in the search bar.
- More sort orders and unique options in search.
- Saved queries.
- User-configurable keymap overrides.

## Readability
Ideas taken from tig, yazi and lazygit.
- Show the sort value as a column when sorting by price, release date, rarity or EDHREC rank, as yazi switches its line mode to the sort field.
- Color the card data: mana symbols in their colors, and rarity in set-symbol colors, as tig colors its fields.
- Color the border cyan while `/` searches, as lazygit's `searchingActiveBorderColor`.
- Show the position in the panel border, such as `9 of 67`, or a scrollbar, as lazygit and tig do.
- A loading spinner in the status line while pages load, as in lazygit.
- A `[theme]` table in the config, with light-terminal defaults as lazygit's `lightTheme`, and `NO_COLOR` support.
- Mouse support: click to focus or select, wheel to scroll, as in lazygit and yazi.
- Empty-state hints, such as "Press <Space>ss to search" in empty panels.

## Planned later releases
- Offline data and local search. This includes the `s:`/`r:` decision for one-printing-per-card data.
- Deck management with the backend to be decided.
- The online-default, offline-fallback search setting.

