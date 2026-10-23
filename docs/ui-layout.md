# UI layout

A heavy border marks the focused panel, and titles show the jump number. The mockups are 112 columns wide, so history is 28 columns and the main panel 84.

**V1. Main view**
```
╭─ [1] Search ─────────────────────────────────────────────────────────────────────────────────────────────────╮
│ t:dragon c:r mv=5                                                                                            │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭─ [2] History ────────────╮┏━ [3] Results ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 9 of 67 ━┓
│▌t:dragon c:r mv=5      67│┃ Adult Gold Dragon                   {3}{R}{W}   Creature                         ┃
│ o:scry id:esper t:i... 57│┃ Backdraft Hellkite                  {3}{R}{R}   Creature                         ┃
│ o:draw t:instant      783│┃ Balefang the Unslayable             {3}{R}{R}   Creature                         ┃
│ f:commander id:rg... 2.1k│┃ Boltwing Marauder                   {3}{B}{R}   Creature                         ┃
│ Sol Ring                1│┃ Bonehoard Dracosaur                 {3}{R}{R}   Creature                         ┃
│ t:goblin -t:creature  214│┃ Caldera Pyremaw                     {3}{R}{R}   Creature                         ┃
│                          │┃ Covetous Dragon                     {4}{R}      Creature                         ┃
│                          │┃ Demanding Dragon                    {3}{R}{R}   Creature                         ┃
│                          │┃▌Glorybringer                        {3}{R}{R}   Creature                         ┃
╰──────────────────────────╯┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
 NORMAL │ [3] Results │ 67 of 67 loaded                              d detail · ⏎ split · / find · q quit
```

**V2. After `<CR>`, split side by side. Results keep focus, and the detail follows the cursor.**
```
╭─ [2] History ────────────╮┏━ [3] Results ━━━━━━━━━━━━━ 9/67 ━┓╭─ [0] Detail ─────────────────────────────────╮
│▌t:dragon c:r mv=5      67│┃ Adult Gold Dragon    {3}{R}{W}   ┃│ Glorybringer                       {3}{R}{R} │
│ o:scry id:esper t:i... 57│┃ Backdraft Hellkite   {3}{R}{R}   ┃│ Creature — Dragon                        4/4 │
│ o:draw t:instant      783│┃ Balefang the Unsl... {3}{R}{R}   ┃│                                              │
│ f:commander id:rg... 2.1k│┃ Boltwing Marauder    {3}{B}{R}   ┃│ Flying, haste                                │
│ Sol Ring                1│┃ Bonehoard Dracosaur  {3}{R}{R}   ┃│ You may exert this creature as it attacks.   │
│ t:goblin -t:creature  214│┃ Caldera Pyremaw      {3}{R}{R}   ┃│ When you do, it deals 4 damage to target     │
│                          │┃ Covetous Dragon      {4}{R}      ┃│ non-Dragon creature an opponent controls.    │
│                          │┃ Demanding Dragon     {3}{R}{R}   ┃│ (An exerted creature won't untap during your │
│                          │┃▌Glorybringer         {3}{R}{R}   ┃│ next untap step.)                            │
│                          │┃                                  ┃│                                              │
│                          │┃                                  ┃│ "What the initiates face in the final trial  │
│                          │┃                                  ┃│ is completely at Hazoret's discretion."      │
│                          │┃                                  ┃│                                              │
│                          │┃                                  ┃│ standard   not legal                         │
│                          │┃                                  ┃│ pioneer    legal                             │
│                          │┃                                  ┃│ modern     legal                             │
╰──────────────────────────╯┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛╰──────────────────────────────────────────────╯
 NORMAL │ [3] Results │ 67 of 67 loaded                        j/k card · 0 detail · d full · q close
```

**V3. After `d`, full detail**
```
╭─ [2] History ────────────╮┏━ [3] Detail ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 9 of 67 ━┓
│▌t:dragon c:r mv=5      67│┃ Glorybringer                                                           {3}{R}{R} ┃
│ o:scry id:esper t:i... 57│┃ Creature — Dragon                                                            4/4 ┃
│ o:draw t:instant      783│┃                                                                                  ┃
│ f:commander id:rg... 2.1k│┃ Flying, haste                                                                    ┃
│ Sol Ring                1│┃ You may exert this creature as it attacks. When you do, it deals 4 damage to     ┃
│ t:goblin -t:creature  214│┃ target non-Dragon creature an opponent controls. (An exerted creature won't      ┃
│                          │┃ untap during your next untap step.)                                              ┃
│                          │┃                                                                                  ┃
│                          │┃ "What the initiates face in the final trial is completely at Hazoret's           ┃
│                          │┃ discretion."                                                                     ┃
│                          │┃                                                                                  ┃
│                          │┃ standard   not legal    historic   legal                                         ┃
│                          │┃ pioneer    legal        alchemy    not legal                                     ┃
│                          │┃ modern     legal        brawl      legal                                         ┃
│                          │┃ commander  legal        timeless   legal                                         ┃
│                          │┃ legacy     legal                                                                 ┃
╰──────────────────────────╯┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
 NORMAL │ [3] Detail │ 67 of 67 loaded                         J/K prev/next card · ⏎ split · q back
```

**V4. Below 100 columns, history is hidden.**
```
┏━ [3] Results ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 4 of 67 ━┓
┃ Adult Gold Dragon         {3}{R}{W}   Creature                                                  ┃
┃ Backdraft Hellkite        {3}{R}{R}   Creature                                                  ┃
┃▌Glorybringer              {3}{R}{R}   Creature                                                  ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
 NORMAL │ [3] Results │ 67 of 67    2 history · d detail · ⏎ split · q quit
```

**V5. `2` at 99 columns.** History floats over the main panel at a quarter of the width, until `<Esc>` or a choice.
```
╭─ [3] Results ───────────────────────────────────────────────────────────────────────── 4 of 67 ─╮
│ ┏━ [2] History ━━━━━━━━┓  {3}{R}{W}   Creature                                                  │
│ ┃▌t:dragon c:r mv=5  67┃  {3}{R}{R}   Creature                                                  │
│ ┃ o:scry id:esper... 57┃  {3}{R}{R}   Creature                                                  │
│▌┃ o:draw t:instant  783┃  {3}{R}{R}   Creature                                                  │
│ ┗━━━━━━━━━━━━━━━━━━━━━━┛                                                                        │
╰─────────────────────────────────────────────────────────────────────────────────────────────────╯
 NORMAL │ [2] History │ 4 entries   ⏎ run · e edit · dd delete · <Esc> close
```

**Double-faced rows**
```
 Delver of Secrets // Insectile Aberration   {U} // —             Creature // Creature
 Bonecrusher Giant // Stomp                  {2}{R} // {1}{R}     Creature // Instant
```

**Main-panel transitions.** The card under the cursor is kept in every state.

| State | `d` | `<CR>` | `q` |
|---|---|---|---|
| Results | Full detail | Split | quit |
| Split | Full detail | no-op | Results |
| Full detail | no-op | Split | Results |

`Q` quits from any state.

**Keys**

| Where | Key | Action |
|---|---|---|
| Normal mode, anywhere | `1` | Focus the search bar and keep the query, in insert mode. |
| | `<leader>ss` | Focus the search bar and clear it, in insert mode. |
| | `<leader>sk` | Keymaps picker. |
| | `2` / `3` / `0` | Focus history (an overlay when hidden), the main panel, or the detail (split only). |
| | `/`, `n`, `N` | Search the focused panel's text. |
| | `,` then a letter | Sort the results. See below. |
| | `Q` | Quit. |
| | `+`, `_` | Reserved and unbound. |
| Search bar, insert mode | `<CR>` | Run the search. It enters history, and focus moves to the results. |
| | `<Esc>` | Normal mode. |
| History | `j`/`k` or `↓`/`↑` | Move. |
| | `<CR>` | Run from the cache, or fetch if the query isn't cached. |
| | `e` | Copy the query to the search bar for editing. |
| | `dd` | Delete the entry. |
| Results | `j`/`k`, `gg`/`G`, `<C-d>`/`<C-u>`, or `↓`/`↑`, `Home`/`End`, `PageDown`/`PageUp` | Move. Reaching the last loaded row loads the next batch. |
| | `d`, `<CR>`, `q` | As in the transition table. |
| Full detail | `j`/`k` or `↓`/`↑` | Scroll. |
| | `J`/`K` | Previous or next card. |
| | `<CR>`, `q` | As in the transition table. |

**Status line.** The left side shows the mode, the focused panel and the loaded and total counts. The right side shows hints from the keymap registry for the focused panel.


## Truncation

Text that does not fit its column ends in `...`. Anything after it on the same line, such as a history count, stays visible. One helper does this everywhere text is cut.

## Sorting

`,` in normal mode, in any panel, opens the "Sort by" window at once, as in yazi. A second key picks the order, and its uppercase form sorts in reverse. `<Esc>` closes the window without sorting.

| Key | Order | Default | Uppercase |
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

Choosing an order runs the displayed query again with Scryfall's `order` and `dir` parameters, so all matches are sorted, not only the loaded ones. Reversed price orders list cards without a price first, as Scryfall returns them. The order stays for the rest of the session, shows in the status line as, for example, `by release date, newest first`, and is not stored in the history.

## Panel contents

- **Search bar** shows the query as typed. When `search.base` is set, its terms appear dimmed on the right of the border as `+ game:paper -is:funny`, because every search adds them.

- **Results rows** show name, mana cost and card types. Supertypes and subtypes are left out. Double-faced cards show both faces joined with ` // `, and a face with no mana cost shows `—`.
- **Detail** shows each face's name, cost, full type line, power and toughness, loyalty or defense, oracle text and flavor text, then the legalities for the formats in `detail.formats`.
- **History** shows each query and its result count, newest first.
