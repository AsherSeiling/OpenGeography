# Layout workspace rework — plan for approval (2026-10-29)

**Status (2026-10-29): plan written; the user then said "execute when done", so execution starts now with the recommended answers to §12 adopted (D1 in-app workspace, D2 shapes/polylines only, D3 the listed scales and papers, D4 USGS + Army styles in degrees and mils, D5 MGRS included, D6 everything in the order below, D7 millimetres with points for type). The user can change any of them.**

Why: "If I cannot produce good maps the software is worthless." The layout feature exists (CG1–CG10, C1–C3), but it is a tab inside the GIS window, its settings are scattered, many pieces of text have no size control, and the two things a map reader needs most — a **declination diagram** and a **land-navigation topo preset** — do not exist. This plan rebuilds the layout experience around a dedicated workspace and makes every element and every piece of text customisable.

## 0. What "done" means: three acceptance maps

Every milestone is judged by producing these, end to end, from the packaged app, exported to PDF:

1. **Land-navigation topo quad** (the user's set-map-reading preset): 1:24,000 on Tabloid/Letter, UTM + MGRS grid with labels, contours with index lines and elevations, hillshade, roads/trails/water/names, declination diagram, scale bars (km, mi, yd, m), grid-reference box, legend and marginal information. A person can plot a position, take a bearing and walk it.
2. **Thematic map**: a county choropleth with legend, inset/overview map, scale bar, north arrow, source line, title block, all fonts and sizes chosen by the user.
3. **Trail/route print** (CalTopo style): hillshade + contours + a route and waypoints, lat/lon and UTM ticks, declination, scale and coordinate scales, custom paper.

Pass criteria: every text item's size can be changed in two clicks; nothing overlaps unreadably at the default sizes; the PDF has selectable text and embedded fonts; the declination and grid values match published references (§6, §10).

## 1. What exists today (from the code, 2026-10-29)

Layout elements: map frame (graticule, grid, labels, overview link, series), text, legend, scale bar, north arrow, sources, shape, picture, table, chart. Per-element decor (background, frame, shadow), rotation, lock, visibility, grouping. Page setup (margins, bleed, crop marks), guides, snapping, align/arrange, rulers, templates, themes, map series, export PDF/PNG/SVG with world file. Device fonts. 77+ browser tests and a pixel-level protocol.

Text size controls that exist: text element, legend entries, sources, table (one size for header and cells), chart, per-label overrides in a map frame.
**Text with no size control:** legend title and layer headings, scale-bar numbers and units, graticule and grid labels, north-arrow letters, chart title/axes/legend, table header apart from cells, and everything a new element will add.

## 2. What is wrong with the current UI (to confirm in a click-through before design is final)

1. The layout is **one document tab** among the map, table and model. It borrows the map's Contents pane (map layers, not layout items) and a shared Properties pane. There is no list of what is on the page.
2. No contextual ribbon: layout commands are scattered between the Insert tab, a gallery and dialogs.
3. Element settings are long forms, not organised by purpose (position, appearance, text, data).
4. No named text styles; changing a font means editing every element.
5. No smart guides, distribute-spacing, transform panel, or layer-style search.
6. The user cannot see which map layers a frame draws, or reorder them, without leaving the layout.

## 2b. Top bar (ribbon) revamp and the Shift+A bug — added at the user's request

**Top bar today.** The ribbon body is a fixed 92 px strip with `overflow-x: auto` (`.ribbon-body` in `ui/src/theme/app.css`): when the groups do not fit the window width the strip gets a **horizontal scroll bar** (visible in the user's screenshot of the Layout tab, where the right groups are cut off). The layout tab has so many commands that no window narrower than about 2,000 px fits them.

**Goal.** The top bar never shows a scroll bar and never hides a command. Target look: Office/ArcGIS Pro (kept, per project rules), but adaptive.

| Part | Design |
|---|---|
| Adaptive groups | Every ribbon group declares priorities and **size variants** (large buttons → medium → small icon-only → collapsed into one dropdown button). A `ResizeObserver` picks the largest variant set that fits; groups that still do not fit collapse from the right into a **More ▾** menu. No scroll bar at any width ≥ 900 px |
| Tab row | Compact; tab colours for contextual tabs (Layout, Feature Layer, Raster Layer, Table, Model); command search ("Tell me", Alt+Q) stays on the right; window controls untouched |
| Quick-access bar | Save, Undo, Redo, Open, New layout, customisable; always visible above or beside the tabs |
| Collapse | Double-click a tab or Ctrl+F1 collapses the ribbon to the tab row (more canvas) |
| Keyboard | Alt key tips for every command; Tab/arrow navigation through the ribbon; focus rings |
| Contextual tabs | A mechanism (declared by each view) used by the Layout workspace (§4.1) |
| Status bar | Tidied: coordinates, scale, CRS, progress, notices; nothing clipped at narrow widths |
| Tests | e2e at 1024, 1280, 1440 and 1920 px wide on every tab: ribbon `scrollWidth ≤ clientWidth`, every command reachable (directly or in **More**); a unit test that the group-fitting function never drops a command; screenshots for review |

**Shift+A does not add nodes (user report; "still does not work").** The handler is `ModelPane.tsx` (`onKey`) with the key map in `nodes/keymap.ts`; the Chromium browser tests (Playwright) pass, so the cause is specific to the real app or to a state the tests do not reach. Suspects, most likely first:

1. **A focused text box wins over the pointer.** `typingField()` returns any focused input *outside* the editor (the ribbon's "Scale 1:" box, the Properties fields, a search box) and the handler then ignores the key ("the keys are for that field"). Clicking such a box and then moving the pointer to the Model canvas leaves the box focused, so Shift+A does nothing. The rule should be: **the pointer decides** — over the canvas, a focused field outside the canvas is blurred and the shortcut runs (Blender behaviour).
2. **The editor is judged inactive.** `editorActive()` needs a selected layer and pointer movement seen over the canvas (`sawMove`), or a click inside the model. In the packaged WebView (WKWebView) pointer events or focus may differ, so `overCanvas()` can be false while the user is plainly over it.
3. **WebView key events differ** (key/code values, or the shortcut reaching the window before the pane).

Plan (task **LW-K**): (a) add a **keyboard test** panel (Help ▸ Keyboard test) that prints what the packaged app receives and what the handler decides, so the cause is observed rather than guessed; (b) reproduce in Playwright's **WebKit** engine (closest to WKWebView; needs a one-time `playwright install webkit`, ask before downloading) and in the Chromium suite with a field focused; (c) fix: pointer-decides rule, focus handling, and a fallback that opens the Add menu at the canvas centre when the pointer position is unknown; (d) regression tests for each cause; (e) keep right-click and the Add button working. Done = Shift+A opens the Add menu with the pointer over the Model canvas in the packaged app (checked by the user, and by the diagnostic panel's log).

## 3. Reference analysis (what we take from each)

| Product | What it does well | What we adopt |
|---|---|---|
| **QGIS Print Layout** | Separate layout window; **Items panel** (list of every item with lock/visibility, drag to reorder); per-item properties with sections (Position and Size, Frame, Background, Rendering, Variables); guides; undo history panel; atlas | Items catalog as a first-class pane; sectioned property pages; undo history; variables. Reimplemented, no QGIS code (GPL) |
| **ArcGIS Pro Layouts** | Layout view opens as its own view with **contextual ribbon tabs**; Contents lists layout elements and, under each map frame, its layers; Format pane with Position/Size, Text, Appearance; map series | Contextual Layout ribbon tabs; Contents that shows page → elements → frame layers; Format-pane style tabs |
| **CalTopo** | Print/PDF of a topo with scale, UTM and lat/lon grids, declination, coordinate scale, paper presets; fast, simple | The land-navigation preset's content and defaults; paper/scale presets; coordinate-scale tool |
| **Adobe Illustrator** | **Layers panel**, Properties/Control panel, **Character and Paragraph panels**, named styles, Align/Distribute/Transform panels, smart guides, artboards, appearance stacks | Layer-like Items tree with groups; Character/Paragraph panel; named text styles; Transform panel; smart guides |

Not adopted: free-form vector drawing (pen and node editing) and CMYK separations — out of scope for v1 (decision D2).

## 4. Target user experience

### 4.1 Layout workspace

Opening a layout switches the window to the **Layout workspace** (an in-app workspace, with an option to pop it out into its own OS window, decision D1):

```
+--------------------------------------------------------------------------------+
| Layout | Insert | Format | Arrange | Export | View   (contextual ribbon tabs)   |
+---------------+--------------------------------------------+-------------------+
| ITEMS         |                                            | PROPERTIES        |
| search...     |                  CANVAS                    |  Position  Size   |
| ▾ Page        |        page on grey, rulers, guides,       |  Appearance       |
|  ▾ Map frame  |        smart guides, zoom/pan             |  Text             |
|    ▸ Layers   |                                            |  Data             |
|  ▸ Legend     |                                            |  (per element)    |
|  ▸ Scale bar  |                                            +-------------------+
|  ▸ Declination|                                            | CHARACTER         |
|  ▸ Title      |                                            |  font size style  |
| (lock/eye/    |                                            | PAGE & GRID       |
|  group/rename)|                                            |                   |
+---------------+--------------------------------------------+-------------------+
| status: zoom %, x/y mm, scale 1:24,000, selection                               |
+--------------------------------------------------------------------------------+
```

- **Items catalog (the "different catalog")**: a tree of everything on the layout: page → elements (icon, name, lock, eye, group); under a map frame, the **layers it draws** (visibility, order, per-layout style); under a legend, its entries; under a series, its pages. Drag to reorder (z-order), Shift/Cmd-click multi-select, F2 rename, G group, search, right-click menu, solo/hide others.
- **Contextual ribbon** (ArcGIS-style): *Layout* (page setup, grid, guides, zoom), *Insert* (every element, gallery with previews), *Format* (appearance of the selection), *Arrange* (align, distribute, order, group, transform), *Export*.
- **Properties pane** adapts to the selection: Position and Size, Appearance, Text, Data. Multi-selection edits common fields.
- **Character panel** (Illustrator-style): family, size (pt), weight, italic, colour, alignment, line height, letter spacing, halo; applies to the selected text slot(s).
- Zoom to fit/100 %/selection, hand tool, smart guides, snapping to elements/guides/grid/margins, nudging with arrow keys, undo history.

### 4.2 Every element customisable — the universal model

All elements share:
- **Placement**: x, y, width, height, rotation, lock, visibility, name, group (exists).
- **Appearance**: fill, fill opacity, stroke (colour, width, dash, cap, join), corner radius, margins/padding, shadow (colour, blur, offset), element opacity, optional clip.
- **Text slots**: every element lists its text slots. A slot has the full **text style**: family, size (pt), bold, italic, underline, colour, alignment, line height, letter spacing, halo (colour, width), case. A slot may use a **named text style** (Title, Subtitle, Body, Caption, Map label, Marginal note) so one change updates many elements.

Text slots by element (every one gets a size control):

| Element | Slots |
|---|---|
| Text | text |
| Map frame | map labels (per layer, exists), graticule labels, grid labels, grid zone designation, scale text |
| Legend | title, layer headings, entries, subtitle/notes |
| Scale bar | numbers, units, ratio text, verbal scale |
| North arrow / declination diagram | N letter, angle labels, MN/GN/TN labels, annual-change note |
| Table | header, cells, title |
| Chart | title, axis labels, tick labels, legend, data labels |
| Sources | text |
| Overview map | title, labels |
| Picture | caption |
| Land-nav marginalia | every line of the marginal information block |

**Guard test** (like the CRS tool-matrix guard): the engine enumerates all text slots; the build fails if a slot lacks a UI size control or a test.

Per-element detail (settings the Properties pane exposes, beyond the universal model):

| Element | Extra settings |
|---|---|
| Map frame | scale / extent / centre / rotation, CRS, layers list with order and visibility, background, clip to polygon, graticule and grid (interval, line style, tick vs line, label position/format: DMS, decimal, UTM metres, MGRS; label size; edge ticks), neighbour/overview link, series follow |
| Legend | items list, groups, columns, patch shape/size, spacing, frame/title style, raster colour bars, auto-update |
| Scale bar | style, segments, subdivisions, units (km, mi, m, ft, yd, nmi), height, labels above/below/inline, ratio, second bar |
| North arrow | library of styles, custom SVG, true/grid/magnetic reference, size, colour |
| Declination diagram | §6 |
| Table | fields, sort, row limit, header/zebra/grid lines, column widths, alignment, number formats |
| Chart | type, series, colours, axes, sort, labels |
| Picture | fit, crop, opacity, link vs embed |
| Shape | all of the appearance model, arrowheads, multi-point polygon/polyline |

## 5. Fonts and sizes

Sizes are in points everywhere text appears (positions stay in mm; the page can show inches in the status bar if chosen). A size box offers common sizes and accepts any number from 3 to 200 pt. Fonts come from the device (`fontdb` listing already exists) with the bundled Noto Sans as fallback; the PDF embeds the fonts used. Named text styles are stored in the layout and in templates.

## 6. Declination diagram (topographic north arrow) generator

**What it draws** (the standard USGS/Army map-margin diagram): three lines from a common point — **GN** (grid north), **TN** (true north, with a star) and **MN** (magnetic north, with a half arrow) — with the angles between them marked and labelled: declination (TN–MN), grid convergence (TN–GN), grid-magnetic angle (GN–MN), in **degrees**, **mils** or both, plus the date, the annual change and the caption conventions of the chosen style.

**Inputs**: the map frame it belongs to (centre of the map or a chosen point), the date (default today, editable), the grid CRS (UTM zone of the frame, or any projected CRS), style (USGS topo, US Army, simple), units.

**Computation** (engine, one definition):
- Magnetic declination and its annual change from the **World Magnetic Model** (WMM2025, valid 2025.0–2030.0; the app already ships the coefficients through `geomagnetism`; NOAA/BGS public domain). Outside the validity period the diagram says so and still draws.
- Grid convergence at the point from the projection (existing `map_factors`); undefined for geographic frames, so GN is omitted there.
- Angles and E/W signs are tested against published values: NOAA's WMM calculator at fixed places and dates, and the declination printed on USGS quads (the exact test table is written in the build task, not guessed here).
- Altitude from the DEM when one is loaded, else sea level.
- Drawn as vector (SVG), scalable, with every label's size and font editable.

## 7. Land-navigation (map-reading) topo preset

A one-click template plus a data-preparation flow ("Topo Map Builder", which supersedes C4):

1. **Choose the area**: by map extent, by a centre point + scale, by an MGRS 100 km square/1 km cell, or by a 7.5' quad.
2. **Choose the format**: 1:24,000 (USGS), 1:25,000 (military), 1:50,000, or custom; paper size; portrait/landscape; imperial or metric legend.
3. **Prepare the data**: 3DEP elevation (existing tiled download), contours from the DEM (interval by scale, index lines every fifth, elevation labels), hillshade, water, roads/trails, buildings, names from the public data connectors already built (TNM, Overture, OGC); each cited in Sources.
4. **Build the layout**: map frame with UTM **and** MGRS grid (1 000 m lines, two-digit big numbers, grid zone designation and 100 km square letters in the margin), lat/lon corner ticks, declination diagram, scale bars (km, mi, yd, m) and a **coordinate scale**/grid-reference box ("how to read a grid reference"), title block, legend and marginal information (datum, projection, grid, contour interval, declination note, date, sources, adjoining sheets box).
5. **Check**: a self-check list shows scale, grid interval, declination date and any missing data.

Variants saved as templates: *Land Nav 1:24,000 Tabloid*, *Land Nav 1:25,000 Letter*, *Land Nav 1:50,000*, *Trail map*.

## 8. Build plan — milestones

Each milestone is its own branch, with acceptance criteria, tests, docs and an open-source register update. Effort is in the usual session lengths (rough).

| ID | Milestone | Content | Depends on | Effort |
|---|---|---|---|---|
| LW-T | **Top bar revamp** | §2b: adaptive groups with size variants and More menu (no scroll bars), compact tab row, quick-access bar, collapse, Alt key tips, contextual-tab mechanism, tidy status bar, width tests | none (first) | 3 |
| LW-K | **Shift+A fix** | §2b: keyboard test panel, WebKit reproduction, pointer-decides rule, fallbacks, regression tests | none (parallel) | 1 |
| LW0 | Click-through audit | Run the current layout in the packaged app, record every friction point, lock the design (wireframes in the repo) | approval | 1 |
| LW1 | **Layout workspace shell** | Workspace switch, dock arrangement, status bar, pop-out option, contextual ribbon tabs (empty commands wired to existing actions) | LW0, LW-T | 3 |
| LW2 | **Items catalog** | Tree of page/elements/frame layers/legend entries, lock/eye/rename/group/reorder/search/multi-select, context menu, sync with canvas selection | LW1 | 3 |
| LW3 | **Universal element model** | Appearance + text-slot schema in the engine and TS mirror, migration of old layouts, named text styles, **text-slot guard test** | LW1 | 3 |
| LW4 | **Properties panes per element** | Sectioned panes (Position, Appearance, Text, Data) for all 11 elements, multi-selection editing, Character panel | LW3 | 4 |
| LW5 | **Grid and graticule rework** | Tick/line styles, label formats (DMS, decimal, UTM, MGRS), label sizes and positions, grid zone designation, edge ticks, 1 000 m subgrid | LW3 | 3 |
| LW6 | **Declination diagram element** | Engine computation, SVG drawing, three styles, degrees/mils, tests against references, UI | LW3 | 2 |
| LW7 | **Map-frame layer management in the layout** | Per-frame layer list in the catalog, per-layout style overrides, clip to polygon | LW2 | 2 |
| LW8 | **Arrange and guides** | Smart guides, distribute spacing, Transform panel, snapping to margins/elements, nudge, undo history | LW2 | 2 |
| LW9 | **Land-nav Topo Map Builder** | Area chooser, contours/hillshade/vector prep, marginalia, grid-reference box, coordinate scale, 4 templates, self-check | LW5, LW6, LW4 | 5 |
| LW10 | **Export fidelity** | Embedded fonts, vector everywhere possible, layers in PDF, bleed/crop, world file, large-page rendering and memory limits | LW4 | 2 |
| LW11 | **Templates and themes refresh** | Rebuilt built-in templates, the three acceptance maps as samples | LW9 | 1 |
| LW12 | **Polish** | Keyboard shortcuts, accessibility, colour-blind-safe defaults, help text, responsible-use notes in the layout, packaged-app checklist on all platforms | all | 2 |

Order: **LW-T ∥ LW-K ∥ LW5/LW6 (engine) first**, then LW0 → LW1 → LW2 ∥ LW3 → LW4 → LW5 ∥ LW6 ∥ LW7 ∥ LW8 → LW9 → LW10 → LW11 → LW12. Total about 33 sessions; the first usable improvement (workspace + Items catalog + sizes) lands after LW4.

## 9. What the user gets, milestone by milestone

- After **LW2**: a real layout workspace with a list of everything on the page.
- After **LW4**: change the size, font and look of anything on the page from one panel.
- After **LW6**: a correct declination diagram on any map.
- After **LW9**: the land-nav quad in a few clicks from a map extent.

## 10. Testing

- **Unit/engine**: WMM declination and annual change against NOAA reference values for a table of places and dates; convergence against `cs2cs`/PROJ; MGRS and UTM labels against known squares; text-slot guard; layout serialisation round trips and migration of every older layout in the test fixtures.
- **Pixel/golden**: SVG/PNG hashes on macOS, Linux and Windows for the three acceptance maps (extends the existing protocol).
- **Browser e2e**: build each acceptance map through the UI.
- **Manual checklist on the packaged app** for each milestone (the assistant cannot drive the native app windows in this environment, so these steps stay with the user until desktop control is available).
- **Reference check by a person**: print the land-nav sample and take a bearing and grid reading from it.

## 11. Open-source register additions

World Magnetic Model coefficients (public domain, already listed); fonts from the device (no bundling beyond Noto Sans); Fluent icons (MIT); any new symbol/pattern assets must be CC0/MIT/OFL and listed in `docs/OPEN_SOURCE.md`. QGIS, ArcGIS Pro, CalTopo and Illustrator are references for ideas only: no code, assets or text copied.

## 12. Decisions needed from the user

- **D1.** Workspace: an **in-app workspace** that replaces the window layout while a layout is open (recommended), with a later option to pop it into its own OS window — or build the separate OS window first?
- **D2.** Vector drawing depth: shapes, lines, arrows and multi-point polygons/polylines (recommended) — or also pen/node editing like Illustrator?
- **D3.** Scales and paper for the land-nav presets: 1:24,000, 1:25,000, 1:50,000 on Letter/Tabloid/A3/A4 — others needed?
- **D4.** Declination diagram styles: USGS and US Army, both with degrees and mils (recommended) — any other (NATO/Canadian)?
- **D5.** Is MGRS (military grid) a must-have on the land-nav quad alongside UTM (assumed yes)?
- **D6.** Priority if effort has to be cut: (a) workspace + sizes (LW1–LW4), (b) declination + land-nav (LW5, LW6, LW9), or both in the order above?
- **D7.** Units on the page: millimetres (current) with points for type; add an inches option?
