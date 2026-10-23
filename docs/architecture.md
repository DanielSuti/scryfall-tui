# Architecture

`scry` is one Cargo package with a library (`src/lib.rs`) and a binary (`src/main.rs`). Dependencies between modules point downward only. A module never imports a module above it.

```
tui (event loop, App, layout, views, keymap, widgets, pickers)
  └─ app (command registry, selection, config, history, results cache)
       └─ search (traits CardSearch, CardRulings; ApiSearch now, LocalSearch planned)
            ├─ scryfall (API client, wire types, parse to domain, errors)
            │    └─ net (throttle)
            └─ domain (Card, Face, TypeLine, Legalities, Ruling, SearchResults)
```

## Modules

- **domain** holds service-neutral card data with its behaviour. Cards are identified by the Scryfall printing `id` and grouped by `oracle_id`. `Card` produces the display values joined across faces with ` // `. `TypeLine` separates supertypes, card types and subtypes.
- **scryfall** owns all knowledge of the Scryfall API: base URL, headers, rate-limit intervals, the Error object and the JSON wire types. `parse` turns wire types into domain values. The bulk-data files of a later release use the same Card objects, so the parser serves both.
- **net::Throttle** enforces a minimum interval between request starts per bucket, and pauses every bucket after a rate-limit response. The caller supplies the intervals.
- **search::CardSearch** is the contract every search source implements: a query in, pages of cards out. `ApiSearch` sends the query to Scryfall unchanged. A planned `LocalSearch` will search the offline copy. Batch loading, which follows pages up to the configured cap, is written once against the trait.
- **search::CardRulings** is the contract for rulings: a card's printing id in, its rulings out. `ApiSearch` implements it with the same client as the search, so both share one throttle.
- **app** holds state that is not drawing: the command registry, the selected card, the config, the search history and the session's results cache.
- **tui** draws and handles keys. Rendering is a pure function of the `App` state. Keys, timers and background results all arrive as `AppEvent`s handed to `App::handle_event`, so tests drive the app without a terminal.

## Keys and commands

- A `Command` has an id, a description, the scope it belongs to, an optional hint label for the status line, and a handler.
- The `Keymap` binds key sequences to command ids per mode and scope. The focused panel's scope is searched before the global scope.
- The which-key popup, the status-line hints and the keymaps picker all read from the same registries, so they list everything that can be typed.
- A feature adds its panel state and drawing in `src/tui/views/`, and its commands in a submodule of `src/tui/app/` that registers them together with their keys. No central match over command ids exists.

## Search flow

1. The search bar sends the query. History records it.
2. The query and the session's sort order form the search request. If the results cache holds the request, its results are shown without a request.
3. Otherwise a background task loads up to `search.max_pages` pages through `CardSearch`. Each page arrives as an `AppEvent` and is added to the cached results.
4. When the cursor reaches the last loaded row and more pages exist, the next batch loads.

The layout and keys are described in [ui-layout.md](ui-layout.md).
