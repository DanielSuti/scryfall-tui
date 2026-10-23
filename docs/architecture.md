# Architecture

`scry` is one Cargo package with a library (`src/lib.rs`) and a binary (`src/main.rs`). Dependencies between modules point downward only. A module never imports a module above it.

```
tui (event loop, App, layout, views, keymap, widgets, pickers)
  └─ app (command registry, selection, config, history, results cache)
       └─ search (traits CardSearch, CardRulings, CardImages; ApiSearch now, LocalSearch planned)
            ├─ scryfall (API client, wire types, parse to domain, errors)
            │    └─ net (schedule)
            └─ domain (Card, Face, TypeLine, Legalities, Ruling, SearchResults)
```

## Modules

- **domain** holds service-neutral card data with its behaviour. Cards are identified by the Scryfall printing `id` and grouped by `oracle_id`. `Card` produces the display values joined across faces with ` // `. `TypeLine` separates supertypes, card types and subtypes.
- **scryfall** owns all knowledge of the Scryfall API: base URL, headers, rate-limit intervals, the Error object and the JSON wire types. `parse` turns wire types into domain values. The bulk-data files of a later release use the same Card objects, so the parser serves both.
- **scryfall::gateway** is the one way out to the network. `Gateway::live` starts one actor per process, which owns the only HTTP client, sets the User-Agent and Accept headers, and refuses hosts other than `api.scryfall.com` and `*.scryfall.io`. It sorts each URL into Scryfall's limits: search-like endpoints, the manifest, the whole API, and the file servers. Each limit has its own queue, so a waiting search doesn't hold back an image, and a request starts only once the interval since the last request under each of its limits has passed. A 429 pauses every queue, longer after each further 429 in a row. A request whose caller stopped waiting is dropped before it is sent. Anything new that talks to Scryfall, such as prices, goes through a `ScryfallClient`, and so through the gateway, and gets these limits without further work.
- **net::Schedule** keeps the time for the gateway: the next start per bucket, where a request can count in several buckets, and pauses. It sends nothing itself.
- **search::CardSearch** is the contract every search source implements: a query in, pages of cards out. `ApiSearch` sends the query to Scryfall unchanged. A planned `LocalSearch` will search the offline copy. Batch loading, which follows pages up to the configured cap, is written once against the trait.
- **search::CardRulings** is the contract for rulings: a card's printing id in, its rulings out. `ApiSearch` implements it with the same client as the search, through the same gateway.
- **search::CardImages** is the contract for card images: a card's image URL in, the encoded image bytes out. `ApiSearch` fetches them from Scryfall's image servers through the same client, under the file servers' limit, so an image never waits for a search or a ruling, but a 429 pauses them all.
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

## Rulings flow

1. Whenever the detail shows a card, the app looks up its rulings by oracle id in the session's rulings.
2. If they are missing, the mode `detail.rulings` sets for the view decides when to fetch: after `pause_ms` through a deadline returned by `App::next_deadline`, which the event loop turns into a tick, only on `L` in `trigger` mode, or not at all. `L` fetches at once in any mode but `never`.
3. A background task fetches them through `CardRulings`, and they arrive as an `AppEvent`.

## Images flow

1. At start-up, unless `detail.images` is `never` in both views, `tui::graphics::detect` asks the terminal for a graphics protocol and its cell size through `ratatui-image`, as Yazi picks its image adapter. Without kitty, sixel or iTerm2 support the app gets no picker and shows no images.
2. Whenever the detail shows a card, the app computes the image box from the panel and the cell size, and the same `LoadSchedule` as the rulings decides when to fetch, keyed by the printing id; `L` fetches at once, as for rulings.
3. A background task fetches the bytes through `CardImages`, and they arrive as an `AppEvent`. The session keeps the last 50.
4. A blocking task decodes them and encodes them for the protocol at the box's size. Only the shown card's encoding is kept; a result for another card or size is dropped.
5. Drawing puts the encoded image into the box as a sliced image, so the split can scroll it partly out of view. Popups hide it.

## Settings flow

1. `app::settings::Setting` lists the settings the config window edits: each one's label, table and key, its value as text, its presets, and a validating setter.
2. The config window, `tui::views::config_window` and `tui::app::config_window`, changes `App::config` through `App::apply_setting`. That setter also updates what depends on the setting, such as the history's length.
3. `App` keeps the file's values as `saved`, which mark the changed settings. `app::config_file::save` writes chosen settings into the file with `toml_edit`, keeping the user's comments and order, starting a missing file from `DEFAULT_CONFIG`.

The layout and keys are described in [ui-layout.md](ui-layout.md).
