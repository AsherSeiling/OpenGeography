# GIS Studio roadmap

The single authoritative roadmap: feature inventory, platform matrix and priorities.
Status marks: ✅ implemented + tested · 🟡 implemented, thin tests · 🟠 partial · ⛔ broken/blocked · ⬜ planned.
Last audit: 2026-10-23 (code inspection + test runs on macOS arm64); status marks refreshed 2026-11-04 from `PROJECT_STATUS.md`, the code and git history, without a new full audit. Test cases per feature: `docs/TEST_PLAN.md`; open bugs: the Open bugs section of `docs/BUGS.md`.

## 1. Feature inventory

### Data and layers
| Feature | Status | Notes |
|---|---|---|
| Import shp/gpkg/geojson/kml/csv/parquet; rasters (GeoTIFF etc.) | ✅ | engine tests |
| CRS: explicit, 7,195-entry picker, all State Plane, Define Projection | ✅ | catalog test is network/ignored |
| CRS hardening: reference-checked transforms, identity by meaning, datum accuracy reporting, tool matrix over the CRS zoo, rasters, map, layouts, world view | 🟠 | Phase R built except R7 (all three platforms in CI, thread stress, performance baselines); `docs/CRS_PLAN.md`; cases in `docs/TEST_PLAN.md` §2A |
| Map view draws in the map's CRS (OpenLayers; engine tiles per CRS grid; true 1:N scale; basemap datum-offset notice; rasters in every CRS since 2026-10-30) | 🟠 | M1–M3 done (`docs/MAP_PROJECTION_PLAN.md`). Left from M4: the 3M-point performance trace and engine-drawn basemaps |
| Multiple maps: each map has its own layers, CRS, basemap and view; side-by-side panels; a layout frame can follow a map | ✅ | WP10–WP11, project format 2, old projects migrate, newer ones are refused; not on `main` yet. Open: MM-4 |
| Open basemaps: OpenFreeMap (Liberty, Bright, Positron), VersaTiles, OpenTopoMap, Natural Earth, plus USGS and OpenStreetMap | ✅ | WP12; not on `main` yet. Vector basemaps draw in Web Mercator only; only USGS and Natural Earth print in layouts; a refusal is explained in words. NASA Blue Marble left out (no licence marking on the layer) |
| Attribute table: spreadsheet editing, add/rename/delete field, calculate field/geometry | ✅ | |
| Definition queries (GUI + Python→SQL), selection, export selection | ✅ | |
| Projects (.gproj = project.json + project.duckdb) | 🟡 | full save/open round trip tested (`tests/project.rs`); rasters live in an app-wide store outside the project (not portable); local file sources are absolute paths |
| TIGER / National Map vector / 3DEP downloads | ✅ | TIGER by year (2010–current, sorted); network tests ignored by default |
| Catalog pane: one tree of Project (Maps, Layouts, Project files) and Public Data | ✅ | Redesigned in WP4: the folder browser and folder connections were removed, so files are added with Add Data |
| Public data toolkit (Living Atlas–style) | ✅ | Phase D complete: tiled 3DEP, ArcGIS catalog, STAC/COG, Overture GeoParquet, OGC API, earthquakes, SSURGO; `docs/PUBLIC_DATA_CATALOG.md` |
| Sources & citations (annotated bibliography, BibTeX) | ✅ | model results cite every upstream input |
| Census data join (ACS / 2020) + keychain key | 🟡 | D2 built; keychain not yet tried on a real OS (M-31) |
| Imagery: Sentinel-2, Copernicus DEM; NAIP; NASA POWER | 🟠 | Sentinel-2 and Copernicus DEM offered through STAC. Landsat and NAIP on AWS need an AWS account, so NAIP needs a later connector. NASA POWER not built, on purpose (`docs/DECISIONS.md`) |
| Server connections: live WMS, WMTS and ArcGIS REST layers, browse a server, print, save a copy | ⬜ | Planned before planetary (user, 2026-11-04); `docs/SERVER_CONNECTIONS_PLAN.md`. ArcGIS vector download already built. |

### Node editor (Model)
| Requirement | Status | Notes |
|---|---|---|
| One tree per layer, switch with selection, saved in project.json | ✅ | trees keyed by layer id (rename-safe) |
| Fixed Layer Input / Output (not deletable) | ✅ | UI-enforced only; engine doesn't validate |
| Output draws result in place of layer; original untouched; Save as new layer | ✅ | failed run keeps last result but nothing marks it stale ⬜ |
| Extra Layer input nodes (menu, drag from Contents, drop on node) | ✅ | |
| Shift+A menu from the registry, search, keyboard, typing guard | ✅ | searches names, keywords and categories from `tool_specs` (WP2, WP3); the main way to reach tools |
| Drop node onto a connection (splice) | ✅ | one undo step; types checked on both sides; misfits refused with a reason |
| Blender shortcuts (node editor only) + rebinding dialog | ✅ | Shift+A, X, Ctrl+X, Shift+D, Ctrl+C/V, G, A/Alt+A/Ctrl+I, M, H, F, Home/., Ctrl+Z/Shift+Z, Ctrl+Enter/F12, Ctrl+right-drag cut |
| Undo/redo | 🟡 | per tree (clears when switching trees); each param keystroke is an undo step |
| Deleted layer handling | 🟠 | engine reports "Layer no longer exists"; other trees still reference it silently; deleted layer's tree kept until next switch |
| Validation vs execution status | ✅ | port checks at connect/insert/run; changed-since-last-run marker; Output flags previous/outdated results |
| Execution: topo order, SHA-256 session cache, skip downstream of failures | ✅ | cache key has no tool version (cache is per session, so acceptable) |
| Progress for model runs | ✅ | step n of N |
| Cancel a model run | ✅ | DuckDB interrupt + step checks; atomic result writes; GDAL raster steps (incl. gdaldem and overviews) stop through the progress hook (`tests/raster_cancel.rs`) |
| Copy/paste, duplicate, mute, collapse, make links | ✅ | auto-layout ⬜ |
| Attribute history between nodes ("git for tables") | ✅ | B2.1 (persisted node cache), B2.2 (engine diff + node-rows commands, `history.rs`) and B2.3 (table pane UI: node picker, diff bar, node-card menu) done 2026-10-23. Designed 2026-10-23 in `docs/ATTRIBUTE_HISTORY_PLAN.md` (build B2.1–B2.3). Requested 2026-10-23: keep each node's output table as a versioned snapshot (what changed vs. the previous node: rows added/removed, fields added/changed); the Attribute Table shows the Output by default with a picker to view any earlier node's table and its diff. Builds on the node cache tables (`__node_<hash>`), which currently live only for a session. |
| Model tests (graph semantics, persistence) | 🟡 | engine tests cover every tool (`tests/tools.rs`, `tests/rasters.rs`); UI graph ops covered by `nodeEditor.test.ts` and `modelStore.test.ts`; no component-level tests of the canvas (manual M-14 in the test plan) |

### Maps and layouts
| Feature | Status | Notes |
|---|---|---|
| Print layouts: map frames, legend, scale bar, north arrow, grid, declination, tables, charts, PDF/PNG/SVG export | ✅ | Phase C and LW1–LW6 |
| Labels on the map and in layouts | ✅ | L1–L3, `docs/LABELS_PLAN.md` |
| Templates and map series | ✅ | C3.1, C3.2 |
| Topo Map Builder (land navigation, USGS quadrangle, military installation) | ✅ | LW9, `docs/TOPO_USGS_PLAN.md` |
| Layout editor redesign: Insert rail, start gallery, Properties tabs, right-click menus, Check Page, named text styles | 🟡 | WP13; not on `main` yet. Named text styles aren't saved in the project (LE-1) |
| Layout rework LW7, LW8, LW10–LW12 | 🟠 | No sessions of their own yet; `docs/LAYOUT_REWORK_PLAN.md` |

### Processing tools (97 spec ids across `nodes/`, 48 tools)
| Group | Status |
|---|---|
| Vector: buffer (geodesic/planar, field distance), clip, intersect, dissolve, project, spatial/attribute join, join by nearest, near, select by location, centroids, Voronoi, grids/hex, points along lines, extract vertices, summarize within, definition query, export | ✅ / 🟡 |
| Vector already present: merge, repair, explode (multipart→singlepart), simplify, bounding geometry (hull/envelope), feature to point, select by location | ✅ |
| Union, symmetrical difference, singlepart→multipart (Collect) | ✅ |
| Concave hull (Minimum Bounding Geometry), Delaunay triangulation | ✅ |
| Raster: contours, hillshade, slope, aspect, ruggedness, clip, warp, polygonize, sample, zonal stats | 🟡 (tested on analytic grids in `tests/rasters.rs`; open bugs B-T5 Warp crash on invalid CRS, B-T7 Warp of a CRS-less raster, B-T6 — see `docs/BUGS.md`) |
| Raster foundation: calculator, reclassify, resample, align, mosaic, rasterize, NoData tools, band select | ✅ (2026-10-29, `tests/raster_ops.rs`, `raster_nodes.rs`, `raster_cancel.rs`) |
| Hydrology, interpolation, spatial statistics, remote sensing | ✅ H1 interpolation, H2 kriging, H3 Moran's I + hot spots, H4 hydrology (WhiteboxTools sidecar), H5 spectral indices + K-means classification done 2026-10-29 (Phase H complete; supervised classification not built) |
| Measure, MGRS, resection/intersection | ✅ |

### UI redesign (`docs/UI_REDESIGN_PLAN.md`, 2026-11-03 to 2026-11-04)
| Feature | Status | Notes |
|---|---|---|
| Tokens, light/dark, custom colours (AA by construction) | ✅ | WP1, WP8b; `appearance.test.ts`; AA measured on the running app in both themes (`e2e/redesign-a11y.spec.ts`) |
| Shift+A tool menu, keywords, search, project box, ribbon, default layout, conditional bottom panel | ✅ | WP2, WP3, WP6; browser only |
| Catalog tree, node cards, panes, dialogs, error pop-up | ✅ | WP4, WP5, WP7, WP8 |
| Multiple maps (project format 2), open basemaps | ✅ | WP10 to WP12; live basemap checks are ignored network tests |
| Layout editor (option B: rail, start gallery, Properties tabs, pre-flight), layout workspace closes the bottom panel and gives Items a column | ✅ | WP13, WP9 |
| Control audit (every ribbon tab, menu, pane and right-click menu) | ✅ | `e2e/redesign.spec.ts`, WP9: 827 controls clicked, none dead; 22 stop at a native file dialog (answered Cancel); see `docs/UI_REDESIGN_VERIFY.md` |
| Packaged app, Windows, Linux | ⬜ | not run on the redesign; Inter and the layout are portable but unverified there |

## 2. Platform matrix
Status levels: implemented → build-verified → tested → release-ready.

| Area | macOS arm64 | macOS Intel | Windows x64 | Linux x64 |
|---|---|---|---|---|
| Dev build + tests | tested | not attempted | tested in CI (engine + UI) | tested in CI (engine + UI) |
| Release package | tested (DMG, self-test in clean env) | ⬜ | build-verified in CI (NSIS + MSI, self-test passes) | build-verified in CI (AppImage + deb, self-test passes) |
| Native deps strategy | Homebrew + dylibbundler | ⬜ | conda-forge GDAL 3.12 + prebuilt libduckdb (CI, GPL-free gate) | conda-forge GDAL 3.12, private in the AppImage with unique sonames (CI, GPL-free gate) |
| Map, vector, raster, projects | tested | — | — | — |
| Clean-machine smoke test | `--self-test` with `env -i` | — | — | — |

Known portability risks:
1. CI runs at https://github.com/AsherSeiling/OpenGeography/actions; history in `docs/CI_LOG.md`.
2. `.cargo/config.toml` hard-codes `/opt/homebrew` (CI deletes it; local Windows/Linux devs must too).
3. Fixed 2026-10-23: bundled GDAL/PROJ data always wins over `GDAL_DATA`/`PROJ_DATA`; rasters are copied into the project, so projects move between machines.
4. GDAL is 3.12 on every platform (conda-forge in CI, Homebrew 3.12.x locally; the gdal crate 0.19 doesn't support 3.13). Local Homebrew builds still link GPL-only libraries; CI release builds don't.

## 3. Licensing
Update 2026-10-23: macOS and Windows CI builds use conda-forge `libgdal-core=3.12` + `libspatialite=*=lgpl*` (no poppler/x265/librttopo) and a license gate fails the build if any appears. Linux moved to conda-forge GDAL 3.12 too (private `gs_`-prefixed libs, strict gate) — done 2026-10-23.

Original finding:
The macOS package includes GPL-only libraries via Homebrew GDAL: poppler (linked by libgdal: PDF driver), x265 (via libheif: HEIF driver) and librttopo (via libspatialite: SQLite/SpatiaLite driver). Fix: build/obtain GDAL without PDF/HEIF/SpatiaLite (or with those drivers as removable plugins), re-run `third_party_notices.py`, confirm zero GPL-only entries. Apply to Windows/Linux packages too.

## 4. Priorities
The order these are done in, with effort and dependencies, is in `docs/MASTER_PLAN.md`.
P0 baseline — done 2026-10-23 (all tests green on macOS).
P1 release reliability — CI green on macOS, Linux and Windows (done 2026-10-23). GPL-free GDAL, bundled-data precedence and project-portable rasters done. Remaining: hands-on Windows/Linux checks, release workflow, macOS notarization and Intel build.
P2 node-tree correctness — done: cancellation, one-step type-checked inserts, stale markers, provenance from all inputs, Blender shortcuts, deleted-layer references flagged, one undo step per parameter edit, attribute history between nodes.
P3 processing correctness — done: parameter validation; union, difference, repair, multipart↔singlepart, simplify, hulls, Delaunay.
P4 raster foundation — done 2026-10-29 (calculator, alignment, NoData, resample, mosaic, rasterize, reclassify, band select).
P5 data — done (Phase D): Catalog, TIGER by year, public data connectors, Census join. NAIP needs a later connector.
P6 layouts — built: C2.1–C2.4, C3.1–C3.2, Topo Map Builder (LW9) and the layout editor redesign (WP13). Remaining: LW7, LW8, LW10–LW12.
P7 Python tool SDK + headless CLI — not built; after the first release (user, 2026-11-04).
P8 advanced analysis — done 2026-10-29 (Phase H: hydrology, interpolation, statistics, remote sensing).
P9 planetary GIS — next, and the last feature before the first release (user, 2026-11-04): extraterrestrial map mode, built-in USGS and NASA data, native planetary files, crater tools. Plan in `docs/PLANETARY_PLAN.md`; not started.
