# Projected map view — plan

_Opus design, 2026-10-23. Requested by the user: when the map's coordinate system changes, the map view must draw in that CRS. Build sessions M1–M4 are in §3. Order: after C2.3 + C2.4, before C3 (see MASTER_PLAN)._

## 1. Starting point
- The map view is MapLibre GL 6.15, which draws only `mercator`, `globe` and `vertical-perspective`. `mapCrs` (ribbon **Coordinate System**) changes only the status-bar coordinates, measurements and the layout default. The view stays in Web Mercator.
- **Vector tiles:** `tiles.rs` builds MVT tiles on the Web Mercator grid (`ST_TileEnvelope`) from each layer's `__g` column, which is Web Mercator geometry with an R-tree index, generalized per zoom.
- **Raster tiles:** `raster::render_tile` renders on the same grid. C2.4 adds `render_extent(info, crs, extent, w, h)`.
- **Map code:** `ui/src/panes/MapView.tsx` (events, tools), `ui/src/map/mapController.ts`, `ui/src/map/style.ts` (MapLibre style JSON, with a vitest), plus `getMap()` callers in `actions.ts`, `StatusBar.tsx`, `layout/actions.ts`, and the National Map and Elevation dialogs. About 630 lines use MapLibre directly.
- **Planetary:** PLANETARY_PLAN D2 planned Mercator plus a "pole spike". Mercator can't show the lunar south pole.

## 2. Decisions

### M-D1. One map library for every CRS: OpenLayers
Replace MapLibre with **OpenLayers** (`ol`, BSD-2-Clause). Reuse the existing style JSON through **`ol-mapbox-style`** (BSD-2-Clause).
- **Why:**
  - OL draws any CRS that proj4 can describe. `proj4` is already a dependency.
  - It takes custom tile grids, reprojects raster/XYZ tiles in the client, reads MVT, and applies MapLibre style JSON, including `step`/`match`/`interpolate` expressions.
  - One library means one path per tool. Polar stereographic views fix the planetary pole problem.
- **Rejected:**
  - *MapLibre for Web Mercator plus a second view for other CRSs:* every tool (identify, select, measure, draw, highlight) would exist twice, and the two views would drift apart.
  - *MapLibre with "fake Mercator" tiles in CRS space:* MapLibre's lon/lat would be wrong for every tool.
  - *A canvas fully rendered by the engine:* too slow for interactive pan and zoom on large layers.
- **Cost:**
  - No 3D globe. Planetary work uses polar and equirectangular CRSs instead; PLANETARY_PLAN D2 is superseded, see §5.
  - Canvas rendering instead of WebGL. Engine tiles are already capped (25k features per tile) and generalized, so canvas is fast enough. If a 3M-feature layer pans noticeably worse than today, try OL's WebGL vector-tile layer (M4).

### M-D2. Tile grid per map CRS
- **Web Mercator:** unchanged. The same XYZ grid, tiles and cache, so the default path stays byte-identical.
- **Projected CRSs:**
  - **Extent:** the CRS's area of use (PROJ), transformed to the CRS and densified to 64 points per edge. Then expand it to a square around its centre.
  - **Resolutions:** `extent_width / 256 / 2^z`, z = 0–22.
  - **Origin:** the top-left of the square.
  - **No area of use:** fall back to the union of the visible layers' extents plus 10%, and tell the user.
- **Geographic CRSs** (EPSG:4326, NAD83 …): OL's native degree grid (2:1 at z0).
- **Grid key:** `crs_key = sha256(normalized PROJJSON)[..12]`. The engine computes the grid once per CRS and returns it to the UI (M-D6), so the UI never derives a grid itself.

### M-D3. Display geometry per (layer, CRS)
Tiles in a non-Mercator grid need geometry in that CRS. Transforming `__g` for every tile fails near the poles, because Web Mercator is undefined there. Instead:
- **Display table:** build a display table `__disp_<layer>_<crs_key>` lazily, from the layer's **source** geometry (source CRS → map CRS). It keeps `__fid`, a `__d` geometry column with an R-tree index, and the bbox columns `__x0`/`__y0`/`__sz`. It uses the same per-zoom generalization as `tiles.rs`.
- **Clipping:** first clip to the CRS's area of use, densified and transformed to the source CRS.
  - Features fully outside aren't drawn. The count is reported once per layer and CRS ("412 features of Counties are outside the area of use of NAD83 / UTM 15N and aren't drawn"). This follows the rule to report dropped features.
  - Features partly outside are clipped.
- **Build:** a `progress::Task` "Projecting <layer> for display", cancellable. The UI shows the layer as "projecting…" until it's done.
- **Lifetime:** like the node cache. Indexed in `__disp_index`, dropped when the layer's data changes (edit, definition query, re-import) or the layer is removed. Kept across CRS switches. Bounded by the same 2 GB budget as node snapshots, evicting the least recently used. Never saved into the project when it's older than the layer.
- Web Mercator keeps using `__g`; no display table.

### M-D4. Rasters and basemaps
- **Engine rasters** (imports, 3DEP, hillshades): tiles come from `render_extent` (C2.4) on the M-D2 grid. Web Mercator keeps `render_tile`.
- **Online basemaps** (USGS XYZ in Web Mercator): OL reprojects them in the client. OSM stays display-only, as it is today.

### M-D5. Coordinates: proj4 only for pixels, the engine for every number
- **proj4 in the UI:** maps the screen to the map CRS only (OL view, pointer position, drawing).
- **The engine for every reported number:** status-bar lon/lat and coordinates, identify, measurements, created feature vertices, selection queries. The engine's PROJ has the datum grids proj4 lacks; NAD27 can be off by metres in proj4.
- **The rule for UI code:** anything shown as a number or stored goes through `api.transformPoint`, or an engine query that takes `crs`.
- **Scale:** the status-bar scale `1:x` uses the true scale factor at the view centre (`layout::geo::factors`), so a UTM map at its edge reads correctly.

### M-D6. Engine API (each one Tauri + devserver + `api.ts`)
- **`crs_display_info(crs)`** → `{ crsKey, proj4, geographic, unitM, grid: { extent, origin, resolutions, tileSize }, areaOfUse: [w, s, e, n] }`. Fails with a readable message when proj4 can't express the CRS; the UI then keeps the previous CRS.
- **Tile URLs** gain the key: `gis://tiles/<layer>/<crsKey>/{z}/{x}/{y}.pbf` and `gis://raster/<layer>/<crsKey>/{z}/{x}/{y}.png`. `crsKey = "3857"` keeps today's path.
- **`display_status(layer, crs)`** → `{ ready, building, dropped }`, for the "projecting…" state and the dropped-features message.
- **Queries that take a `crs` argument for their input geometry:** identify, rectangle select, select by location and create features. Selection queries and the identify tolerance are in map-CRS units.

### M-D7. CRS changes in the UI
- Changing the map CRS keeps the view centre (transformed) and the ground scale at the centre. Layers redraw as their display tables become ready.
- The first layer added still sets the map CRS when none is set. Old projects that saved a non-Mercator `mapCrs` now open in that CRS. Note it in the release notes.
- The Coordinate System picker gets a quick "Web Mercator (EPSG:3857)" entry.
- The layout's "Set from Map View" uses the view's centre and resolution in the map CRS directly.

## 3. Build sessions (Sonnet)

### M1 — Engine: CRS grids, display tables, tiles in any CRS
- **Scope:** `crs_display_info`, `__disp_*` tables with `__disp_index` (M-D3), MVT for a CRS grid, the tile-protocol route with `crsKey` (src-tauri `tiles.rs`), the devserver route, `display_status`, and `api.ts` types.
- **Acceptance:**
  - P1. The 3857 tile bytes for a fixture layer are identical to before.
  - P2. A UTM 15N (EPSG:26915) tile at a known grid position contains the fixture feature at the expected tile coordinates (±1 px).
  - P3. An Antarctic polar stereographic (EPSG:3031) tile containing the pole renders a polygon around the pole without NaN or infinite coordinates.
  - P4. A world layer displayed in UTM 15N reports the dropped count and draws no features outside the area of use.
  - P5. Editing the layer drops its display tables. A layer edit while a display build runs cancels the build cleanly.
  - P6. 1M points: the display table builds in < 10 s (release), and a dense tile renders in < 300 ms.
- **Risky (Opus review):** DuckDB writes and transactions for the display tables and index; CRS math for the grid.

### M2 — UI: swap MapLibre for OpenLayers
- **Scope:**
  - **Licence gate first:** add `ol` and `ol-mapbox-style`, then run `python3 scripts/third_party_notices.py`. If any npm dependency isn't permissive, stop: OPUS NEEDED.
  - **View:** an OL map with proj4 registration from `crs_display_info`, the M-D2 tile grid, vector-tile layers styled through `ol-mapbox-style` from `style.ts` (the style vitest still passes), and XYZ basemaps.
  - **Coordinates and CRS:** the coordinate readout through the engine (M-D5), CRS switching (M-D7), and the "projecting…" state with the dropped-features message.
  - **Not in M2:** tools are untouched beyond compiling. Mark them "being ported" if needed.
- **Acceptance:**
  - P7. A browser check (devserver + vite) shows the same project in Web Mercator, UTM 15N and EPSG:5070 Albers, with screenshots. Straight UTM grid lines show as straight.
  - P8. The status bar's lon/lat under a known vertex matches the engine to 1e-7°.
  - P9. CRS switching keeps the centre within 1 px and the scale within 0.5%.

### M3 — UI: port the map tools
- **Scope:** click identify, rectangle select, select by location on the map, the measure tool, create features and drawing, the selection highlight (style function on the selected fids), zoom to layer or selection, and "Set from Map View". `getMap()` callers move to an OL-backed `mapController`. Remove `maplibre-gl` from `package.json`, then regenerate the notices.
- **Acceptance:**
  - P10. Every tool works in UTM 15N and in Web Mercator (browser checks).
  - P11. A feature drawn in UTM 15N into a WGS 84 layer stores vertices that round-trip to the clicked map coordinates within 1 mm.
  - P12. Measure: a 10 km line in UTM 15N measures 10 km ± 1 m geodesic.
  - P13. No `maplibre` import remains (grep test).

### M4 — Rasters in any CRS, performance, polar check
- **Scope:** engine raster tiles on the M-D2 grid via `render_extent` (needs C2.4); hillshade and 3DEP in UTM. If 3M points pan noticeably slower than the Web Mercator baseline, try OL's WebGL vector-tile layer behind the same API. Then rebuild the Mac app and run `--self-test`.
- **Acceptance:**
  - P14. A 3DEP hillshade lines up with a vector layer in UTM 15N within 1 pixel at z14.
  - P15. EPSG:3031 shows a raster across the pole.
  - P16. Pan and zoom on 3M points in UTM 15N stay interactive (no frame over 200 ms in a performance trace).

**Order:** M1 → M2 → M3 → M4, one at a time:
- M1 adds Tauri commands, so it never runs in parallel with another command-adding task.
- M2 and M3 rewrite the map pane, so they don't run beside any map-UI task.
- M4 needs C2.4's `render_extent`.

## 4. Risks
- **ol-mapbox-style gaps:** check every expression `style.ts` emits (the M2 style test covers it). Fall back to a small hand-written OL style function for any unsupported one.
- **proj4 can't express a CRS:** rare projections. The engine says so in `crs_display_info`, and the UI keeps the previous CRS with a message.
- **Display-table size:** a multi-GB layer in a new CRS takes another copy. The budget and LRU eviction bound it; the progress bar and Cancel make it visible.

## 5. Effects on other plans
- **PLANETARY_PLAN D2** (Mercator display plus the MapLibre pole spike) is superseded. Bodies use polar stereographic and equirectangular CRSs from PROJ's IAU catalogue through the same display path. F0 no longer needs the pole spike. There's no 3D globe.
- **LAYOUTS_PLAN:** "Set from Map View" becomes exact (same CRS). Nothing else changes.
