# Open-source software, data and sources used or referenced

Hand-kept register (started 2026-10-29). `THIRD_PARTY_NOTICES.md` is the generated licence list of everything compiled into or shipped with a build (it must be regenerated from a CI build, see `docs/DECISIONS.md` D2); this file says **why** each thing is here and **how** it is used or referenced.
GIS Studio is MIT OR Apache-2.0. Only permissive code (MIT, Apache-2.0, BSD, ISC, public domain; LGPL libraries only as shared libraries that can be replaced) is linked or bundled. GPL code is never bundled, linked or copied.

**Standing rule (user, 2026-10-29):** every change that adds, removes or newly references open-source software, data or a published method updates this file in the same commit, and `THIRD_PARTY_NOTICES.md` when the shipped set changes (`scripts/third_party_notices.py`, plus its static list for non-crate items). Algorithms taken from a paper, a standard or another program are listed under "Referenced methods" with the citation.

## 1. Libraries linked into the app

| Name | Licence | Role | Where |
|---|---|---|---|
| GDAL (+ `gdal`, `gdal-sys` crates) | MIT | vector/raster I/O, warp, translate, grid (IDW/TIN), rasterize, contours, fill-nodata, overviews | `raster.rs`, `raster_ops.rs`, `layers.rs` |
| PROJ (inside GDAL and DuckDB) | MIT | coordinate reference systems and datum shifts | `crs.rs`, `db.rs` |
| DuckDB (`duckdb` crate) + Spatial and httpfs extensions | MIT | storage, SQL, spatial functions, GeoParquet over HTTP | `db.rs`, `layers.rs`, `overture.rs` |
| `geographiclib-rs` | MIT | geodesic distance/area on an ellipsoid | `measure.rs` |
| `fasteval` | MIT | raster calculator expressions | `raster_ops.rs` |
| `resvg`, `svg2pdf`, `pdf-writer`, `png`, `ttf-parser` | Apache-2.0 OR MIT | print layouts: SVG, PDF and PNG export, fonts | `layout/` |
| `ureq` | MIT OR Apache-2.0 | HTTP downloads | `tnm*.rs`, `census.rs`, `stac.rs`, … |
| `keyring` | MIT OR Apache-2.0 | OS keychain for the Census API key | `src-tauri/src/secrets.rs` |
| `serde`, `serde_json`, `anyhow`, `base64`, `hex`, `sha2`, `flate2`, `lru`, `parking_lot`, `percent-encoding`, `tempfile`, `tiny_http` | MIT OR Apache-2.0 (`lru`: MIT) | plumbing, caches, dev server, tests | engine, devserver, shell |
| Tauri 2 (`tauri`, `tauri-plugin-dialog`, `tauri-build`) | Apache-2.0 OR MIT | desktop shell | `src-tauri/` |
| React 19, React DOM | MIT | user interface | `ui/src` |
| Fluent UI React v9 (`@fluentui/react-components`, `react-icons`) | MIT | components and icons | `ui/src` |
| `@xyflow/react` | MIT | node editor canvas | `ui/src/nodes` |
| `dockview-react` | MIT | dockable panes | `ui/src` |
| OpenLayers (`ol`), `ol-mapbox-style` | BSD-2-Clause | map display | `ui/src/map` |
| `proj4` | MIT | client-side projections | `ui/src/map` |
| `geographiclib-geodesic` | MIT | geodesic measurements in the UI | `ui/src` |
| `geomagnetism` | Apache-2.0 | magnetic declination (World Magnetic Model) | layouts (north arrow) |
| `mgrs` | MIT | MGRS coordinates | status bar |
| `fontdb` (a dependency of `resvg`/`usvg`, used directly) | MIT | lists the fonts installed on the computer (macOS, Windows, Linux system font folders) and copies the families a layout names into its render database | `fonts.rs`, `layout/export.rs` |
| `zustand`, `zundo`, `@tanstack/react-virtual` | MIT | state, undo, long lists | `ui/src` |

Build and test tooling (not shipped): Vite (MIT), Vitest (MIT), TypeScript (Apache-2.0), `@vitejs/plugin-react` (MIT), Playwright (Apache-2.0), `@tauri-apps/cli` (Apache-2.0 OR MIT), `@types/*` (MIT).

## 2. Programs shipped or run

| Name | Licence | Role |
|---|---|---|
| WhiteboxTools v2.4.0 (John Lindsay), built from commit `28ed01f` by `scripts/build-whitebox.sh`, shipped as a separate program in `resources/whitebox/` | MIT | hydrology nodes (fill depressions, D8 flow direction and accumulation, basins, streams, watersheds): `whitebox.rs`, `nodes/hydrology_tools.rs` |
| GDAL, PROJ and their dependent libraries bundled in the installers (conda-forge GDAL 3.12 on Windows/Linux; Homebrew on the local Mac build) | MIT, BSD, LGPL (SpatiaLite `lgpl` build) | see `THIRD_PARTY_NOTICES.md`; `scripts/license-gate.sh` fails a build that bundles GPL-only libraries (poppler, x265, librttopo). The local Homebrew Mac build does include some; release installers do not |
| `cs2cs` / `projinfo` (PROJ) | MIT | reference answers for the datum tests (`crates/engine/tests/fixtures`), not shipped |

## 3. Data bundled in the app

| Data | Licence / terms | Where |
|---|---|---|
| Natural Earth 110m land | Public domain | `crates/engine/assets/ne_110m_land.geojson`, world outline layer |
| NOAA NADCON and state HPGN datum grids (58 files, GeoTIFF via PROJ-data) | Public domain | `crates/engine/assets/proj-grids/` (README there) |
| EPSG dataset inside `proj.db`; extra CRS definitions | EPSG terms of use, © IOGP; PROJ | `crates/engine/assets/extra_crs.json` |
| World Magnetic Model coefficients | Public domain (NOAA NCEI / BGS) | through `geomagnetism` |
| Noto Sans Regular, Bold, Italic | SIL OFL 1.1 | `crates/engine/assets/fonts/` |
| Catalogs of public services (ArcGIS, STAC, OGC API) | facts about public endpoints | `crates/engine/assets/*_catalog.json` |

## 4. Public data the user can download (recorded in Sources on import)

U.S. Census TIGER/Line, ACS and 2020 Census (public domain, API key by the user); USGS 3D Elevation Program and The National Map (public domain); USGS earthquake catalog and Water Data (public domain); NRCS Soil Data Access / SSURGO (public domain); PAD-US, FEMA National Flood Hazard Layer, National Wetlands Inventory, wildfire perimeters (U.S. Government data, ArcGIS REST services); Sentinel-2 L2A and Copernicus DEM through a public STAC (open licences, attribution required); Overture Maps GeoParquet (licence varies by theme, ODbL or CDLA-Permissive; the release is pinned in `overture.rs`); OSM-based basemap tiles (ODbL, attribution on the map). Each import writes its own citation (`sources.rs`).

## 5. Referenced methods (written from the description; no third-party code)

| Method | Reference | Where |
|---|---|---|
| Moran's I, randomisation variance | Moran 1950; Cliff & Ord 1981 | `spatial_stats.rs` |
| Getis-Ord Gi* | Ord & Getis 1995 | `spatial_stats.rs` |
| Ordinary kriging and variogram models | Matheron 1963; Isaaks & Srivastava 1989 | `kriging.rs` |
| K-means with k-means++ seeding | Lloyd 1982; Arthur & Vassilvitskii 2007 | `remote_sensing.rs` |
| NDVI, NDWI, MNDWI, NBR, NDBI, SAVI | Rouse et al. 1974; McFeeters 1996; Xu 2006; Key & Benson 2006; Zha et al. 2003; Huete 1988 | `remote_sensing.rs` |
| IDW, linear (TIN), nearest, moving average | GDAL grid algorithms (MIT); QGIS `qgis:idwinterpolation`, `qgis:tininterpolation` named for familiarity | `raster_ops.rs` |
| Geodesics | Karney 2013, doi:10.1007/s00190-012-0578-z | `measure.rs`, planetary plan |
| Tissot indicatrix, map-projection scale factors | Snyder 1987, Map Projections: A Working Manual | `tissot.rs`, `crs.rs` |
| Planetary: crater digitising, size-frequency, production functions; geologic symbols | Kneissl et al. 2011 (CraterTools), Crater Analysis Techniques Working Group 1979, Neukum et al. 2001, FGDC-STD-013-2006 | `docs/PLANETARY_PLAN.md` (design only) |
| Label settings of the QGIS Labels panel (Placement: around point / offset / cartographic quadrants, above/on/below line, Priority, Allow overlaps, Wrap on character, Buffer, Background, Shadow, data-defined rotation/size/colour) | QGIS user manual, "Labels"; GPL program, setting names and behaviour only, reimplemented | `layout/labels.rs`, `ui/src/map/labels.ts`, `docs/LABELS_PLAN.md` |
| Contour label placement along lines (arc-length walk, near-straight chord test, upright text, index contours) | own algorithm; "index contour" convention from topographic mapping practice (USGS topographic map symbols) | `layout/labels.rs` (`along_line`), `ui/src/map/labels.ts` |
| Export Table coordinate columns, reprojection with explicit CRS and a sidecar note | PROJ transformation via DuckDB Spatial `ST_Transform` (always x/y); no external method | `export_table.rs` |
| QGIS Processing algorithm ids (about 30: `native:splitvectorlayer`, `native:union`, `gdal:slope`, …) | QGIS documentation, named in each tool's header comment so users can find the equivalent | `nodes/*.rs` |
| ArcGIS Pro tool names (Spatial Autocorrelation, Hot Spot Analysis, Fill, Flow Direction, …) | proprietary software: names only, so users can find the equivalent; no code or documentation text | node descriptions |

## 6. Considered and not used

| Name | Licence | Why not |
|---|---|---|
| QGIS, GRASS GIS, SAGA GIS, RichDEM | GPL | bundling or copying would make the installers GPL; algorithms are reimplemented from their published descriptions |
| Orfeo ToolBox | Apache-2.0 | candidate for heavy remote sensing (supervised classification, segmentation); too large for now |
| PDAL | BSD | candidate for LiDAR (LAS/LAZ to DEM); not needed yet |
| `smartcore`, `linfa` | Apache-2.0 / MIT | candidates for supervised classification (waits for the training-area design) |
| the `kriging` crate (0.0.1) | MIT | too immature; kriging written directly |

## 7. Keeping this current

For every new or changed dependency, bundled file, downloaded source, algorithm or cited method: add or edit a row here (name, licence, role, where), update `THIRD_PARTY_NOTICES.md` if it ships (`python3 scripts/third_party_notices.py` after dependency changes, from a CI build), and cite the source in the tool's header comment. Check the licence on the project's own page at the time you adopt it; a GPL-only source is a reference for ideas, never a dependency.
