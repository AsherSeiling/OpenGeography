# Labeling suite plan (user request 2026-10-29)

Builds on L1 (map labels, `ui/src/map/labels.ts`), L2/L3 (layout labels, `crates/engine/src/layout/labels.rs`, `ui/src/layout/labelEdits.ts`). Settings and algorithm ideas follow QGIS's labeling panel (names cited below); everything is reimplemented, no QGIS code is copied (GPL).

## Product decisions (made here, no UI-style change)
- Same style: the **Labels** section in layer Properties grows from one flat form into tabs/sections (Text · Placement · Buffer & Background · Rendering), Office/ArcGIS-Pro look, no new panes.
- One definition: `LabelStyle` (TS `store.ts`, Rust `layout/model.rs`) gets new **optional** fields (`serde(default)`, so old projects load unchanged). Rust and TS must stay in sync; the layout reads the same style as the map.
- Fonts: system fonts are listed by the engine (`list_fonts`, from the `fontdb` that resvg already uses; macOS, Windows, Linux). Layouts/PDF/PNG load **only the requested families** from the system into the render database (the database still never loads all system fonts, so output differs between machines only when a chosen font is missing). A missing font falls back to the bundled Noto Sans and the layout reports "Font X not found; used Noto Sans". Fonts are embedded and subset in PDF by svg2pdf. Some fonts forbid embedding; we do not check licences of user-installed fonts (user's responsibility), stated in docs.

## Settings (QGIS name → ours)
| Group | Setting (QGIS 3.x label) | Ours (`LabelStyle` field) |
|---|---|---|
| Text | Font family, style (bold/italic), Size, Colour | `fontFamily`, `bold`, `italic`, `size`, `color` |
| Text | Letter/word spacing, Line height, Wrap on character, Alignment | `letterSpacing`, `lineHeight`, `wrapAt` (characters), `align` |
| Text | Data defined size / colour / rotation | `sizeField`, `colorField`, `rotationField` |
| Placement point | Around point / Offset from point / Cartographic (quadrant) | `pointMode: "around"\|"offset"\|"quadrant"`, `quadrant`, `distance` |
| Placement line | Parallel, Curved, Horizontal; Above / On / Below line | `lineMode: "curved"\|"parallel"\|"horizontal"`, `linePos: "above"\|"on"\|"below"`, `distance` |
| Placement polygon | Offset from centroid, Around centroid, Horizontal, Free, Outside/Inside | `polyMode: "centroid"\|"surface"\|"inside"\|"outline"` |
| Rendering | Priority (0–10), Allow overlap, Show all labels, Repel/Declutter | `priority`, `allowOverlap` (per label: never hide), `priorityField` |
| Buffer | Draw text buffer (size, colour) | `haloWidth`, `haloColor` (existing) |
| Background | Shape (rectangle/rounded), fill, stroke, size | `background: {fill, stroke, padding, radius}` |
| Shadow | Draw drop shadow (offset, blur, colour) | `shadow: {dx, dy, color, blur}` |
| Contours | (no QGIS equivalent; see below) | `contour: {...}` |

## Stages (each its own branch, merged `--no-ff`; tests: engine, vitest, e2e)
1. **claude/labels-fonts** — `list_fonts` command (Tauri + devserver + `api.ts`), font picker (searchable, shows each name in its own face), bold/italic, the font in map labels, in layout labels and in every layout text element; render database loads requested families; fallback and warning; PDF/PNG tests with a system font.
2. **claude/labels-placement** — new placement and rendering options on the map (OpenLayers) and in layouts (engine `place`): point positions, line above/below/on with curved text, polygon modes, priority and declutter, allow-overlap, background, shadow, wrap, rotation/size/colour from a field.
3. **claude/labels-contours** — contour labels (below).

## Contour labels
Applies to a line layer with a numeric elevation field (named by the user; the Contour node's `ELEV` field is preselected). Settings: `field`, `interval` (label every Nth contour or every X units: "index contours", e.g. every 5th line = every 100 m with 20 m contours), `spacing` (mm along a line between labels, default 60), `inLine` (break the line under the label with a background-coloured gap, or put the label above it), `upright` (text never upside-down: flip when the line runs right-to-left), `decimals`/`unit` suffix formatting, minimum line length for a label. Placement: walk each line by arc length; a label fits where the straight chord equals text width and the line turns less than a limit over that stretch (else try the next position); labels never overlap each other or a placed label of any layer.

## Test plan
`docs/TEST_PLAN.md` section LB (cases added with each stage).

## Not in this plan
Callouts/leader lines, label masks of other layers, label-only layers from expressions, HTML text formatting inside a label. Revisit after use.
