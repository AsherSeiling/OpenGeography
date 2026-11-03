# UI redesign plan

_Written 2026-11-03. Reference: `ANYGIS Mockup.svg` (the user's wireframe). Mock-up: `docs/design/ui-redesign-mockup.html`. **Status: approved by the user 2026-11-03; build started.** Layout editor: option B with add-ons 1, 2, 3, 4, 7, 9 (`docs/LAYOUT_UI_OPTIONS.md`). The mock-ups `docs/design/ui-redesign-mockup.html` and `docs/design/layout-options-mockup.html` are the visual reference, and `applyTheme()` in the first holds the exact derivation of the colours. Where this plan and the mock-up disagree, the mock-up wins for looks and this plan for behaviour._

## 1. What the reference says

It is a wireframe, 824×491, so it fixes layout and character rather than finished styling.

**Character**
- Flat grey panels (`#D9D9D9`) divided by 1 px black rules. No shadows, gradients or inset cards.
- One saturated colour: cyan `#00C0E8`, used as a solid ribbon band.
- One curve in the whole drawing: the active tab's rounded top corners, flowing into the band.
- Inter, regular weight, sentence case.

**Layout (share of the window, used for default sizes)**
| Area | Size | Content |
|---|---|---|
| Top-left cell | left column wide, full ribbon height | "Standard Save, Undo, Redo" (moved to the tab row at the user's request) |
| Tab row | ≈ 3.5 % high | Map, Insert, View, Analysis; search bar far right |
| Band | ≈ 8 % high | Ribbon commands |
| Left column | 15 % wide | Map Layers (top 26 %), Catalog / Project Tree (74 %) |
| Centre | 64 % wide | Map (53 %), bottom panel (47 %) |
| Right column | 21 % wide | Properties (81 %), unlabelled box (19 %) |

**Instructions written on it (taken as requirements)**
1. Catalog / Project Tree: remove the file explorer for files not yet added; layouts live under the project tree; build one public-data tree and keep it here.
2. The bottom panel (attribute table / node editor) opens only when one of them is opened. Otherwise the map fills the centre.

**Conflict with the current app.** There is no Data tab in the reference. Its commands move (§4). The status bar is not in the wireframe; it stays, because the map CRS must always be shown.

## 2. Design tokens

Defined once in `ui/src/theme/tokens.css` as CSS variables on `:root[data-theme]`. `ui/src/theme/fluentTheme.ts` maps them onto Fluent v9 `Theme` objects and dockview's variables, so Fluent controls, dockview and our own CSS read the same values. A vitest fails on a raw hex colour in chrome code; colours that are map or layout *content* (symbol defaults, ramps) are allow-listed.

**Colour.** Default palette below (panel set "Glacier"). Ribbon, highlight, panels and tool colours can all be changed by the user (§2a).
| Token | Light | Dark | Use |
|---|---|---|---|
| `--surface-0` | `#E3E8EC` | `#262C32` | Chrome: pane headers, tab row, status bar |
| `--surface-1` | `#F2F5F7` | `#1C2126` | Pane bodies |
| `--surface-2` | `#FFFFFF` | `#13171B` | Fields, node card bodies, table cells |
| `--canvas` / `--canvas-dot` | `#EAEEF1` / `#C3CCD3` | `#171B1F` / `#343C43` | Node editor ground and dot grid |
| `--rule` | `#25323B` | `#5C6973` | Rules between panes, card and menu outlines |
| `--rule-soft` | `#BCC6CE` | `#3B444C` | Rows, inner dividers |
| `--text` | `#121A20` | `#EDF1F4` | |
| `--text-2` | `#46535D` | `#AEB9C2` | Secondary text (AA on every surface) |
| `--accent` (source value) | `#22CCF5` | `#22CCF5` | User-chosen. Outlines the shape of the open tab and its pane as one, plus menus. No filled band and no outline on individual buttons (user, 2026-11-03) |
| `--band` | `--surface-2` | `--surface-2` | Ribbon band, active tab, menu head |
| `--band-text` | `--text` | `--text` | Text and icons on the band |
| `--sel-bg` | `#6FE0FF` | `#6FE0FF` | Highlight: selected rows, selected card ring. Lighter and brighter than the ribbon, at the user's request |
| `--sel-text` | computed | computed | |
| `--hover` | `rgba(18,26,32,.08)` | `rgba(255,255,255,.08)` | |
| `--link` | accent darkened until AA, `#0562A8` by default | accent lightened until AA | Blue text: selected tab, on or pressed button (with semibold, so it does not rely on colour alone), links. Cyan itself fails AA as text on light panels |
| `--focus` | `--text` | `--link` | Focus ring: 2 px, 1 px offset |
| `--error` | `#B3261E` | `#FF8A80` | |
| `--warn` | `#8A5A00` | `#F2C14E` | |
| `--ok` | `#1F6B3A` | `#7BD88F` | |

"Computed" means the first of `#06222B`, white or black that reaches 4.5:1 on that colour.

**Tool families** (node card header and menu chip, same in both themes, user-changeable, text computed): Input `#22CCF5`, Vector `#9ADBB0`, Raster `#F5CF73`, Hydrology `#A9C4FF`, Tables `#D2C3F7`, Coordinate Systems `#F6B8CF`, Analysis `#F9B48C`, Styling `#DDE0A8`. Output and Save to File: `--rule` with white text.

**Type.** Inter (SIL OFL, `@fontsource-variable/inter`, bundled). UI only; layout and map fonts stay as they are. Numbers use `font-variant-numeric: tabular-nums`. Coordinates use the system mono stack.
| Token | Size / line / weight | Use |
|---|---|---|
| `--type-micro` | 10.5 / 13 / 500 | Group labels, key hints, chips |
| `--type-small` | 11 / 14 / 400 | Descriptions, ribbon labels |
| `--type-body` | 12 / 16 / 400 | Default |
| `--type-strong` | 12 / 16 / 600 | Row names, card titles |
| `--type-tab` | 13 / 18 / 500 | Tabs, pane titles |
| `--type-input` | 14 / 20 / 400 | Menu search |
| `--type-title` | 16 / 22 / 600 | Dialog titles |

**Spacing:** 2, 4, 6, 8, 12, 16, 24 px.

**Shape.** Panes and bands are square. `--r-control: 2px`. The reference's single curve becomes the motif, on anything that is "a tab of something": `--r-tab: 10px 10px 0 0` (ribbon tabs, pane tabs, Shift+A menu) and `--r-card: 8px` (node cards: rounded on all four corners, user 2026-11-03).

**Elevation.** Docked things have none, only rules. Floating things only: `--e-float: 0 8px 24px rgba(0,0,0,.28)` plus a 1 px `--rule` outline.

**Icons.** Fluent Regular (already a dependency, MIT), one colour, 24 px large and 16 px small on the ribbon, 20 px in menus. `toolIcon.tsx` stays the one tool-to-icon map. No new icon set.

**Motion.** 90 ms for menus, 120 ms for panel changes, ease-out, opacity and small translate only. None under `prefers-reduced-motion`.

**Sockets** are shape-coded, so they do not rely on colour: Vector circle, Raster square, Table diamond, Any ring.

## 2a. Custom colours

View › Appearance › **Colours** opens a floating panel, so changes show live on the real screen.
- **Theme:** light or dark.
- **Panels:** three panel sets: Glacier (default), Paper (warm), Wireframe (the reference's greys).
- **Accent colour** and **Highlight colour:** six or seven swatches each, plus any custom colour.
- **Tool colours:** one picker per family.
- **Reset to default.**

Readability is enforced, not advised: text and icons on a chosen colour are computed, and accent text is shifted until it reaches AA, so no choice can make a label unreadable. Checked across all panel sets, both themes and 53 test colours including random ones: lowest ratio 4.5:1.

Colours are saved per user (not in the project), because a project opened by someone else should not change their app. Map and layout content never follows these colours.

## 2b. Multiple maps (user, 2026-11-03)

A project holds any number of named maps. Today the app has one: `order`, `mapCrs`, `basemap` and `worldView` are single values in `store.ts`, read in about 80 places, and `mapController` holds one map.

**What belongs where**
| Belongs to the project (shared) | Belongs to each map |
|---|---|
| Layer data, sources, the layer's one node tree | Name, explicit CRS, basemap, saved view, world view |
| Layouts | Which layers it shows, their order and visibility |

**UI.** Map tabs above the map (dockable, so two can sit side by side), + for a new map. Catalog › Project › Maps lists them. Map Layers shows the active map, and + there adds a project layer. Status bar and Map Layers show the active map's CRS. Insert › New Map, Duplicate Map. A new map takes the last map's CRS and says so. Closing a map never deletes layers. Removing a layer from a map keeps it in the project.

**Layouts.** A map frame names a map (`FrameLayers` gains `{ map: id }`). The existing fixed-list option stays.

**Saved projects.** Old projects open as one map named "Map", with nothing else changed. Project format version goes up.

**Style.** In this version a layer's style is shared by every map. Per-map styles are a later step, because the Symbology node writes to the layer and `LayerInfo` holds the style.

**Engine.** None expected for drawing: display tables are already keyed by layer and CRS. `record_basemap` citations must track every map's basemap. No new Tauri command is expected; if one appears it also goes in the devserver.

## 2c. More open basemaps (user, 2026-11-03)

Each map picks its own, and the credit shows on the map. Terms below come from each project's public pages (2026-11-03); WP 12 re-reads each live policy before enabling it.
| Basemap | Kind | Licence | Key | Notes |
|---|---|---|---|---|
| OpenFreeMap Liberty, Bright, Positron | Vector | Data ODbL, code MIT | None | No stated request limit, no SLA. Credit: "OpenFreeMap © OpenMapTiles, data from OpenStreetMap" |
| VersaTiles Colorful | Vector | Data ODbL, stack permissive | None | Project warns hosted assets may change, so bundle the style |
| OpenTopoMap | Raster | CC BY-SA 3.0 | None | Volunteer server; asks larger users to get in touch. Low-volume, screen only |
| NASA Blue Marble (GIBS) | Raster | NASA: CC0 unless marked otherwise; layer not confirmed | None | Check this layer's marking; acknowledge GIBS |
| Natural Earth | Bundled vector | Public domain | None | Works offline and in every CRS. Grows the installer slightly |

Not offered: EOX Sentinel-2 cloudless (its licence restricts commercial use), and anything needing an account or key.

**Limits.** Web tile basemaps draw in Web Mercator, and vector tiles may not reproject to every map CRS, so a basemap that can't draw in the map's CRS says so instead of guessing. On layouts, keep today's rule: only USGS, plus Natural Earth; the others stay screen-only because of bulk-download terms. Each one records a citation in `sources.rs` and a row in `OPEN_SOURCE.md`.

## 3. Information architecture

| What | Where | Why |
|---|---|---|
| Project name and menu | Top-left cell, as wide as the left column | Reference. |
| Save, Undo, Redo | Tab row, upper right, in line with the tabs and search (user, 2026-11-03; replaces the reference) | Undo acts on whatever is active (layout, else node tree). |
| Commands | Ribbon: Map, Insert, View, Analysis, plus contextual tabs | Reference. |
| Search (commands, tools, layers) | Tab row, right; Alt+Q | Reference. Uses the same matcher as the menu. |
| Layers | Map Layers pane (renamed from Contents), top left, always visible | Reference. Not tabbed with Catalog any more. |
| Project tree, Layouts, public data | Catalog pane, bottom left | Reference. |
| Map | Centre. Fills it unless the bottom panel is open | Reference. |
| Node Editor (renamed from Model), Attribute Table | Bottom panel, tabs, absent until opened | Reference. Closing the last tab removes the panel. |
| Properties | Right | Reference. |
| Sources | Right, under Properties | The unlabelled box in the reference. Default taken when the user approved without changing it. |
| Every geoprocessing tool | Shift+A menu | Brief. |

**Toolbox decision: the Geoprocessing pane is removed**, not demoted. The menu's category rail is the "browse all" view, and a second list would be a second way to learn. What only the pane offered moves to the menu: other layers as inputs (a "Layers" category) and placing onto a wire (§5).

**Catalog, what goes.** The Browse mode (folder browser, folder connections, file inspector). Adding files stays through Add Data and dropping onto the window. `fs_list` and `fs_inspect` stay in the engine, unused by this pane, until a later clean-up. This also removes folder connections and the file inspector, as the reference asks.

**Data tab, where its commands go.**
| Command | New home |
|---|---|
| Add Data | Map › Layer |
| Public data dialogs (TIGER, National Map, 3DEP, ArcGIS, STAC, Overture, OGC, Earthquakes & Soils) | Public Data tree; Map › Layer keeps a "Public Data" button that focuses the tree |
| Add/Calculate Field, Calculate Geometry, Census join, Definition Query, Define Projection, Export, Split | Feature Layer contextual tab (most already there) |
| Map CRS | Map › Coordinate System (already there) |
| Sources | View › Windows |
| Generate Test Data | View › Testing |

**Saved projects.** A `.gproj` stores its dock layout. Old layouts name the removed pane and the old arrangement, so the dock layout gets a version number; an older or unknown one falls back to the new default. Project data is untouched.

## 4. Screen by screen

- **Top-left cell.** Project name with a chevron (New, Open, Save As). In a layout it also holds Back to Map. Its width follows the left column (minimum 190 px), or 190 px when that column is closed.
- **Ribbon.** Tab row 30 px on `--surface-0`, with Save, Undo, Redo and search at the right; active tab is `--band` with `--r-tab`, joined to the band. Band 74 px with `--band-text` icons and labels; rule between groups; `--band-on-*` for checked tools. `ribbonFit.ts` keeps adapting to width, measuring the smaller space. Contextual tabs keep their band name as a small `--link` label, now beside the tab name. Keytips stay.
- **Analysis tab.** Add Tool (large), Run, Node Editor, Attribute Table, and a shelf of favourite tools (the old Tool Gallery, now showing favourites and recents).
- **Map Layers.** Map row with the CRS always visible, then layers; active layer is `--sel-bg`. Drawing order, drag, multi-select and context menus unchanged. Layouts section leaves.
- **Catalog.** One tree: Project (Layouts, Project Files, the existing "This project" list), Public Data (grouped by theme; double-click opens the existing dialog). No new data sources.
- **Map chrome.** Square zoom and scale controls with 1 px rule; notices and measure panels use `--e-float`. Basemap unchanged in both themes.
- **Bottom panel.** Pane tabs with `--r-tab`. Toolbar keeps Run, Add, undo and the tree name. Empty-state hint names Shift+A.
- **Node cards.** Header tinted by family, `--r-card`, 1 px `--rule`, settings inside as now, sockets shape-coded. Layer Input and Output keep "fixed". Wires are `--rule`; selection is a 3 px `--band` ring. All `.mnode` and `.bnode` colours become tokens.
- **Properties, Sources, Items, Attribute Table.** Same content. Headers `--surface-0`, bodies `--surface-1`, fields `--surface-2`, selected rows `--sel-bg`.
- **Layout workspace.** Items, rulers, contextual tabs and page chrome use tokens. The page itself stays white in both themes, because it is the print.
- **Dialogs and error pop-up.** Fluent components through the theme; `--type-title`, square footer rule, `--e-float`. Text unchanged.
- **Status bar.** 24 px, `--surface-0`, top rule. Scale, status, pointer, CRS, project.

## 5. Shift+A menu

**One component**, `ui/src/nodes/ToolMenu.tsx`, replacing `AddMenu.tsx`. Mounted once at app level (inside `dialogs/Dialogs.tsx`) so it works with the Node Editor closed.

**Opens from** Shift+A anywhere (except while typing in a text box, keeping the current rule); right-click on the canvas; the ribbon's Add Tool; the Node Editor toolbar's Add. Position: at the pointer over the canvas or for right-click, otherwise centred on the window. Always kept on screen.

**Layout** (680×452, smaller when the window is): cyan head with the search field; left rail (Start, Layers, then tool groups with counts); list; preview column; footer with key hints. Under 720 px wide the rail becomes a dropdown and the preview a line under the list.

**Each row:** icon, name, one-line description, star. If it matched on a synonym, a chip says what matched ("cut").

**Start view:** Favourites, then Recent (last 8), then everything by group. Stored per user in `localStorage`, ignored if storage is unavailable.

**Search.** Typing filters on every keystroke. Whole-word starts, not substrings ("cut" must not match "execute"). Query and text split on anything that is not a letter or digit, so "Warp (Reproject Raster)" and "Add USNG / MGRS Field" are found by their full names. Rank: name starts with query → a word in the name starts with it → keyword → category → description; favourites break ties. All words must match. Lives in `nodes/toolSearch.ts`, shared with the ribbon search.

**Preview** (highlighted row, hover or keyboard): category path, inputs and output with their kinds, number of settings, and where it will connect ("Connects after Buffer").

**Keyboard.** Focus stays in the field. ↑↓ move, Home/End, PageUp/PageDown, ←→ change category while the field is empty, Enter adds, Esc closes, Mod+D toggles favourite. Combobox/listbox roles with `aria-activedescendant`; result count announced.

**Catalogue.** Reads `tool_specs`. No list of tools in the UI. Plain-language words go in one new field on the spec output, `keywords`, from `crates/engine/src/nodes/keywords.rs`, keyed by tool id. It adds metadata to `SpecOut` rather than editing 78 struct literals. An engine test fails if a tool has no keywords or an id matches no tool. No new command. Starting draft for all 75 tools: `docs/design/tool-keywords.json`.

Note: "Merge Rasters" in the brief is called **Mosaic Rasters** in the engine. "merge" will find Union, Merge and Mosaic Rasters; no tool is renamed.

**Placing a node** (reuses `splice.ts`, `connectProblem`, `addAndMaybeSplice`):
1. Opened with the pointer on a wire: splice into it.
2. A node is selected and its output suits the tool's first input: place to its right and connect; if it fed something, splice in between.
3. The tree is only Layer Input → Output: splice between them.
4. Otherwise place at the pointer, unconnected.

Never a connection that `connectProblem` rejects. The new card is selected and Properties follows it. If the Node Editor is closed, it opens first.

An empty second input (Clip features, Join table…) shows **Choose a layer** on the card. Picking one adds a Layer card and connects it. It builds what dragging a layer in builds today; dragging still works. Tool behaviour is unchanged.

**No layer yet.** No active layer: the menu shows a line "Tools go into a layer's node tree" with the layers to choose from, or an Add Data button if there are none, and adds the tool after the choice.

## 6. Work packages

Each is one Sonnet session in its own worktree, one commit, tests as stated. Each brief says: run `graphify query` before searching code; decide spacing, icons, microcopy and states alone; come to the lead for tool behaviour, feature removal, new dependencies, token changes, or anything outside this plan.

| WP | What | Files (only writer) | Acceptance | After |
|---|---|---|---|---|
| **1 Tokens** | `tokens.css`, `fluentTheme.ts`, Inter, `data-theme` on `:root`; split `app.css` into per-area files with no visual change beyond tokens; hex-colour test | `theme/*`, `main.tsx`, `App.tsx` (theme lines), `package.json`, `OPEN_SOURCE.md`, `THIRD_PARTY_NOTICES.md` | App runs in both themes on tokens; contrast test passes; vitest, tsc, e2e green | – |
| **2 Keywords** | `keywords.rs`, `SpecOut.keywords`, TS type | `crates/engine/src/nodes/keywords.rs`, `nodes/mod.rs`, `lib.rs`, `ipc/api.ts` | Engine test: every tool has keywords, no stray ids; devserver serves them | – |
| **3 Tool menu** | `ToolMenu.tsx`, `toolMenuStore.ts`, `toolSearch.ts`, tests; delete `AddMenu.tsx`; global Shift+A; placement rules | `nodes/ToolMenu*`, `nodes/toolSearch*`, `nodes/categories.ts`, `panes/ModelPane.tsx`, `dialogs/Dialogs.tsx`, `theme/toolmenu.css`, `e2e/shift-a.spec.ts` | §5 all works; new e2e: every tool found by name and by a keyword; "cut" finds Clip; "merge" finds Union, Merge, Mosaic Rasters; keyboard-only add | 1, 2 |
| **4 Project tree** | Catalog tree, Layouts moved from Contents, Public Data tree, Map Layers rename | `panes/CatalogPane.tsx`, `panes/ContentsPane.tsx`, `theme/catalog.css`, `theme/contents.css`, their e2e | Browse gone; layouts open from the tree; all 8 public data dialogs open from it; nothing else lost from Contents | 1 |
| **5 Node cards** | Cards, sockets, wires | `nodes/ModelNodes.tsx`, `nodes/ParamControl.tsx`, `theme/nodes.css` | Family headers, shape sockets, Choose a layer on empty inputs, settings unchanged; AA in both themes | 1 |
| **6 Shell** | Top-left cell, ribbon tabs and commands, search, default layout, conditional bottom panel, remove Geoprocessing pane, layout version | `ribbon/*`, `panes/panes.ts`, `App.tsx`, `actions.ts`, `panes/ToolsPane.tsx` (delete), `theme/ribbon.css`, `theme/dock.css`, `e2e/ribbon-width`, `e2e/map`, `e2e/tool-categories` | Layout matches §1; panel appears and disappears; old projects open with the new layout; ribbon fits at 1024 px | 3, 4 |
| **7 Panes and map chrome** | Properties, Sources, Items, Attribute Table, map overlays, status bar | `panes/PropertiesPane`, `SourcesPane`, `ItemsPane`, `AttributeTable`, `panes/table/*`, `statusbar/*`, map CSS | No raw hex; all states styled; content unchanged | 1 |
| **8 Layout and dialogs** | Layout workspace, dialogs, error pop-up | `layout/*`, `dialogs/*` except `Dialogs.tsx`, `errors/*`, `topo/*`, `raster/*`, `crs/*`, `census/*`, `help/*` | No raw chrome hex; page stays white; export pixel tests unchanged | 3 |
| **8b Colours** | `theme/appearance.ts` (derivation, storage, tested contrast), `theme/AppearancePanel.tsx`, `theme/appearance.css` | those files only; WP6 adds the ribbon button | §2a works live in both themes; vitest proves AA for random colours; survives restart; Reset works | 1 |
| **10 Maps: state** | `maps[]` in the store, migrate the 80 readers, project format and old-project migration | `store/store.ts`, readers, `actions.ts` | Old projects open unchanged; per-map CRS, basemap, layers; tests prove no CRS is guessed | 6 |
| **11 Maps: UI** | Map tabs, several `MapView`s, Catalog › Maps, frame picks a map | `panes/MapView.tsx`, `map/mapController.ts`, `layout/*` frame props | Two maps side by side stay independent; layout frame follows its map | 10 |
| **12 Basemaps** | §2c, one citation each, credit on the map, CRS refusal | `map/*`, `commands.tsx`, `sources.rs`, `layout/basemap.rs`, `OPEN_SOURCE.md` | Each draws, credits and cites; network tests marked ignored like the others | 11 |
| **13 Layout editor** | Option B with add-ons 1, 2, 3, 4, 7, 9 and the menu levels in `docs/LAYOUT_UI_OPTIONS.md` | `layout/*`, `ribbon/*` | Matches the mock-up; every current layout command still reachable; the wrong empty-page hint is gone; pixel tests unchanged | 8, 11 |
| **9 Verify** | Full walk-through, screenshots, docs | `docs/*`, `e2e/redesign.spec.ts`, `PROJECT_STATUS.md` | §7 and the control audit | all |

**Order.** 1 and 2 together. Then 3, 4, 5, 7, 8b together. Then 6 and 8 together. Then 10, 11, 12, 13 in order. Then 9. No two run at once on the same file; WP1's CSS split is what makes this possible. The lead reviews each diff and the running UI before merging.

**Expected test edits.** Tests that name "Model", "Contents", "Geoprocessing", `.addmenu`, `Find Tools` or the Data tab change with the WP that changes the name. A test may change only for a renamed target, never to drop a check.

## 7. Done when

- **Control audit:** an e2e test clicks every ribbon button on every tab, including contextual tabs, every menu item, the top-left cell and each pane's controls, and fails on any that does nothing or throws. It runs against the real app; anything dead is fixed or logged in `BUGS.md`. The mock-up passed this same kind of sweep, but that proves nothing about the app.
- Real-UI run on the devserver: Shift+A, search, add three different tools, connect, run, export a layout. Light, dark, narrow window, keyboard only, Layout workspace.
- `npx vitest run && npx tsc -p tsconfig.json`, `cargo test -p engine` and e2e pass.
- AA contrast, visible focus, labels on icon-only buttons.
- Before and after screenshots in `docs/screenshots/ui-redesign/`.
- `DECISIONS.md`, `TEST_PLAN.md`, `OPEN_SOURCE.md`, `THIRD_PARTY_NOTICES.md`, `PROJECT_STATUS.md` updated; `graphify update .` run.
- Not checked by this plan: the packaged app and Windows/Linux. Inter and the layout are portable, but they are marked verified only once run there.
