# GIS Studio roadmap

The single authoritative roadmap: feature inventory, platform matrix and priorities.
Status marks: ✅ implemented + tested · 🟡 implemented, thin tests · 🟠 partial · ⛔ broken/blocked · ⬜ planned.
Last audit: 2026-10-23 (code inspection + test runs on macOS arm64).

## 1. Feature inventory

### Data and layers
| Feature | Status | Notes |
|---|---|---|
| Import shp/gpkg/geojson/kml/csv/parquet; rasters (GeoTIFF etc.) | ✅ | engine tests |
| CRS: explicit, 7,195-entry picker, all State Plane, Define Projection | ✅ | catalog test is network/ignored |
| Map view draws in the map's CRS (OpenLayers; engine tiles per CRS grid; true 1:N scale; basemap datum-offset notice) | 🟡 | M1 + M2 done 2026-10-23 (`docs/MAP_PROJECTION_PLAN.md`); M3 exact identify/select/measure/draw in any CRS in progress; M4 rasters in any CRS ⬜ (rasters draw in Web Mercator only until then) |
| Attribute table: spreadsheet editing, add/rename/delete field, calculate field/geometry | ✅ | |
| Definition queries (GUI + Python→SQL), selection, export selection | ✅ | |
| Projects (.gproj = project.json + project.duckdb) | 🟡 | rasters live in an app-wide store outside the project (not portable); local file sources are absolute paths |
| TIGER / National Map vector / 3DEP downloads | ✅ | TIGER by year (2010–current, sorted); network tests ignored by default |
| File manager (Catalog pane: browse folders/GIS files, drag to map or model, folder connections) | ✅ | GIS-aware listing (shapefile sidecars hidden, .gdb/.gproj as items), GDAL details, drag to map/Contents/node editor |
| Public data toolkit (Living Atlas–style) | ⬜ | plan and dataset list in `docs/PUBLIC_DATA_CATALOG.md` |
| Sources & citations (annotated bibliography, BibTeX) | ✅ | model results cite every upstream input |
| Census data join (ACS / 2020) + keychain key | ⬜ | |
| NAIP imagery; NASA GIBS / CMR / POWER | ⬜ | |

### Node editor (Model)
| Requirement | Status | Notes |
|---|---|---|
| One tree per layer, switch with selection, saved in project.json | ✅ | trees keyed by layer id (rename-safe) |
| Fixed Layer Input / Output (not deletable) | ✅ | UI-enforced only; engine doesn't validate |
| Output draws result in place of layer; original untouched; Save as new layer | ✅ | failed run keeps last result but nothing marks it stale ⬜ |
| Extra Layer input nodes (menu, drag from Contents, drop on node) | ✅ | |
| Shift+A menu from the registry, search, keyboard, typing guard | ✅ | no aliases ⬜ |
| Drop node onto a connection (splice) | ✅ | one undo step; types checked on both sides; misfits refused with a reason |
| Blender shortcuts (node editor only) + rebinding dialog | ✅ | Shift+A, X, Ctrl+X, Shift+D, Ctrl+C/V, G, A/Alt+A/Ctrl+I, M, H, F, Home/., Ctrl+Z/Shift+Z, Ctrl+Enter/F12, Ctrl+right-drag cut |
| Undo/redo | 🟡 | per tree (clears when switching trees); each param keystroke is an undo step |
| Deleted layer handling | 🟠 | engine reports "Layer no longer exists"; other trees still reference it silently; deleted layer's tree kept until next switch |
| Validation vs execution status | ✅ | port checks at connect/insert/run; changed-since-last-run marker; Output flags previous/outdated results |
| Execution: topo order, SHA-256 session cache, skip downstream of failures | ✅ | cache key has no tool version (cache is per session, so acceptable) |
| Progress for model runs | ✅ | step n of N |
| Cancel a model run | ✅ | DuckDB interrupt + step checks; atomic result writes; slow for per-feature geodesic tools; GDAL raster steps finish first |
| Copy/paste, duplicate, mute, collapse, make links | ✅ | auto-layout ⬜ |
| Attribute history between nodes ("git for tables") | ✅ | B2.1 (persisted node cache), B2.2 (engine diff + node-rows commands, `history.rs`) and B2.3 (table pane UI: node picker, diff bar, node-card menu) done 2026-10-23. Designed 2026-10-23 in `docs/ATTRIBUTE_HISTORY_PLAN.md` (build B2.1–B2.3). Requested 2026-10-23: keep each node's output table as a versioned snapshot (what changed vs. the previous node: rows added/removed, fields added/changed); the Attribute Table shows the Output by default with a picker to view any earlier node's table and its diff. Builds on the node cache tables (`__node_<hash>`), which currently live only for a session. |
| Model tests (graph semantics, persistence) | 🟠 | engine tests cover tools; no tests for UI graph ops (splice, undo, tree switching) |

### Processing tools (97 spec ids across `nodes/`, 48 tools)
| Group | Status |
|---|---|
| Vector: buffer (geodesic/planar, field distance), clip, intersect, dissolve, project, spatial/attribute join, join by nearest, near, select by location, centroids, Voronoi, grids/hex, points along lines, extract vertices, summarize within, definition query, export | ✅ / 🟡 |
| Vector already present: merge, repair, explode (multipart→singlepart), simplify, bounding geometry (hull/envelope), feature to point, select by location | ✅ |
| Union, symmetrical difference, singlepart→multipart (Collect) | ✅ |
| Concave hull (Minimum Bounding Geometry), Delaunay triangulation | ✅ |
| Raster: contours, hillshade, slope, aspect, ruggedness, clip, warp, polygonize, sample, zonal stats | 🟡 |
| Raster missing: calculator, reclassify, resample/align, mosaic, rasterize, NoData tools, band select | ⬜ |
| Hydrology, interpolation, spatial statistics, remote sensing | ⬜ |
| Measure, MGRS, resection/intersection | ✅ |

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
P2 node-tree correctness — done: cancellation, one-step type-checked inserts, stale markers, provenance from all inputs, Blender shortcuts. Deleted-layer references flagged and one undo step per parameter edit also done. Remaining: **attribute history between nodes**, UI graph tests.
P3 processing correctness — parameter validation; missing vector tools (union, difference, repair, multipart↔singlepart, simplify, hulls, Delaunay).
P4 raster foundation — calculator, alignment, NoData, resample, mosaic.
P5 data — done: file manager (Catalog), TIGER by year. Remaining: public data toolkit connectors (ArcGIS REST catalog → STAC/COG → GeoParquet; docs/PUBLIC_DATA_CATALOG.md), Census join, NAIP, NASA.
P6 layouts (designed: `docs/LAYOUTS_PLAN.md`; C2.1–C2.4 ✅ — page, frames, legend, scale bar, north arrow, sources, graticule/grid, rasters, USGS basemaps; C3.1–C3.2 templates and map series ⬜), then the Topo Map Builder.
P7 Python tool SDK + headless CLI.
P8 advanced analysis (hydrology, interpolation, statistics, remote sensing).
P9 planetary GIS (Moon, Mars first): body-aware CRS/geodesics, USGS data importers, crater tools, ISIS/ASP bridges — plan in `docs/PLANETARY_PLAN.md` (not started).
