# Layouts (print maps) — plan

_Opus design, 2026-10-23. Phase C, task C1. Build sessions: C2.1–C2.4 and C3.1–C3.2 below. C4 (Topo Map Builder) and F5 (planetary cartography) build on this._

**Goal:** a user makes a print map on a page in a **true projected CRS**, at a stated scale. The page has a legend, scale bar, north arrow, title, text, sources and a graticule/grid. It exports to **vector PDF**, PNG and SVG. Output is identical on macOS, Windows and Linux.

---

## 1. Constraints that decide the design

- **MapLibre draws only Web Mercator (or a globe).** It can't draw a frame in UTM, State Plane or Albers, so it can't produce a true-scale print map. Capturing its canvas gives a Mercator raster, not vectors.
- **The data lives in the engine.** DuckDB holds full-resolution geometry in each layer's own CRS, and GDAL/PROJ are already linked. Sending 100k+ features to JavaScript for printing would duplicate the tile pipeline and be slow.
- **Licence:** MIT OR Apache-2.0 project, no GPL. Cairo (LGPL) and Skia (huge to build) are out.
- **Determinism across platforms:** no system fonts. Use bundled fonts only, so the same layout gives the same bytes everywhere.
- **Planetary (F5):** scale bars, north arrows and graticules must not hard-code WGS 84.
- **Styles live in the UI today** (`LayerStyle` in `ui/src/store/store.ts`, saved in `project.json`).

---

## 2. Decisions

### L1. Rendering path: the engine renders (option B), through SVG
**Chosen:** the engine builds **one SVG document** per page, then converts it:
- **PDF** (vector, fonts embedded and subset): `svg2pdf` (on `krilla`). Expected licence MIT OR Apache-2.0.
- **PNG** (preview and raster export): `resvg` + `tiny-skia`. Expected licences Apache-2.0 OR MIT, and BSD-3-Clause.
- **SVG** export: the document itself. It's useful for Inkscape/Illustrator finishing work.
- Text: `usvg` + `fontdb`, loaded **only** with the bundled fonts.

**Licence gate (first thing in C2.1):** confirm each crate's licence and its transitive dependencies with `python3 scripts/third_party_notices.py` and the existing CI licence gate before writing code on top of them. If any is not permissive, fall back to `krilla` (PDF) + `tiny-skia` (PNG) directly from an internal display list, and stop for an Opus review.

**Why not A (MapLibre canvas capture + JS PDF):**
- it can't do a true CRS;
- the map data comes out as a raster;
- it needs WebGL at 300 dpi sizes, which hits GPU and canvas size limits;
- the headless CLI (phase G) couldn't render.

B costs more engine code, but one renderer serves preview, PDF, PNG, SVG, map series, the Topo Map Builder and the CLI.

**Preview = the same renderer.** The Layout view shows an engine-rendered PNG of the page at screen resolution (about 96–150 dpi, debounced 150 ms). An HTML overlay on top holds the selection handles. WYSIWYG is exact because the preview *is* the export path.

### L2. Document model (serde in `crates/engine/src/layout/model.rs`, mirrored as a TS type in `ui/src/layout/types.ts`)
```jsonc
{
  "id": "L1", "name": "Layout 1", "version": 1,
  "body": "earth",                       // L4 — from day one
  "page": { "width": 279.4, "height": 215.9, "units": "mm", "margin": 12.7, "background": "#ffffff" },
  "elements": [
    { "id": "f1", "kind": "map_frame", "x": 12.7, "y": 30, "w": 200, "h": 160, "rotation": 0, "locked": false, "visible": true,
      "crs": "EPSG:26915",              // explicit; never guessed (L3)
      "center": [500000, 4649776],       // in frame CRS units
      "scale": 24000,                    // nominal scale denominator
      "layers": "map",                   // "map" = follow map order/visibility, or [{ "id": "3", "visible": true }]
      "basemap": "none",                 // C2.4: "none" | "usgsTopo" | "usgsImagery" | "usgsRelief"
      "graticule": null,                 // C2.4: { "interval": "auto" | degrees, "labels": true }
      "grid": null,                      // C2.4: { "interval": "auto" | CRS units, "labels": true }
      "border": { "width": 0.35, "color": "#000000" } },
    { "id": "t1", "kind": "text", "x": 12.7, "y": 10, "w": 254, "h": 14, "text": "{layout_name}", "font": "sans-bold", "size": 18, "align": "center", "color": "#000" },
    { "id": "lg", "kind": "legend", "frame": "f1", "x": 220, "y": 30, "w": 46, "h": 80, "title": "Legend", "columns": 1 },
    { "id": "sb", "kind": "scale_bar", "frame": "f1", "x": 220, "y": 120, "w": 46, "h": 10, "units": "km", "segments": 4 },
    { "id": "na", "kind": "north_arrow", "frame": "f1", "x": 250, "y": 140, "w": 12, "h": 18, "style": "simple" },
    { "id": "src", "kind": "sources", "frame": "f1", "x": 12.7, "y": 195, "w": 200, "h": 10, "size": 6 }
  ],
  "series": null                        // C3.2
}
```
- **Units:** page in millimetres, origin top-left, y down (same as SVG). Font size in points.
- **Text tokens:** `{layout_name}`, `{project}`, `{date}`, `{scale}` (of the first frame), `{crs}`, `{page_name}`/`{page_number}`/`{page_count}` (series).
- **Storage:** a `__layouts (id VARCHAR PRIMARY KEY, json JSON, updated TIMESTAMP)` table in `project.duckdb`. It travels with the project and is readable by the future CLI. The engine validates on save (unknown `kind` → error; `version` field for later migrations).
- **Editing state** lives in a new zustand store `ui/src/store/layout.ts`, with zundo undo like `model.ts`. It saves to the engine debounced (500 ms) and on project save. It marks the project dirty.
- **Styles for rendering:** each render request sends `{ layout, styles, order, visible, mapCrs }` from the UI store. The Rust side has a `LayerStyle` serde struct mirroring the TS one. A drift test deserializes a JSON fixture written by a vitest test from `defaultStyle()` output, so both sides fail if the shapes diverge. (Moving styles into the engine is a phase G question; don't do it now.)

### L3. Scale, CRS and orientation correctness
All geodesic maths uses `geographiclib-rs` (MIT) `Geodesic::new(a, f)` from the layout's **body** (L4). The existing DuckDB `ST_*_Spheroid` functions are WGS 84 only, so they're not used for layouts.

- **Frame CRS** must be explicit. A new frame takes the map's `mapCrs`. If that is missing → refuse with "Set a coordinate system for the map frame". If it's geographic (degrees) → allowed, with a persistent warning chip: "Geographic coordinates: scale varies across the frame; choose a projected CRS for a true-scale map" and a button using `suggest_crs`. Never guess silently.
- **Nominal scale:** frame width in ground (CRS) units = `w_mm / 1000 × scale` (converted by the CRS's linear unit, e.g. US survey feet). For a geographic frame, scale is defined at the frame centre along the parallel, and labelled "≈ 1:N at centre".
- **Point scale factor k at the frame centre:** compute numerically. Project the centre ±1 m east and north (via inverse/forward PROJ), take the geodesic ground distance with geographiclib, and divide grid distance by ground distance (k = mean of east and north for conformal; for non-conformal CRSs, use the bar's direction, east).
- **Scale bar = ground truth at the frame centre:** a bar labelled D km is drawn `D × 1000 × k / scale × 1000` mm long. Show a warning chip in the frame's properties if k varies by more than 1% between the frame's corners and centre ("Scale varies 2.3% across this frame").
- **North arrow = true north at the frame centre:** the angle of the projected vector from the centre to a point 10 m geodesically north (meridian convergence), plus the frame rotation. A grid-north/magnetic diagram is C4 (WMM is already in `nav/geodesy.ts`).
- **Data selection:** for each layer, transform the frame polygon (densified, 32 points per side) to the layer CRS and to EPSG:3857, and prefilter on the existing `__g` R-tree. Then `ST_Transform(geom, layer_crs, frame_crs, always_xy := true)`, then clip to the frame (a margin of 2 mm of ground for strokes, plus an SVG `clipPath`). Features with no CRS → skipped and **reported** in the render result ("Layer X skipped: no coordinate system").
- **Generalization for print:** `ST_SimplifyPreserveTopology` with tolerance = 0.1 mm on the page in ground units (`0.0001 × scale` m). Points are never dropped. Report dropped (empty-after-clip) counts per layer only in debug output; they're outside the frame, not data loss.

### L4. `body` from day one
- `layout.body: String`, default `"earth"`. Engine `layout::geo::Body { name, a, f, lon_convention }`, with `Body::get("earth")` = WGS 84. Any other name → error "Planetary bodies arrive in phase F". F1 adds the Moon and Mars to the same table.
- Every scale bar, north arrow, k and graticule calculation takes `&Body`. No other code path in `layout/` may assume WGS 84 (grep test: no `6378137` literal outside `geo.rs`).
- Graticule labels go through `Body::format_lon/lat`. For Earth that's °E/°W, °N/°S. Mars and Moon can use 0–360° east later.

### L5. UI structure (Office/ArcGIS Pro style, unchanged look)
- **Insert → Layouts → New Layout** (the `newLayout` command already exists in `Ribbon.tsx`). It opens a gallery: Letter, Legal, Tabloid, A4, A3, each portrait/landscape (templates arrive in C3.1; in C2, page sizes only).
- **Layout view** = a dockview **document tab** beside the Map tab, one per layout. The page sits on a grey canvas, with zoom (fit page / 100% / wheel) and rulers in mm. Clicking selects, dragging moves, handles resize, Delete removes, arrow keys nudge 1 mm (Shift: 0.1 mm).
- **Contextual ribbon tab "Layout"** (shown only while a layout tab is active, like ArcGIS Pro's contextual tabs):
  - *Page*: size, orientation, margins;
  - *Insert*: Map Frame, Text, Legend, Scale Bar, North Arrow, Sources (Graticule/Grid in C2.4);
  - *Map Frame*: "Set from Map View" (current MapLibre centre and zoom → centre in frame CRS + a scale rounded to a standard series value), a scale box `1:[24,000]`, CRS picker;
  - *Arrange*: bring forward/back, align, lock;
  - *Export*: PDF, PNG, SVG.
- **Contents pane:** a "Layouts" section listing the project's layouts (rename, duplicate, delete), and the selected layout's elements in z-order.
- **Properties pane:** a new target `{ kind: "layoutElement", layout, id }` with each element's settings. It reuses the existing Fluent field components.
- **Panning inside a frame:** "Activate" the frame (double-click), then drag to pan and scroll to step through standard scales. Each step re-renders a low-dpi preview.
- **Export dialog:** format, dpi (PNG 72–600, default 300; rasters inside PDF capped at 300), file path (Tauri save dialog). It runs as a `progress::Task` "Exporting layout — <name>" with Cancel, checked between layers and before encoding.

### L6. What the sources element shows
`sources` lists the citations (via `sources.rs`) of every layer drawn in its frame, including derived layers' inputs (they already carry them). Short form: "Data: U.S. Census Bureau TIGER/Line 2024; USGS 3DEP…". This satisfies "every map cites its data sources". A new layout with a frame gets a Sources element by default.

---

## 3. Engine layout (files)
```
crates/engine/src/layout/
  mod.rs      // pub API: save/list/delete, render_preview, export
  model.rs    // L2 serde types + validation + LayerStyle mirror
  geo.rs      // Body, k, convergence, graticule/grid line generation
  frame.rs    // per-layer SQL → SVG paths (transform, clip, simplify, style)
  surround.rs // text, legend, scale bar, north arrow, sources
  svg.rs      // SVG writer (escaping, clip paths, fonts by family name)
  export.rs   // SVG → PDF (svg2pdf) / PNG (resvg)
resources/fonts/  Noto Sans Regular/Bold/Italic (OFL-1.1, add to notices)
```
- Rasters in frames (C2.4): refactor `raster::render_tile` into `render_extent(info, crs, extent, w_px, h_px)` (same colour ramp), with `render_tile` calling it. Embed as an SVG `<image>` data URI.
- New Tauri commands (each also in the devserver and `api.ts`): `list_layouts`, `save_layout`, `delete_layout`, `render_layout_preview` (returns PNG bytes + warnings), `export_layout`. Preview images may also be served over `gis://localhost/layout/<id>.png?v=<hash>&dpi=` if returning bytes over IPC is slow. Measure in C2.2 and choose.

---

## 4. Build sessions (Sonnet)

### C2.1 — Engine renderer core (engine only, no UI)
Licence gate (L1) first. Then `model.rs`, `geo.rs` (Body, k, convergence), `frame.rs` (vector layers, single/categorized/graduated styles, opacity, point radius), `svg.rs`, `export.rs`, bundled fonts. Engine tests only.

Acceptance:
- P1. The licence gate passes with the new crates, and the third-party notices are regenerated.
- P2. A fixture polygon/line/point layout in EPSG:26915 at 1:24,000 renders to SVG. Its PDF contains vector path operators, embedded font subsets and extractable text, and **no image XObject** when there are no rasters.
- P3. Determinism: rendering twice gives the same SVG bytes. A golden SHA-256 test is checked on all three CI platforms.
- P4. At UTM 15N, lon −96° (3° west of the central meridian −93°), lat 45°:
  - k ≈ 1.00029 (0.9996 × (1 + (Δλ cos φ)²/2 + …));
  - meridian convergence magnitude ≈ 2.12° (≈ Δλ sin φ).
  Both must match PROJ's own `proj_factors` values within 1e-6 and 0.001°. Use `proj_factors` as the reference in the test, not hand constants.
- P5. Frame CRS missing → clear error. A layer with no CRS → skipped, and reported in the warnings.
- P6. A 100k-polygon layer renders a letter page to PNG at 150 dpi in < 1.5 s release (`#[ignore]` perf test; record the number).

### C2.2 — Layout view UI + commands
Commands (Tauri + devserver + `api.ts`), `__layouts` storage, `ui/src/store/layout.ts` (zundo), Layout document tab, preview image, select/move/resize, Properties target, contextual ribbon tab (Page, Insert Map Frame, Map Frame group, Export PDF/PNG/SVG), Contents "Layouts" section.

Acceptance:
- P7. New Layout → Letter landscape → Insert Map Frame → Set from Map View shows the map's layers in the frame's CRS. Save, reopen the project, and the layout is intact.
- P8. Move/resize/undo/redo work. Undo history is per layout (vitest on store actions).
- P9. Export PDF from the browser build writes a file, with a progress bar and working Cancel.
- P10. A browser check with one screenshot of the Layout view.

### C2.3 — Surround elements
Text with tokens, legend (from the frame's layers' styles, including categorized/graduated classes), scale bar (L3), north arrow (L3), sources (L6).

Acceptance:
- P11. Scale bar: inverse-project the drawn bar's end points at the frame centre; the geodesic distance equals the label within 0.1% (UTM 15N, State Plane in US feet, and Albers CONUS fixtures).
- P12. The north arrow angle equals the computed convergence within 0.01°, and follows frame rotation.
- P13. The legend lists exactly the visible frame layers in draw order, with class labels.
- P14. The sources text includes the TIGER and 3DEP citations for a frame showing those layers, including a derived layer (via its inputs).
- P15. The scale-variation warning appears for a 1:5,000,000 Web Mercator frame at 60°N.

### C2.4 — Graticule/grid, rasters, basemaps
Graticule (densified lat/lon lines transformed to the frame CRS, clipped, edge labels via `Body`), projected grid in frame CRS units, raster layers via `render_extent`, USGS basemaps via GDAL's WMS/XYZ driver warped to the frame CRS (OSM excluded from print: its tile policy forbids bulk downloads. The UI disables it with a tooltip). Each basemap used records its citation.

Acceptance:
- P16. Graticule lines in a Lambert Conformal frame are curved correctly. A point on the 45°N line inverse-projects to 45° ± 1e-6.
- P17. A hillshade raster appears in PDF as one image clipped to the frame, ≤ 300 dpi.
- P18. A USGS Topo basemap renders in a UTM frame and its citation appears in Sources.
- P19. Then rebuild the Mac app and run `--self-test`.

### C3.1 — Templates
Bundled templates in `resources/layout_templates/*.json` (6 page sizes × 2 orientations, with frame, title, legend, scale bar, north arrow, sources), "Save as Template" to the app data dir (path via `resources.rs`, no hard-coded paths), and a New Layout gallery that shows both.
- P20. Every bundled template validates and renders (test loops over the folder).

### C3.2 — Map series
`series: { index_layer, name_field, sort_field, extent: "fit" | "fixed_scale", margin_pct, scale? }`. The frame with `follows_series: true` centres on each index feature. Tokens get `{page_*}`. Export goes to one multi-page PDF or one PNG per page, with the progress bar counting pages and Cancel between pages. Optional: other features of the index layer greyed out ("highlight current page").
- P21. A 10-county index layer gives a 10-page PDF in sort order. Each page's frame contains its county and the title shows the county name.
- P22. Cancel after page 3 leaves no partial file (write to a temp file, rename on success).

**Exit for C (C2–C3):** P1–P22 pass, CI green on all three platforms, and a user can produce a print-scale PDF map with legend, scale bar, north arrow, graticule and sources.

**Later, not in C2/C3:**
- GeoPDF georeferencing (OGC best-practice Measure dictionary: Avenza/field use). It's a small addition to `export.rs`; schedule it with C4.
- Feature labels (no labelling engine exists yet; needs collision handling). Plan it with C4, which needs contour and place labels.
- Grid-north/magnetic declination diagram (C4).

**Order and parallel safety:**
- C2.1 is engine-only and touches only new files plus `Cargo.toml` and `lib.rs` exports. It can run beside D3 (STAC) if D3 doesn't touch `lib.rs` at the same time. It can't run beside B2.1 or B2.2 (both edit `lib.rs`).
- C2.2 adds Tauri commands. It is never parallel with another command-adding task.
