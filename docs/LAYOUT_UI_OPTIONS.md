# Layout editor options

_Written 2026-11-03. **Approved: option B with add-ons 1, 2, 3, 4, 7, 9.** Not built yet. Mock-up: `docs/design/layout-options-mockup.html?opt=A` (B, C, D). Screens: `docs/screenshots/ui-redesign/layout-option-*.png`, current app: `before-layout.png`._

## What is wrong today (seen in the running app)

1. The Model pane stays open under the page and takes about a fifth of the height.
2. Items shares a narrow column with Contents and Catalog, so tab names are cut off ("ontents").
3. Properties and Geoprocessing still show map things.
4. An empty page only says "Insert a Map Frame or Text from the Layout tab", but those commands are on the Elements tab. **This is a bug.**
5. An empty layout gives no starting point, though presets exist for the Topo builder.
6. Inserting an element takes a tab change, and the settings are long forms.

## Menu levels (applies to every option)

Today the tabs overlap. There are two Inserts (the Insert tab and Elements › Insert), Format holds only map-frame and arrange commands, Spread mixes distribute, same size and group, series export sits apart from Export, and Analysis and Feature Layer stay visible in a layout.

**Tab, group, command, then at most one dropdown.** Each tab answers one question:
| Tab | Question | Groups |
|---|---|---|
| Page | What sheet is this? | Sheet, Look, Template, Map Series |
| Insert | What goes on the page? | Maps, Map Furniture, Text and Data, Graphics |
| Arrange | How do things sit together? | Order, Line Up, Group |
| View | How do I see it? | Zoom, Panes, Appearance |
| Export | How does it leave the app? | This Page, Quality, Map Series |
| Map Frame | Only while a frame is selected | Map, Scale and CRS |

**Rules.** Insert and View mean the same in the map and the layout. Map tabs leave while in a layout, and Back to Map stays top left. Commands grey out rather than move; only Map Frame comes and goes. Text formatting lives in Properties and the context bar, not a tab.

**Properties, by what is selected:** what it is first, Position last.
- Page: Sheet, Guides, Info
- Map Frame: Map, Layers, Grid, Style, Position
- Legend: Entries, Text, Style, Position
- Text: Text, Style, Position
- Scale Bar: Scale, Text, Position
- North Arrow: Arrow, Position
- Sources: Text, Position

**Right-click, by what is under the pointer.** Empty page: Paste, Insert, Page Setup. Element: Cut, Copy, Duplicate, Delete, Order, Align, Group, Lock. Map frame: Set from Map View and Which Map, then the same.

**How each option shows it.** A: tabs. B: the rail is Insert, so there is no Insert tab. C: the same five as drop-down menus. Checked against the current app: all 54 current layout commands have a home. "Sources" appears twice on purpose: the element in Insert and the pane in View. Which Map is new, for multiple maps. Rename and move only; no tool behaviour changes. Exact property fields are illustrative until built.

## Choose one arrangement

| Option | Idea | Good for | Cost |
|---|---|---|---|
| **A Focused ribbon** | Same ribbon. In a layout, the left column becomes Items and Maps, the right becomes a tabbed inspector, and the bottom panel closes | Least to learn; closest to the rest of the app | Lowest |
| **B Tool rail (recommended)** | A slim ribbon for page commands; a vertical rail for inserting; a context bar on the selected element | Fastest for repeated work: insert and restyle without changing tabs | Medium |
| **C Studio** | The ribbon goes away; the page fills the window, with floating panels and "Back to map" | Largest page; feels like a separate design tool | Highest; a second shell to maintain |

## Choose any add-ons

| # | Add-on | Works with | Cost |
|---|---|---|---|
| 1 | Start gallery on an empty layout: thematic, land navigation, USGS quadrangle, two maps, inset, blank (screen D) | A, B, C | Low |
| 2 | Shift+A menu for layout elements, same as for tools | A, B, C | Low |
| 3 | Maps panel: drag a map onto the page to make a frame | A, B, C; needs multiple maps | Low |
| 4 | Inspector tabs: Style, Text, Data, Position | A, B, C | Medium |
| 5 | Context bar on the selected element: text style, size, colour, align, lock | B built in; A, C optional | Medium |
| 6 | Smart guides with live distances | A, B, C | Medium |
| 7 | Export panel with a pre-flight check: CRS named, sources listed, scale bar matches, legend matches | A, B, C | Medium |
| 8 | Page strip for map series, shown only when a layout has one | A, B | Low |
| 9 | Named text styles: change "Legend body" once, everywhere | A, B, C | Medium |

Always included, whichever is chosen: fixes for problems 1 to 4 above, new tokens, keyboard access, and the page stays white in dark mode.

## Recommendation

**B with 1, 2, 3, 4, 7 and 9.** Rail and gallery fix what a new user hits first. Pre-flight matches the project's correctness rule. Text styles fix a gap already listed in `LAYOUT_REWORK_PLAN.md`. 6 and 8 can follow.
