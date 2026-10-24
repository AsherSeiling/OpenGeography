# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**. The general-purpose prompt is `docs/prompts/sonnet-runner.md`.

> Status 2026-10-23: M1 (engine) is merged; M2 (OpenLayers view in any CRS) is done on branch `claude/m2-openlayers-view-cc786c`, awaiting Opus review. Next is M3 (`docs/MAP_PROJECTION_PLAN.md`). A3 (your installer check) and the `v0.1.0` tag are independent.

---

## 1. M3 — UI: port the map tools (projected map view, part 3)

M2 left the map drawing in any CRS (`panes/MapView.tsx`, `map/mapController.ts`, `map/crs.ts`, `map/readout.ts`, `map/olStyle.ts`) with identify, select, box select, measure and land-nav still working from engine lon/lat. What is still approximate (M2 decisions in `docs/DECISIONS.md`): identify and select take a lon/lat and a MapLibre-style zoom made by `legacyZoom`; box select sends the lon/lat envelope of its four corners; the identify tolerance isn't in map-CRS units; the sketch is drawn from lon/lat with proj4 (display only).

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/MAP_PROJECTION_PLAN.md (§2 M-D5, M-D6, §3 M3, R3). Then ui/src/panes/MapView.tsx (the tool effects), ui/src/map/mapController.ts, and grep `identify`, `select_box`, `select_by_location`, `getViewBounds`, `legacyZoom` before opening other files.

TASK M3 as the plan says, on top of M2: queries that take a `crs` argument for their input geometry (identify, rectangle select, select by location, create features/drawing if the map has them yet), the selection tolerance and the identify tolerance in map-CRS units, measure with vertices stored through the engine, zoom to layer/selection and "Set from Map View" using the view's centre and resolution in the map CRS directly (no lon/lat detour), then drop `legacyZoom` and `groundForLegacyZoom` from `map/crs.ts`. Each new or changed Tauri command also goes into crates/devserver and ui/src/ipc/api.ts. P13: add a grep test that no `maplibre` import remains. R3: replace MapLibre wording in README.md, docs/prompts/sonnet-runner.md and the current-state lines of docs/DECISIONS.md and docs/MASTER_PLAN.md (leave history).

Acceptance P10–P13 of the plan, plus browser checks (devserver + `npx vite --port <free port>`; another session may hold 1420) with screenshots for UTM 15N and Web Mercator. Engine-side changes to coordinate maths are an Opus review hunk: say so in the review block.

Test: npx vitest run && npx tsc -p tsconfig.json; cargo test -p engine; cargo build -p gis-studio -p devserver. Commit M3 on its own; update PROJECT_STATUS.md (short) and DECISIONS.md; replace this block with a brief for **M4**.
```
