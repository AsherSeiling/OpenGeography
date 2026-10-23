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
| Attribute table: spreadsheet editing, add/rename/delete field, calculate field/geometry | ✅ | |
| Definition queries (GUI + Python→SQL), selection, export selection | ✅ | |
| Projects (.gproj = project.json + project.duckdb) | 🟡 | rasters live in an app-wide store outside the project (not portable); local file sources are absolute paths |
| TIGER / National Map vector / 3DEP downloads | ✅ | network tests ignored by default |
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
| Model tests (graph semantics, persistence) | 🟠 | engine tests cover tools; no tests for UI graph ops (splice, undo, tree switching) |

### Processing tools (97 spec ids across `nodes/`, 48 tools)
| Group | Status |
|---|---|
| Vector: buffer (geodesic/planar, field distance), clip, intersect, dissolve, project, spatial/attribute join, join by nearest, near, select by location, centroids, Voronoi, grids/hex, points along lines, extract vertices, summarize within, definition query, export | ✅ / 🟡 |
| Vector still missing: union, difference, symmetrical difference, merge/append, repair geometry, multipart↔singlepart, simplify, convex/concave hull, Delaunay | ⬜ |
| Raster: contours, hillshade, slope, aspect, ruggedness, clip, warp, polygonize, sample, zonal stats | 🟡 |
| Raster missing: calculator, reclassify, resample/align, mosaic, rasterize, NoData tools, band select | ⬜ |
| Hydrology, interpolation, spatial statistics, remote sensing | ⬜ |
| Measure, MGRS, resection/intersection | ✅ |

## 2. Platform matrix
Status levels: implemented → build-verified → tested → release-ready.

| Area | macOS arm64 | macOS Intel | Windows x64 | Linux x64 |
|---|---|---|---|---|
| Dev build + tests | tested | not attempted | not attempted | not attempted |
| Release package | tested (DMG, self-test in clean env) | ⬜ | CI running, not green yet | CI running, not green yet |
| Native deps strategy | Homebrew + dylibbundler | ⬜ | conda-forge GDAL 3.10 + prebuilt libduckdb (unrun) | Ubuntu GDAL + AppImage (unrun) |
| Map, vector, raster, projects | tested | — | — | — |
| Clean-machine smoke test | `--self-test` with `env -i` | — | — | — |

Known portability risks:
1. CI runs at https://github.com/AsherSeiling/OpenGeography/actions; history in `docs/CI_LOG.md`.
2. `.cargo/config.toml` hard-codes `/opt/homebrew` (CI deletes it; local Windows/Linux devs must too).
3. `resources::init()` keeps any pre-existing `GDAL_DATA`/`PROJ_DATA`, so another GDAL install can override bundled data.
4. Rasters are stored in a per-user app folder, not in the project → projects with rasters don't move between machines.
5. GDAL versions differ per platform (3.13 Homebrew, 3.10 conda, Ubuntu 3.8).

## 3. Licensing
The macOS package includes GPL-only libraries via Homebrew GDAL: poppler (linked by libgdal: PDF driver), x265 (via libheif: HEIF driver) and librttopo (via libspatialite: SQLite/SpatiaLite driver). Fix: build/obtain GDAL without PDF/HEIF/SpatiaLite (or with those drivers as removable plugins), re-run `third_party_notices.py`, confirm zero GPL-only entries. Apply to Windows/Linux packages too.

## 4. Priorities
P0 baseline (done 2026-10-23: all tests green on macOS) →
P1 release reliability: push to a remote so CI runs; fix Windows/Linux builds; GPL-free GDAL; bundled-data precedence; project-portable rasters →
P2 node-tree correctness: ~~model-run cancellation~~ (done 2026-10-23); splice as one undo step + type check; stale-result marker; deleted-layer references; provenance from all inputs; graph tests →
P3 processing correctness: parameter validation, missing vector tools (union/difference/repair/…) →
P4 raster foundation: calculator, alignment, NoData, resample, mosaic →
P5 data: Census join, NAIP, NASA →
P6 layouts, then Topo Map Builder →
P7 Python tool SDK + headless CLI →
P8 advanced analysis (hydrology, interpolation, statistics, remote sensing).
