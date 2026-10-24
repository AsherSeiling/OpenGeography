# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**. The general-purpose prompt is `docs/prompts/sonnet-runner.md`.

> Status 2026-10-23: C2.3/C2.4 are merged. M1 (engine: CRS grids, display tables, tiles in any CRS) is done on branch `claude/m1-crs-tiles-080e54`, awaiting Opus review. Next is M2 (`docs/MAP_PROJECTION_PLAN.md`). A3 (your installer check) and the `v0.1.0` tag are independent.

---

## 1. M2 — UI: swap MapLibre for OpenLayers (projected map view, part 2)

The engine now serves what M2 needs (M1): `api.crsDisplayInfo(crs, fallbackExtent?)` → grid, proj4, key; `api.buildDisplay(layer, crs)` starts the "Projecting…" build; `api.displayStatus(layer, crs)` → { ready, building, dropped, error }; `tileUrl(layer, fields, crsKey)`. Call `crsDisplayInfo` before any tile request for a key (the engine learns key → CRS from it). A tile of an unbuilt display table is a 404: build first, poll `displayStatus`, then refresh the layer source. Rasters in non-Mercator CRSs are M4.

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/MAP_PROJECTION_PLAN.md (§2 M-D1, M-D5, M-D7, §3 M2, review notes R1, R2). Read ui/src/panes/MapView.tsx, ui/src/map/mapController.ts, ui/src/map/style.ts (+ its vitest), ui/src/ipc/api.ts (CrsDisplayInfo, DisplayStatus, tileUrl) and grep `getMap(` before opening other files.

TASK M2 as the plan says: licence gate first (add `ol` and `ol-mapbox-style`, run `python3 scripts/third_party_notices.py`; any non-permissive npm dependency → OPUS NEEDED), then an OpenLayers map with proj4 registered from `crsDisplayInfo`, the engine tile grid, vector-tile layers styled via ol-mapbox-style from style.ts, XYZ basemaps, engine-backed status-bar coordinates (R2: one transform in flight, ~20 Hz), CRS switching keeping centre and scale (M-D7), and the "projecting…" state with the dropped-features message. Tools only need to compile ("being ported" if needed).

Acceptance P7, P8, P8b, P9, P9b of the plan. Browser checks (devserver + vite) with screenshots for Web Mercator, UTM 15N, EPSG:5070.

Test: npx vitest run && npx tsc -p tsconfig.json; cargo build -p gis-studio -p devserver. Commit M2 on its own; update PROJECT_STATUS.md (short) and DECISIONS.md; replace this block with a brief for **M3**.
```
