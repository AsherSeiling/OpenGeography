# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**. The general-purpose prompt is `docs/prompts/sonnet-runner.md`.

> Status 2026-10-23: C2.3 and C2.4 (layout surround elements, graticule/grid, rasters, basemaps) are done on the branch `claude/c2-3-4-layout-elements-0bebf9` (the orchestrator reviews, merges and rebuilds the Mac app). Next is the projected map view, M1 first (`docs/MAP_PROJECTION_PLAN.md`). A3 (your installer check) and the `v0.1.0` tag are independent.

---

## 1. M1 — Engine: CRS grids, display tables, tiles in any CRS (projected map view, part 1)

The map view is MapLibre (Web Mercator only). The plan replaces it with OpenLayers in the map's CRS (M2–M4). M1 is engine and commands only: no UI change. C2.4 already added `raster::render_extent(info, crs, extent, w_px, h_px)` (the raster tile path for M4) and `layout::geo::FrameCrs` (PROJ transforms and area checks you can reuse for the area of use).

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/MAP_PROJECTION_PLAN.md (§2 M-D2, M-D3, M-D6 and §3 M1). Read only crates/engine/src/tiles.rs, the tile protocol in src-tauri/src (grep `gis://`, `tiles`), crates/devserver/src/main.rs tile routes, the `Engine::tile` cache in crates/engine/src/lib.rs, crates/engine/src/crs.rs, crates/engine/src/layers.rs (where `__g` is built) and ui/src/ipc/api.ts. Grep before opening other files.

TASK M1, as the plan says: `crs_display_info(crs)` (proj4 string, grid from the area of use, key = sha256 of normalized PROJJSON, 12 hex chars), `__disp_<layer>_<crs_key>` tables with `__disp_index` and an R-tree (built from the layer's source geometry, clipped to the area of use, per-zoom generalization like tiles.rs, a cancellable `progress::Task` "Projecting <layer> for display"), MVT tiles for a CRS grid, tile route with `crsKey` (`gis://tiles/<layer>/<crsKey>/{z}/{x}/{y}.pbf`; `3857` keeps today's path and today's bytes), `display_status(layer, crs)` → { ready, building, dropped } and `api.ts` types. Every Tauri command is also added to crates/devserver and ui/src/ipc/api.ts.

Acceptance P1–P6 of M1 in the plan (Web Mercator tile bytes unchanged; UTM 15N tile has the fixture feature within 1 px; EPSG:3031 pole tile has no NaN/inf; a world layer in UTM 15N reports the dropped count; editing a layer drops its display tables and cancels a running build cleanly; 1M points: table < 10 s and a dense tile < 300 ms in release, as `#[ignore]` perf tests with the numbers recorded).

Rules: DuckDB writes and transactions for the display tables and index, and the grid maths, are the risky hunks: keep them small, list them in the review block, and do not push. Explicit CRS, never guess; report dropped features; geodesic where distance matters; no GPL code; no new UI.

Test narrow, then cargo test -p engine, npx vitest run && npx tsc -p tsconfig.json, cargo build -p gis-studio -p devserver. Commit M1 on its own. Update PROJECT_STATUS.md (short), add DECISIONS.md entries, and replace this block with a brief for **M2** (docs/MAP_PROJECTION_PLAN.md §3 M2), built the same way.
```
