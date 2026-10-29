# Session report and plan — 2026-10-28 (lead-developer session)

Standing orders: finish R → E; **no GitHub CI until all phases are done** (push with `[skip ci]`); one **local desktop build** (macOS `scripts/package-macos.sh`) when Phase R is complete; cross-platform stays a design rule (no OS checks outside `resources.rs`/`raster.rs::store_dir`), the CI/Windows/Linux verification is the final push. M4 on hold; tags and A3 are the user's.

## 1. Plan for the rest of the session

| # | Block | Exit criteria | Status |
|---|---|---|---|
| 1 | R6 leftovers: Tissot layer, layer source panel, layout `{datum}`/`{units}` tokens, picker recents | R-H9, R-H10, R-H11, R-I5 automated | **done** (area-of-use data chip left, G15) |
| 2 | R8 world view: world-mode display tables, graticule / outline / standard-parallel layers, bundled Natural Earth 110m outline, projection builder | R-W cases automated | **done** (R-W10 colouring, R-W11 perf deferred) |
| 3 | R7 prep + **local macOS build** (`package-macos.sh`, `--self-test`, copy `.app` + DMG, `cargo clean`) | build runs, self-test passes | **done** (self-test OK after fixing P-1; `cargo clean` at session end) |
| 4 | Phase D: D0 tiled DEM, D1 ArcGIS REST catalog, D2 Census + keychain, D3 STAC/COG, D4 GeoParquet, D5 OGC API, D6 JSON APIs/SSURGO/WMS | each with mock-service tests; network tests `#[ignore]` | **D0–D6 done** (WMS/WMTS and NASA POWER deferred) |
| 5 | Phase E: E1 raster calculator/reclassify/NoData/band select, E2 resample/align/mosaic/rasterize, E3 cancellable gdaldem | tests per tool, added to the CRS matrix guard | planned |
| 6 | Bookkeeping at each block: TEST_PLAN, TEST_PLAN_FULL, DECISIONS, BUGS, PROJECT_STATUS, roadmap, Claude Doc rows | docs match code | continuous |

Rule per block: acceptance criteria → smallest change → focused tests → broader tests → commit on `claude/<task>` → merge `--no-ff` with `[skip ci]`.

## 2. Features added this session (all merged to `main`)

**Q3 — Layout Properties pane and large DEM download**
- Fix for the Properties pane going blank (U-5, root cause in BUGS); LP-0 tracer.
- `plan_elevation` guard for 3DEP downloads (size, requests, resolution used; confirm above 500 MB) — D-1.

**L1–L3 — Labels**
- L1: feature labels on the map (field, size, colour, halo, scale range, declutter; polygon anchors from the engine via `__anchors`).
- L2: labels in print layouts with page collision handling; unplaced labels reported.
- L3: per-layout label overrides; move, rotate, edit text, hide, reset; pinned labels placed first.

**B6 / B7**
- B6: Split by Attribute — one layer or one file per value (QGIS `native:splitvectorlayer` reimplemented), confirm above 200 outputs, limit 1000, never overwrites a source file.
- B7: drag-to-reorder categories in unique-value symbology; the order sets draw order and legend order.

**C3.0–C3.2 — Layout polish, templates, series**
- Rulers, Arrange group (order/align), scale bar "1:N", dpi box, basemap tile disk cache.
- 10 built-in layout templates; save/delete user templates.
- Map series (atlas): one page per index feature, following frame, multi-page PDF / PNG per page export.

**R8 — World view**
- Map tab: World View toggle (any CRS over the whole Earth, cut at the opposite side of its central meridian, latitude limit stated), Graticule (curved lines plus central meridian, standard parallels, edge of map), World Outline (bundled Natural Earth 110m, cited), Projection Builder (conic / cylindrical / azimuthal / world families with sliders and validation).

**D0 — Tiled large-area elevation download**
- Elevation download for any area: cut into service-sized tiles on one cell grid, retried, resumable, cancellable, joined into one compressed GeoTIFF; UTM for one zone, geographic for wider/polar areas; coarsened with a stated note above 10⁹ cells; confirmation above 500 MB; the Elevation dialog shows the engine's plan (size, requests, CRS, notes).

**D1 — ArcGIS Services**
- Data → ArcGIS Services: PAD-US, FEMA flood zones and boundaries, National Wetlands Inventory and current wildfire perimeters (endpoints verified live), or any pasted MapServer/FeatureServer layer; each cited in Sources.

**D2 — Census Data**
- Data → Census Data: ACS 5-year/1-year and 2020 Census values joined by GEOID to the active layer as a new layer; API key kept in the OS keychain (Tauri shell only); suppressed values stored as no value and counted.

**D3–D6 — more public data**
- Data → Imagery & DEM (STAC): Sentinel-2 and Copernicus DEM scenes searched by area, date and cloud cover, clipped from the cloud; Overture Maps (buildings, places, roads, water, land use, boundaries); OGC API Features (USGS Water Data sites or a pasted service); Earthquakes & Soils (USGS catalog, SSURGO map units).

**Packaging**
- The packaged macOS app now carries the NOAA NADCON datum grids (bug P-1, found by the start-up datum check).

**R0–R6 — Coordinate-system hardening**
- Datum shifts fixed (C-1): PROJ data folder located at start-up, known-answer datum check (start-up alert and `--self-test`).
- CRS identity by meaning (`crs::key`/`same`) replaces string comparison everywhere; importer fixes (C-4, C-5, C-7, C-8, G1).
- Reference suite against `cs2cs` (PROJ 9.8.1) with a written tolerance table; tool matrix of 33 recipes × 8 CRSs with a guard that every tool is covered.
- Geographic-CRS fixes: triangulation/voronoi/hull in UTM, project tool counts unprojectable features, GeoJSON export reprojects to WGS 84, terrain on geographic DEMs, pole-safe identify/select.
- Raster CRS fidelity (modified definitions kept, all-NoData rasters accepted).
- R6b: Tissot circles (Map tab), Coordinate System tab shows code/unit/datum/area/"reprojected", layout text tokens `{crs}` `{datum}` `{units}`, recent coordinate systems in the picker.
- R6 core: `crs_info` and `map_factors` commands; picker previews custom definitions; status bar shows k, γ and an "outside area of use" notice.

## 3. Test coverage of this session's features

| Feature | Engine tests | UI unit (vitest) | Browser e2e |
|---|---|---|---|
| Q3 Properties pane / DEM guard | `downloads.rs` (plan_elevation) | `store/layout.test.ts` | `layout-props.spec.ts` |
| L1 map labels | `display.rs`, `tiles` anchors | `map/labels.test.ts` | `labels.spec.ts` (L1) |
| L2/L3 layout labels | `layout.rs` | `layout/labelEdits.test.ts` | `labels.spec.ts` (L2, L3) |
| B6 split | `split.rs` | `dialogs/splitDialog.test.ts` | `split.spec.ts` |
| B7 category order | — | `map/categoryOrder.test.ts` | `category-order.spec.ts` |
| C3.0 polish | `layout.rs`, `basemap_cache.rs` | `layout/arrange.test.ts`, `rulers.test.ts` | `layout-polish.spec.ts` |
| C3.1 templates | `templates.rs` | `layout/types.test.ts` | `templates.spec.ts` |
| C3.2 series | `series.rs` | `layout/series.test.ts` | `series.spec.ts` |
| R1 reference | `crs_reference.rs` | — | — |
| R2 identity/importers | `crs_identity.rs`, `crs_import.rs` | — | — |
| R3 export / datum | `crs_export.rs`, `crs_edge.rs` | — | `map.spec.ts` (M-4) |
| R4 tool matrix | `crs_tools.rs`, `crs_matrix.rs` | — | — |
| R5 rasters/poles | `crs_raster.rs`, `display.rs` | — | — |
| D0 tiled DEM | `dem_tiles.rs` (mock service: tiling, seams, failures, cancel, resume, confirm, metadata) | — | `dem.spec.ts` |
| D1 ArcGIS catalog | `arcgis.rs` (mock MapServer, catalog integrity) | — | `arcgis.spec.ts` |
| D2 Census | `census.rs` (mock Census API, key redaction) | `census/presets.test.ts` | `census.spec.ts` |
| D3 STAC | `stac.rs` (mock STAC + Range-capable COG server) | — | `stac.spec.ts` |
| D4 Overture | `overture.rs` (GeoParquet over HTTP, httpfs) | — | `overture.spec.ts` |
| D5 OGC API | `ogcapi.rs` (mock service with paging) | — | `ogc.spec.ts` |
| D6 quakes + soils | `jsonapis.rs` (mock services) | — | `hazards.spec.ts` |
| R8 world view | `world_view.rs` (grid, cut, densify, reference lines, outline) | `crs/projectionBuilder.test.ts` | `world.spec.ts` |
| R6 CRS UI | `crs_identity.rs` (`crs_info_*`, `map_factors_*`), `crs_ui.rs` (Tissot, statement, same_crs) | `statusbar/statusBar.test.ts`, `dialogs/crsRecents.test.ts` | `crs-ui.spec.ts` |

Gaps found by this table are closed in the blocks above (each new feature adds its row here) and listed in `docs/TEST_PLAN.md` as "planned" until automated.
