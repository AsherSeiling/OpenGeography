# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**. The general-purpose prompt is `docs/prompts/sonnet-runner.md`.

> Status 2026-10-23: Phase A is down to A3 (your hands-on installer check) and tagging v0.1.0, neither of which conflicts with these. B2–B4 are done, C2.1 (layout renderer core) and C2.2 (Layout view, commands, storage) are done, so C2.3 + C2.4 (one session, two commits) is the next Sonnet task. After it: the projected map view M1–M4 (`docs/MAP_PROJECTION_PLAN.md`).

---

## 1. C2.3 + C2.4 — Surround elements, then graticule/grid, rasters and basemaps (layouts, parts 3–4)

C2.1/C2.2 are finished: the engine renders map frames and text to SVG/PDF/PNG (`crates/engine/src/layout/`), stores layouts in `__layouts`, and the Layout view edits them (`ui/src/layout/`, `ui/src/store/layout.ts`). Legend, scale bar, north arrow and sources elements exist in the model (`model.rs` `Element::Legend` etc.) but `render_svg` only emits a "drawn in a later release" warning for them.

Also fix (from the C2.2 review): the Properties pane shows float noise in mm and coordinate fields (e.g. "253.99999999999997"). Round displayed values: 0.1 mm for sizes and positions, 0.01 CRS units for the centre, without changing stored values unless the user edits them.

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/LAYOUTS_PLAN.md (§2 L3, L6 and §4 C2.3). Read only crates/engine/src/layout/{mod,model,geo,frame,svg}.rs (geo.rs already has `factors` (k + convergence), `bar_length_mm`, `frame_extent`, `Body`), crates/engine/src/sources.rs (`list`, `inherit`; derived layers already carry their inputs' citations), crates/engine/tests/layout.rs (fixture helpers), ui/src/layout/{types.ts,actions.ts,ElementProperties.tsx}, ui/src/ribbon/{Ribbon.tsx,commands.tsx} (Layout tab). Grep before opening other files.

TASK C2.3: legend, scale bar, north arrow, sources, text tokens.
- Engine: new `layout/surround.rs`, called from `render_svg` in place of the warning. All maths takes `&Body`; no 6378137 literal outside geo.rs (a grep test exists).
  - Scale bar: ground distance D km drawn `D·1000·k/scale·1000` mm long at the frame centre (k from `geo::factors`); segments, units (km/mi/m/ft), label "1:N" optional. Pick a round D that fits the element's width.
  - North arrow: rotation = meridian convergence at the frame centre + the frame's rotation (sign convention: test it against `factors`). Styles "simple" and "compass".
  - Legend: from the frame's drawn layers in draw order (top first), only visible ones; single → swatch (polygon/line/point per geometry), categorized/graduated → one row per class with its label. Needs the layer's geometry type: `lookup(id)`. Title, columns, font from the bundled Noto Sans, text clipped/ellipsized to the element width. Report (warning) when content does not fit the element's height.
  - Sources: the citations (`sources.rs`) of every layer drawn in the frame, derived layers via their inputs, in a short form ("Data: U.S. Census Bureau TIGER/Line 2024; USGS 3DEP…"), wrapped to the element width at `size` pt. Add `Engine` access so `render_layout_svg` can pass citations in (keep `render_svg`'s signature testable: a `sources: &dyn Fn(&str) -> Vec<String>` closure like `lookup`).
  - A new layout that gets a Map Frame also gets a Sources element (L6) — UI side in `addMapFrame`.
- UI: ribbon Layout → Insert: Legend, Scale Bar, North Arrow, Sources (each attaches to the selected/first map frame, default sizes/positions from the plan); Properties fields (legend title/columns, scale bar units/segments, north arrow style, sources size); mirror the new fields in `types.ts`.
- Warning chip for scale variation is already emitted by the engine; make sure the Layout view shows it for the P15 case.

Acceptance: P11–P15 in the plan (scale bar vs inverse-projected geodesic distance within 0.1% for UTM 15N, State Plane in US feet, Albers CONUS fixtures; north arrow angle within 0.01° of convergence and follows frame rotation; legend lists exactly the visible frame layers in draw order with class labels; sources text includes TIGER + 3DEP citations including a derived layer; scale-variation warning for a 1:5,000,000 Web Mercator frame at 60°N). Add a golden-hash test for a page with all elements (update with GOLDEN_PRINT=1; the hash must match on CI's three platforms, so avoid anything platform-dependent). One browser screenshot (devserver + vite) of the Layout view with all four elements, saved under docs/screenshots/.

Rules: no new Tauri commands expected (if you add one, add it to src-tauri, crates/devserver and ui/src/ipc/api.ts); explicit CRS, never guess; GIS correctness first; Fluent UI v9 + existing ribbon style; no GPL code.

Test narrow, then cargo test -p engine, npx vitest run && npx tsc -p tsconfig.json, cargo build -p gis-studio -p devserver. Commit C2.3 on its own. Then do **C2.4** in the same session: docs/LAYOUTS_PLAN.md §4 C2.4 (graticule via `Body`, projected grid, raster layers through `raster::render_extent(info, crs, extent, w_px, h_px)` with `render_tile` calling it, USGS basemaps via GDAL WMS/XYZ warped to the frame CRS with citations; OSM disabled for print). Keep `render_extent`'s signature exactly as in the plan: the projected map view (MAP_PROJECTION_PLAN M4) reuses it. Acceptance P16–P18 (skip P19, the Mac rebuild; the orchestrator does it). Basemap tests that need the network are `#[ignore]` with a local fixture for the default run. Test again, then commit C2.4 separately. Update PROJECT_STATUS.md (short), add DECISIONS.md entries, and replace this block with a brief for **M1** (docs/MAP_PROJECTION_PLAN.md §3 M1), built the same way.
```
