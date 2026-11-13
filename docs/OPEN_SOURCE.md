# Open-source software, data and sources used or referenced

Hand-kept register (started 2026-10-29). `THIRD_PARTY_NOTICES.md` is the generated licence list of everything compiled into or shipped with a build (it must be regenerated from a CI build, see `docs/DECISIONS.md` D2); this file says **why** each thing is here and **how** it is used or referenced.
GIS Studio is MIT OR Apache-2.0. Only permissive code (MIT, Apache-2.0, BSD, ISC, public domain; LGPL libraries only as shared libraries that can be replaced) is linked or bundled. GPL code is never linked or copied into GIS Studio. Since 2026-11-06 (user), unmodified GPL tools may ship as separate programs the app runs, with their licence and source (`docs/DECISIONS.md`, "GPL tools as separate programs").

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
| Inter (`@fontsource-variable/inter`), bundled; UI typeface only (layout and map fonts are separate), imported in `ui/src/main.tsx`, no network request | SIL OFL 1.1 | UI typeface | `docs/UI_REDESIGN_PLAN.md` §2 |
| OpenFreeMap Liberty, Bright, Positron (tiles.openfreemap.org); screen only | Data ODbL, tile schema OpenMapTiles, code MIT | vector basemaps, read live; policy re-read 2026-11-04 | `ui/src/map/basemaps.ts`, `sources.rs` |
| VersaTiles Colorful (style and sprites bundled, tiles from tiles.versatiles.org); screen only | Data ODbL, land cover ESA WorldCover CC BY 4.0, schema and style CC0, style code MIT | vector basemap; policy re-read 2026-11-04 | `ui/public/basemaps/versatiles`, `ui/src/map/basemaps.ts` |
| OpenTopoMap (tile.opentopomap.org); screen only | CC BY-SA 3.0, data ODbL and SRTM | raster basemap; policy re-read 2026-11-04. It asks to be told about use, which is for the project owner to send | `ui/src/map/basemaps.ts`, `sources.rs` |
| Natural Earth 1:110m land, lakes, admin-0 countries | Public domain | bundled basemap, works offline, in every CRS and in layouts | `crates/engine/assets/natural-earth`, `scripts/make-natural-earth.sh`, `layout/basemap.rs` |
| NASA GIBS Blue Marble: referenced, **not offered** | No licence marking found on the layer (GIBS docs, capabilities and metadata read 2026-11-04) | left out under the plan's rule | `docs/UI_REDESIGN_PLAN.md` §2c |
| `@xyflow/react` | MIT | node editor canvas | `ui/src/nodes` |
| `dockview-react` | MIT | dockable panes | `ui/src` |
| OpenLayers (`ol`), `ol-mapbox-style` | BSD-2-Clause | map display | `ui/src/map` |
| `proj4` | MIT | client-side projections | `ui/src/map` |
| `geographiclib-geodesic` | MIT | geodesic measurements in the UI | `ui/src` |
| `world_magnetic_model` (with `uom`, `time`) | MIT OR Apache-2.0 | magnetic declination and its yearly change from NOAA/BGS WMM2025 (also bundles WMM2020); the model itself is public domain | layouts (declination diagram, `layout/declination.rs`) |
| `mgrs` | MIT | MGRS coordinates | status bar |
| `fontdb` (a dependency of `resvg`/`usvg`, used directly) | MIT | lists the fonts installed on the computer (macOS, Windows, Linux system font folders) and copies the families a layout names into its render database | `fonts.rs`, `layout/export.rs` |
| `zustand`, `zundo`, `@tanstack/react-virtual` | MIT | state, undo, long lists | `ui/src` |
| `h3o` | BSD-3-Clause | Uber H3 hexagonal grid (pure Rust port) for Hex Bins | `nodes/hex_bin.rs` |
| `las` 0.11, `laz` 0.13 (laz-rs, pure Rust) | MIT; Apache-2.0 | Read LAS/LAZ point clouds: header, point counts, bounds, class counts, CRS from the WKT record or GeoTIFF keys (never guessed); unpack LAZ for WhiteboxTools; write the test files | `pointcloud.rs`, `nodes/lidar_tools.rs` |
| `statrs` 0.19.1 | MIT | Probability distributions for the statistics tools: Student's t, F, chi-square, normal, binomial (CDFs and quantiles); used in `stats/` | `stats/describe.rs`, `stats/inference.rs` |
| `spade` | MIT OR Apache-2.0 | Delaunay and constrained Delaunay triangulation, natural neighbour interpolation | `nodes/tin_interpolation.rs` |
| `smartcore` | Apache-2.0 | Random Forest and k-nearest-neighbour classifiers (pure Rust, no BLAS) | `nodes/classify_supervised.rs` |

Build and test tooling (not shipped): Vite (MIT), Vitest (MIT), TypeScript (Apache-2.0), `@vitejs/plugin-react` (MIT), Playwright (Apache-2.0), `@tauri-apps/cli` (Apache-2.0 OR MIT), `@types/*` (MIT).

## 2. Programs shipped or run

| Name | Licence | Role |
|---|---|---|
| WhiteboxTools v2.4.0 (John Lindsay), built from commit `28ed01f` by `scripts/build-whitebox.sh`, shipped as a separate program in `resources/whitebox/` | MIT | hydrology nodes (fill depressions, D8 flow direction and accumulation, basins, streams, watersheds): `whitebox.rs`, `nodes/hydrology_tools.rs` |
| SAGA GIS 9.5.1 (`saga_cmd` and tool libraries, SAGA User Group Association / O. Conrad et al.), commit `00530d3`, built unmodified without the GUI by `scripts/build-saga.sh` staged into `resources/saga/` by `scripts/stage-resources.sh`. Statically linked wxWidgets 3.3.4 base (wxWindows Licence); libomp (Apache-2.0 with LLVM exceptions) beside it on macOS, libgomp (GPL-3 with the runtime exception) on Linux | GPL-2.0-or-later (saga_cmd, most tools), GPL-3.0-or-later (some tools), LGPL-2.1-or-later (saga_api) | terrain, grid and vector tools run as a separate program; plan, licence duties and speed check in `docs/SAGA_PLAN.md`; decision "GPL tools as separate programs" |
| CPython 3.13.18 (Python Software Foundation) as built by python-build-standalone release 20261026 (Astral, `astral-sh/python-build-standalone`), fetched and SHA-256 checked by `scripts/stage-resources.sh`, shipped as a separate program in `resources/python/` | PSF License (CPython); python-build-standalone is MPL-2.0 and bundles OpenSSL (Apache-2.0), SQLite (public domain), zlib, libffi, bzip2, xz, ncurses, readline/libedit and other permissive libraries under their own terms | user scripts (`def calculate(row)`) for the field calculator and Definition Query nodes: `pyscript.rs`, `pyscript_worker.py`, `nodes/calculate_field.rs`, `nodes/filter.rs`. Not linked into the app; run in their own process |
| Blockly 13.4.0 (Google, `google/blockly`, pinned exactly in package.json) | Apache-2.0 | the block editor for Python scripts (UR-22): `ui/src/scripts/BlocksEditor.tsx`, `blocks.ts`; loaded only when a block editor opens |
| GDAL, PROJ and their dependent libraries bundled in the installers (conda-forge GDAL 3.12 on Windows/Linux; Homebrew on the local Mac build) | MIT, BSD, LGPL (SpatiaLite `lgpl` build) | see `THIRD_PARTY_NOTICES.md`; `scripts/license-gate.sh` fails a build that bundles GPL-only libraries (poppler, x265, librttopo). The local Homebrew Mac build does include some; release installers do not |
| `cs2cs` / `projinfo` (PROJ) | MIT | reference answers for the datum tests (`crates/engine/tests/fixtures`), not shipped |

## 3. Data bundled in the app

| Data | Licence / terms | Where |
|---|---|---|
| Natural Earth 110m land | Public domain | `crates/engine/assets/ne_110m_land.geojson`, world outline layer |
| NOAA NADCON and state HPGN datum grids (58 files, GeoTIFF via PROJ-data) | Public domain | `crates/engine/assets/proj-grids/` (README there); more grids: see below |
| EPSG dataset inside `proj.db`; extra CRS definitions | EPSG terms of use, © IOGP; PROJ | `crates/engine/assets/extra_crs.json` |
| World Magnetic Model coefficients | Public domain (NOAA NCEI / BGS) | through `geomagnetism` |
| Noto Sans Regular, Bold, Italic | SIL OFL 1.1 | `crates/engine/assets/fonts/` |
| Viridis, magma, inferno, plasma colour maps (matplotlib; cividis: Nuñez et al.) | CC0 (public domain) | `raster_style.rs` ramp library (hex stops typed in) |
| ColorBrewer 2.0 sequential, diverging and qualitative palettes (C. Brewer, M. Harrower) | Apache-2.0 | `raster_style.rs` ramp library (hex stops typed in) |
| Okabe–Ito colour-blind-safe palette (M. Okabe, K. Ito 2002), 7 hex values | published palette, free to use | `layout/tabular.rs` chart colours |
| USGS "Topographic Map Symbols" sheet (U.S. Geological Survey) and the DMA Fort Campbell Military Installation Map (series V741S) | Works of the U.S. Government (public domain); used as the visual reference: symbols are redrawn as plain geometry (strokes, fills, patterns, markers) and the colours are our own sampling of the printed sheet, no image or text is copied | `topo_symbols.rs`, `layout/topo.rs` |
| Catalogs of public services (ArcGIS, STAC, OGC API) | facts about public endpoints | `crates/engine/assets/*_catalog.json` |
| PROJ-data grid catalog snapshot (458 files: name, area, datums, size, SHA-256) | list of facts from `https://cdn.proj.org/files.geojson` (PROJ-data project, grids under their providers' licences) | `crates/engine/assets/proj_grids_catalog.json`; the Datum Grids menu |
| 41 datum-shift grids shipped in the installers (about 46 MB): Canada (NRC NTv2 and provincial, Québec MERN), Australia (ICSM), New Zealand (LINZ), United Kingdom (Ordnance Survey OSTN15), Austria, Belgium, France, Germany (ADV, Hessen), Netherlands, Portugal, Spain, Switzerland | Open Government Licence – Canada; CC BY 4.0 (Québec, Australia ICSM, New Zealand LINZ, Austria, Belgium, Spain, Netherlands, Portugal); CC0 (Switzerland swisstopo); BSD-2-Clause (Ordnance Survey); Licence Ouverte (France IGN); "free redistribution" (Germany ADV); Data licence Germany – attribution 2.0 (Hessen). Each provider's `<source>_README.txt` on cdn.proj.org states its terms and attribution | list and checksums in `crates/engine/assets/bundled_grids.txt`; fetched by `scripts/fetch-bundled-grids.sh` at packaging time, not stored in git |
| Other PROJ-data grids (Brazil CC BY-SA 4.0, Germany Baden-Württemberg, geoids, deformation models, …) | per provider; not shipped | downloaded only when the user clicks in the Datum Grids menu, kept in the user's grid folder |

## 4. Public data the user can download (recorded in Sources on import)

U.S. Census TIGER/Line, ACS and 2020 Census (public domain, API key by the user); USGS 3D Elevation Program and The National Map (public domain); USGS earthquake catalog and Water Data (public domain); NRCS Soil Data Access / SSURGO (public domain); PAD-US, FEMA National Flood Hazard Layer, National Wetlands Inventory, wildfire perimeters (U.S. Government data, ArcGIS REST services); Sentinel-2 L2A and Copernicus DEM through a public STAC (open licences, attribution required); Overture Maps GeoParquet (licence varies by theme, ODbL or CDLA-Permissive; the release is pinned in `overture.rs`); OSM-based basemap tiles (ODbL, attribution on the map). Each import writes its own citation (`sources.rs`).

## 5. Referenced methods (written from the description; no third-party code)

| Method | Reference | Where |
|---|---|---|
| Raster renderers and stretches (Singleband gray, Singleband pseudocolor; ArcGIS Pro Stretch/Classify): names and settings only | QGIS (GPL) and ArcGIS Pro documentation, ideas only; no code or assets | `raster_style.rs` |
| Georeferencing from control points: GCP polynomial transformer (order 1–3, least squares) and thin plate spline transformer, GenImgProj/GCP/TPS warping and suggested output extent. We call GDAL's own `GDALCreateGenImgProjTransformer2` (`METHOD=GCP_POLYNOMIAL` / `GCP_TPS`), `GDALSuggestedWarpOutput` and the warp operation (GDAL, MIT/X); the maths is GDAL's (`gdal_crs.cpp`, `gdal_tps.cpp`); Bookstein 1989 for thin plate splines | GDAL documentation "gdaltransform / GCP transformers"; Bookstein, F. L. 1989, IEEE TPAMI 11(6) | `georef/warp.rs` |
| Vector spatial adjustment: affine and rubber sheet through GDAL's own GCP (order 1) and thin plate spline transformers; similarity (Helmert) as the closed-form least-squares scale, rotation and shift of two point sets (complex-number form) | GDAL transformers as above; Helmert 1880s transformation; Umeyama, S. 1991, IEEE TPAMI 13(4) (similarity from point sets) | `georef/adjust.rs` |
| QGIS `.points` control point file (read only): the layout of the file (mapX, mapY, sourceX, sourceY, enable, residual columns; image row written as a negative number) | QGIS Georeferencer documentation (format only; no QGIS code or assets) | `georef/mod.rs` |
| Moran's I, randomisation variance | Moran 1950; Cliff & Ord 1981 | `spatial_stats.rs` |
| Getis-Ord Gi* | Ord & Getis 1995 | `spatial_stats.rs` |
| Ordinary kriging and variogram models | Matheron 1963; Isaaks & Srivastava 1989 | `kriging.rs` |
| K-means with k-means++ seeding | Lloyd 1982; Arthur & Vassilvitskii 2007 | `remote_sensing.rs` |
| NDVI, NDWI, MNDWI, NBR, NDBI, SAVI | Rouse et al. 1974; McFeeters 1996; Xu 2006; Key & Benson 2006; Zha et al. 2003; Huete 1988 | `remote_sensing.rs` |
| KML, GPX and DXF export; Shapefile field report; clipping on export (`ST_Intersection` / `ST_CollectionExtract` with the union of a polygon layer) | GDAL KML / GPX / DXF drivers inside DuckDB Spatial's GDAL (MIT); DuckDB Spatial (MIT) | `nodes/export.rs` |
| Spreadsheet and table files: XLSX, ODS, DBF, TSV, JSON records, Parquet read and written with GDAL's XLSX / ODS / Shapefile (dBASE) drivers inside DuckDB Spatial's GDAL, and DuckDB's own CSV / JSON / Parquet writers (no new crate; `rust_xlsxwriter` and `calamine` were not needed) | GDAL and DuckDB (MIT) | `nodes/export.rs`, `layers.rs::sources_for_path` |
| Clip raster by a polygon layer (`gdalwarp -cutline`, nearest neighbour on the source grid) | GDAL warp (MIT); QGIS `gdal:cliprasterbymasklayer` named for familiarity | `raster.rs::clip_to_cutline`, `nodes/raster_tools.rs` |
| IDW, linear (TIN), nearest, moving average | GDAL grid algorithms (MIT); QGIS `qgis:idwinterpolation`, `qgis:tininterpolation` named for familiarity | `raster_ops.rs` |
| Geodesics | Karney 2013, doi:10.1007/s00190-012-0578-z | `measure.rs`, planetary plan |
| Tissot indicatrix, map-projection scale factors | Snyder 1987, Map Projections: A Working Manual | `tissot.rs`, `crs.rs` |
| Planetary: crater digitising, size-frequency, production functions; geologic symbols | Kneissl et al. 2011 (CraterTools), Crater Analysis Techniques Working Group 1979, Neukum et al. 2001, FGDC-STD-013-2006 | `docs/PLANETARY_PLAN.md` (design only) |
| Planetary tools under consideration (not shipped or used yet): USGS PGM ArcGIS Toolbox, Planetary ARD Search Tools and OpenCraterTool for ArcGIS Pro (CC0); Craterstats (BSD-3-Clause, planned as a bundled program); Planetcantile and Circle Craters (BSD-3-Clause); geologic-symbols SVGs (CC BY 3.0); FGDC-STD-013 Section 25; GPL projects for method reference only (OpenCraterTool for QGIS, Mappy) | Licences read from each project 2026-11-04 | `docs/PLANETARY_PLAN.md` §13 |
| Label settings of the QGIS Labels panel (Placement: around point / offset / cartographic quadrants, above/on/below line, Priority, Allow overlaps, Wrap on character, Buffer, Background, Shadow, data-defined rotation/size/colour) | QGIS user manual, "Labels"; GPL program, setting names and behaviour only, reimplemented | `layout/labels.rs`, `ui/src/map/labels.ts`, `docs/LABELS_PLAN.md` |
| Contour label placement along lines (arc-length walk, near-straight chord test, upright text, index contours) | own algorithm; "index contour" convention from topographic mapping practice (USGS topographic map symbols) | `layout/labels.rs` (`along_line`), `ui/src/map/labels.ts` |
| Export Table coordinate columns, reprojection with explicit CRS and a sidecar note | PROJ transformation via DuckDB Spatial `ST_Transform` (always x/y); no external method | `export_table.rs` |
| QGIS Processing algorithm ids (about 30: `native:splitvectorlayer`, `native:union`, `gdal:slope`, …) | QGIS documentation, named in each tool's header comment so users can find the equivalent | `nodes/*.rs` |
| ArcGIS Pro tool names (Spatial Autocorrelation, Hot Spot Analysis, Fill, Flow Direction, …) | proprietary software: names only, so users can find the equivalent; no code or documentation text | node descriptions |
| MGRS 100 km square lettering (LW5, grid zone designation) | Written from the MGRS description (zone, latitude band, column and row letter sets with the even-zone row offset); checked against the MIT `mgrs` package in the tests | `layout/grid.rs` `mgrs_designation` |
| USNG / MGRS references (tools Add USNG / MGRS Field, USNG / MGRS to Points, USNG / MGRS Grid; Export Table columns; automatic UTM zone) | Written from the grid's published description: the FGDC United States National Grid standard (FGDC-STD-011-2001) and the MGRS definition (zone, latitude band, 100 km column/row letter sets with the even-zone shift, Norway and Svalbard zone exceptions); WGS 84 lettering; positions by PROJ. Checked against the MIT `mgrs` package in the tests, and the polar (UPS) lettering against NGA GeoTrans values produced once with the Python `mgrs` package (MIT; installed in a throw-away folder, not shipped). No third-party code | `usng.rs`, `nodes/usng_tools.rs`, `export_table.rs` |
| Go To Coordinates (RB4): typed latitude/longitude (decimal, degrees-minutes-seconds), UTM, USNG/MGRS, map x/y and any CRS | Reuses `usng.rs` and PROJ (`crs::transform_point`) already listed; DMS reading is plain arithmetic (degrees + minutes/60 + seconds/3600). Tests use the `mgrs` package and NGA GeoTrans values already used for `usng.rs`, and `geographiclib-rs` for distances. No new software or data | `location.rs`, `tests/location.rs` |
| WhiteboxTools tools exposed as nodes (Wave 1 T1): viewshed, cost distance / pathway / allocation (Dijkstra on the 8-neighbour grid), Euclidean distance and allocation (Shih & Wu 2004), plan / profile / total / tangential / Gaussian curvature (Zevenbergen & Thorne 1987; Florinsky), TPI (Weiss 2001; Jenness 2006), DEV (Wilson & Gallant 2000), relative topographic position (Newman et al. 2018), geomorphons (Jasiewicz & Stepinski 2013), breach depressions (Lindsay 2016), FD8 flow accumulation (Freeman 1991; Quinn et al. 1991), D-infinity (Tarboton 1997), wetness index (Beven & Kirkby 1979), stream power index (Moore et al. 1991), HAND (Renno et al. 2008; Nobre et al. 2011), stream order (Strahler 1957; Shreve 1966; Horton 1945), focal filters | the program is MIT and already shipped (section 2); no code copied, each node only passes arguments | `nodes/whitebox_tools.rs` |
| SAGA tools exposed as nodes (Wave 1 T2): Multiresolution Index of Valley Bottom Flatness (Gallant & Dowling 2003), TPI-based landform classification (Weiss 2001; Guisan, Weiss & Weiss 1999), potential incoming solar radiation (Wilson & Gallant 2000; Böhner & Antonić 2009), channel network and Strahler order (O'Callaghan & Mark 1984; Strahler 1957) | each runs the unmodified `saga_cmd` program as a separate process; grids exchanged through GDAL's SAGA driver (MIT); no SAGA code copied | `saga.rs`, `nodes/saga_tools.rs` |
| LiDAR nodes (Wave 1 T8): LAS to bare-earth DEM by TIN (Delaunay) or IDW (Shepard 1968), slope-based ground filter (Vosselman 2000; Lindsay's WhiteboxTools implementation), height above ground, top-surface DSM, point density, canopy height model; file format ASPRS LAS 1.4 (R15) | the bundled WhiteboxTools program does the work (MIT, section 2); the file is read with `las`; no code copied | `nodes/lidar_tools.rs`, `pointcloud.rs` |
| Descriptive statistics (Wave ST2): compensated (Neumaier 1974) two-pass mean and variance; quantiles R type 7 (Hyndman & Fan 1996); skewness G1 and kurtosis G2 (Joanes & Gill 1998); reliability-weighted mean and variance; accuracy tests against the NIST Statistical Reference Datasets (public domain; Michelson, Mavro, NumAcc1-4, Lottery, Lew in `crates/engine/tests/data/nist/`) | published formulas implemented here; the NIST data are U.S. government works | `stats/describe.rs`, `nodes/stat_describe.rs` |
| GDAL utilities as nodes (Wave 1 T3): Proximity (`GDALComputeProximity`, as gdal_proximity.py) and Sieve (`GDALSieveFilter`, Hellemans/GDAL, as gdal_sieve.py); TRI/TPI/Roughness already exist as `ruggedness` (gdaldem; Riley et al. 1999, Wilson et al. 2007) | GDAL is MIT and already linked (section 1); no code copied | `nodes/gdal_tools.rs`, `nodes/qgis_raster.rs` |
| Kernel density, quartic kernel K(u) = (3/pi)(1-u^2)^2 | Silverman 1986, "Density Estimation for Statistics and Data Analysis", eq. 4.5 (2-D biweight kernel); the same kernel ArcGIS Pro documents | `nodes/kernel_density.rs` |
| Mean centre (on the ellipsoid: mean of Earth-centred positions returned to the surface along the normal); standard distance sqrt(sum w d^2 / sum w) with geodesic d | Neft 1966; Bachi 1963; Mitchell 2005, "The ESRI Guide to GIS Analysis, vol. 2"; Karney 2013 geodesics through `geographiclib-rs` (already linked) | `nodes/centrography.rs`, `geomkit.rs` |
| Directional distribution (standard deviational ellipse): eigen-decomposition of the weighted second moments, semi-axes sqrt(2 lambda) times the chosen number of standard deviations; computed in an azimuthal equidistant projection on a geographic CRS | Lefever 1926; Yuill 1971, "The standard deviational ellipse; an updated tool for spatial description", Geografiska Annaler B 53; PROJ `aeqd` | `nodes/centrography.rs` |
| Average nearest neighbour: E = 0.5 / sqrt(n/A), SE = 0.26136 / sqrt(n^2/A), z = (obs - E)/SE | Clark and Evans 1954, Ecology 35:445-453; constants as in ArcGIS Pro's documentation | `nodes/point_patterns.rs` |
| Ripley's K, L(d) = sqrt(K/pi), reduced-sample (border) edge correction, Monte Carlo CSR envelope; random numbers by xoshiro256** seeded through SplitMix64 | Ripley 1976, J. Applied Probability 13:255-266; Ripley 1977, J. Royal Statistical Society B 39:172-212; Besag 1977; Diggle 2003, "Statistical Analysis of Spatial Point Patterns"; Blackman and Vigna 2018 (PRNG, public domain algorithm written out in `geomkit.rs`) | `nodes/point_patterns.rs`, `geomkit.rs` |
| Tabulate area (cell-centre assignment, scan-line polygon fill) and true cell areas on the ellipsoid (authalic area of a longitude/latitude cell) | Snyder 1987, "Map Projections: A Working Manual", eq. 3-12; even-odd scan-line fill (standard) | `nodes/tabulate_area.rs`, `geomkit.rs` |
| Profile along a line: geodesic stationing, bilinear sampling at cell centres | Karney 2013 (`geographiclib-rs`); bilinear interpolation (standard) | `nodes/profile_line.rs` |
| Cut and fill: sum of (after - before) times true cell area, separately for negative and positive differences | Easa 1988 (grid volume computation); ArcGIS Cut Fill definition (names only) | `nodes/cut_fill.rs` |
| Network analysis: Dijkstra's shortest-path algorithm with a binary heap, union-find connected components, snapping to the nearest point of the nearest segment, geodesic lengths (no graph library) | Dijkstra 1959, "A note on two problems in connexion with graphs", Numerische Mathematik 1:269-271; Tarjan 1975 (union-find); Karney 2013 through `geographiclib-rs` (already linked); `petgraph` and `fast_paths` (both MIT/Apache-2.0) were considered and not added (see docs/DECISIONS.md, Wave 1 Lane B) | `network.rs`, `nodes/network_tools.rs` |
| Tool help (UX12): the citation shown in each tool's help is one of the references listed in this section, plus Douglas and Peucker 1973, Cartographica 10:112-122 (Simplify, GEOS), Delaunay 1934 (GEOS) and Easa 1988 (Cut and Fill). The before-and-after pictures are the tool's own result on a built-in sample, no outside images | the references named in each row; GEOS and DuckDB Spatial (MIT / LGPL as already listed) | `nodes/help.rs` |
| Hex bins on the H3 grid, cell area on the ellipsoid (geodesic polygon area of the cell outline) | Uber H3 through `h3o`; Karney 2013 through `geographiclib-rs` | `nodes/hex_bin.rs` |
| Natural neighbour (Sibson) interpolation; constrained Delaunay TIN with breaklines and linear (barycentric) interpolation | Sibson 1981, "A brief description of natural neighbour interpolation"; Chew 1989, constrained Delaunay triangulations; `spade` | `nodes/tin_interpolation.rs` |
| DBSCAN with geodesic neighbourhoods (written from the paper) | Ester, Kriegel, Sander and Xu 1996, KDD-96 | `nodes/dbscan.rs` |
| Supervised classification: Random Forest and k-nearest neighbours with a seeded stratified hold-out and confusion matrix | Breiman 2001, Machine Learning 45:5-32; Cover and Hart 1967; `smartcore` | `nodes/classify_supervised.rs` |

## 6. Considered and not used

| Name | Licence | Why not |
|---|---|---|
| QGIS, GRASS GIS, SAGA GIS, RichDEM | GPL | not linked or copied. Since 2026-11-06 they may ship as separate programs (decision "GPL tools as separate programs"); SAGA `saga_cmd` is the first (section 2, `docs/SAGA_PLAN.md`). Otherwise algorithms are reimplemented from their published descriptions |
| Orfeo ToolBox | Apache-2.0 | candidate for heavy remote sensing (supervised classification, segmentation); too large for now |
| PDAL | BSD | candidate for LiDAR (LAS/LAZ to DEM); not needed yet |
| `smartcore`, `linfa` | Apache-2.0 / MIT | candidates for supervised classification (waits for the training-area design) |
| the `kriging` crate (0.0.1) | MIT | too immature; kriging written directly |
| `@fluentui/react-color-picker` (9.3.2, re-exported by `@fluentui/react-components`) | MIT | the built-in colour picker (`ui/src/theme/ColorField.tsx`, `docs/COLOUR_AND_OUTLINE_PLAN.md`): saturation area, hue and alpha sliders. Already part of the shipped Fluent UI set, no new supplier |
| `statrs`, `nalgebra` | check at ST0 | candidates for distributions and matrices in the statistics workspace (`docs/STATISTICS_WORKSPACE_PLAN.md`); licences to be confirmed before use |
| NIST Statistical Reference Datasets | US public domain | planned as expected values in statistics tests; not shipped |
| R | GPL-2 or later | planned only as a source of expected numbers in statistics tests; never shipped, linked or copied |

## 7. Keeping this current

For every new or changed dependency, bundled file, downloaded source, algorithm or cited method: add or edit a row here (name, licence, role, where), update `THIRD_PARTY_NOTICES.md` if it ships (`python3 scripts/third_party_notices.py` after dependency changes, from a CI build), and cite the source in the tool's header comment. Check the licence on the project's own page at the time you adopt it; a GPL-only source is a reference for ideas, never a dependency.

_Packaging tools (not shipped): appimagetool (MIT, AppImage project) repacks the Linux image in `scripts/linux-private-libs.sh`; WhiteboxTools is built by `scripts/build-whitebox.sh` (see section 2)._
