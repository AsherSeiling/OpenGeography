# Open-source code for more tools: candidates

Written 2026-11-06 at the user's request. A list, not a plan: nothing here is chosen or built.

**The user's rule:** a source is acceptable if it can be packaged natively. Code in a slow language is rewritten in Rust, or a fast alternative is used.

**The project's rules still apply:**
- Use permissive code (MIT, Apache, BSD, ISC, Boost) directly.
- GPL tools may ship unmodified as separate programs when that isn't much slower (user, 2026-11-06). They are never linked or copied in; otherwise their algorithms are reimplemented and their ids cited.
- Each addition goes in `docs/OPEN_SOURCE.md` in the same commit.

The licences below are as I know them. **Check each licence again before adding it**: some projects split an MIT core from paid or GPL parts.

There are about 158 tools today. The "Gives" columns name tools that don't exist yet, checked against the tool ids in `crates/engine/src/nodes/`.

## Tier 1: already shipped or linked, so close to free

| Source | Licence | Language | Gives | Notes |
|---|---|---|---|---|
| **WhiteboxTools** (already shipped, v2.4.0) | MIT (open core) | Rust | Hundreds of tools, of which only the hydrology nodes are used. Not yet exposed: **viewshed**, **cost distance and cost path**, Euclidean distance and allocation, curvature (profile, plan, total), TPI and deviation from mean elevation, wetness index, **HAND** (height above nearest drainage), stream order (Strahler, Shreve), more watershed tools, focal statistics and filters, **LiDAR** (LAS → DEM, ground classification, normalising heights), image tools (PCA, change detection, edge detection), raster overlay | The biggest gain for the least work: the program is already bundled and called through `whitebox.rs`. Each tool needs only a ToolSpec, tests and a citation. Avoid the paid "Whitebox Toolset Extension" tools, which aren't open. |
| **GDAL** utilities (already linked) | MIT | C/C++ | **Viewshed** (`GDALViewshedGenerate`), proximity / distance raster, sieve (remove small patches), TRI / TPI / roughness / colour relief (`gdaldem`), pansharpening, footprints, VRT mosaics | Called through the `gdal` crate, like warp and contours. Check GDAL 3.12 for a built-in zonal-stats command based on ExactExtract (see Tier 3). |
| **DuckDB Spatial / GEOS** (already linked) | MIT / LGPL-2.1 | C++ | Line merge, node lines, maximum inscribed circle, reduce precision, coverage simplify (shared edges stay shared), snapping | Exposed through SQL, like today's vector tools. Check which GEOS functions this DuckDB Spatial version exposes. |

## Tier 2: Rust crates (native, permissive, small)

| Crate | Licence | Gives |
|---|---|---|
| `geo` (georust) | MIT / Apache-2.0 | Smoothing (Chaikin), Visvalingam–Whyatt simplification, geodesic densify, Fréchet and Hausdorff distance, line interpolation, centroids, rotate and scale (shape editing E3 too) |
| `rstar`, `kiddo` | MIT / Apache-2.0 | Spatial indexes for fast nearest neighbours: near tables, k-nearest, faster IDW and kriging neighbourhoods |
| `spade` | MIT / Apache-2.0 | Constrained Delaunay (breaklines), **natural neighbour interpolation**, TIN from points and breaklines |
| `h3o` (Rust H3) | BSD-3 (H3 itself Apache-2.0) | **Hex binning** and H3 aggregation |
| `petgraph` + `fast_paths` | MIT / Apache-2.0 | **Network analysis** on road layers (TIGER, National Map, Overture): shortest route, **service areas / drive-time zones**, closest facility, origin–destination matrix |
| `osmpbf` | MIT / Apache-2.0 | Reads OpenStreetMap extracts, for networks and features |
| `linfa`, `smartcore` | MIT or Apache-2.0 / Apache-2.0 | **Supervised classification** (random forest, SVM), **DBSCAN** clustering, PCA. Already listed as candidates in `OPEN_SOURCE.md`. |
| `las`, `laz` | MIT / Apache-2.0 | Read LAS/LAZ point clouds. With WhiteboxTools' LiDAR tools, this closes the 3DEP lidar gap. |
| `ndarray`, `rayon` | MIT / Apache-2.0 | Fast parallel raster loops for anything rewritten in Rust (kernel density, focal statistics) |
| `pmtiles` | MIT / Apache-2.0 | Export maps as PMTiles for the web |

## Tier 3: native C/C++ libraries (more packaging work)

| Source | Licence | Gives | Cost |
|---|---|---|---|
| **ExactExtract** | Apache-2.0 | **Exact zonal statistics**: partial cells weighted by how much they overlap the polygon | Small. It may already be in GDAL 3.12; check that first. |
| **Valhalla** | MIT | Full routing: turn restrictions, isochrones, map matching | Large, with its own tile build. Only if `petgraph` routing isn't enough. |
| **OSRM** | BSD-2 | Fast routing | Large; same as Valhalla |
| **PDAL** | BSD | Full point-cloud pipelines | Large. Use the WhiteboxTools and `las` route first. |
| **Orfeo ToolBox** | Apache-2.0 | Remote sensing: object-based segmentation, classification, SAR | Very large (ITK) |
| **OpenCV** | Apache-2.0 | Image segmentation, feature matching, morphology | Large; take only the modules needed |
| **Tippecanoe** | BSD-2 | Vector tiles from big layers | Medium; only if web export becomes a feature |
| **SFCGAL** | LGPL-2.0 | 3D geometry, straight skeleton (roof lines, centrelines) | Medium; dynamically linked like GEOS |

## Tier 4: Python or JavaScript code, rewritten in Rust (permissive, so methods can be followed closely)

| Source | Licence | Gives once rewritten |
|---|---|---|
| **PySAL** (`esda`, `libpysal`, `mgwr`, `spreg`) | BSD-3 | **Local Moran's I (LISA)**, Geary's C, spatial weights options, **geographically weighted regression (GWR / MGWR)**, spatial lag and error regression, join counts |
| **scikit-learn** | BSD-3 | Methods for anything `linfa` lacks (HDBSCAN, isolation forest) |
| **PyKrige** | BSD-3 | Universal and regression kriging (extends `kriging.rs`) |
| **OSMnx** | MIT | Street network metrics: centrality, connectivity, intersection density |
| **momepy** | BSD-3 | Urban morphology: building and block shape metrics |
| **Turf.js** | MIT | Many small vector operations, for any `geo` doesn't cover |
| **pvlib** | BSD-3 | Solar position and irradiance, for a **solar radiation** tool on DEMs |
| **Landlab** | MIT | Landscape evolution and flow routing models (for geoscience) |
| **rasterstats** | BSD-3 | Zonal statistics reference (ExactExtract replaces it) |

Easy tools with no library needed, from textbook methods:
- **kernel density** (points and lines);
- point density;
- **mean centre, standard distance, directional ellipse**;
- average nearest neighbour;
- **Ripley's K**;
- tabulate area and cross tabulation;
- profile along a line;
- cut and fill / volume between surfaces.

## GPL: as separate programs, or reference only (rule changed 2026-11-06)

**The user's rule since 2026-11-06:** a GPL tool may ship unmodified as a separate program the app runs, like WhiteboxTools, when that isn't much slower than a rewrite. It is never linked or copied in. Its licence and source ship with it (`docs/DECISIONS.md`, "GPL tools as separate programs").

| Source | Licence | As a separate program? | Gives |
|---|---|---|---|
| **SAGA GIS** (`saga_cmd`) | GPL-2 (some parts LGPL) | **Good fit.** A command-line program with hundreds of tool libraries, in C++; reads and writes GeoTIFF and shapefiles. Build the command line without the GUI. | Terrain morphometry, landform classification, channel networks, solar radiation, many grid and vector tools |
| **RichDEM** | GPL-3 | **Good fit.** Small C++ command-line tools | Priority-flood depression filling, flow metrics, terrain attributes |
| **GeoDa** (`libgeoda`) | GPL-3 | **Possible.** Needs a small command-line wrapper, which is itself GPL and shipped with its source | LISA, spatial regression, regionalisation (SKATER, REDCAP) |
| **GRASS GIS** | GPL-2 | **Doubtful.** Its tools need a GRASS database and session, many are Python scripts, and it's hundreds of MB. Check packaging on all three platforms before relying on it. | `r.sun`, `r.cost` / `r.walk`, `v.net.*`, `r.watershed` |
| **TauDEM** | GPL-3 | **Doubtful.** Needs MPI at run time | D-infinity flow, terrain stability |
| QGIS Processing | GPL-2 | **No.** It needs QGIS (standalone rule) | Tool behaviour and naming users know (already cited by id) |
| gstat | GPL | **No.** An R package | Geostatistics behaviour, for reference |
| pgRouting | GPL-2 | **No.** Needs PostgreSQL | Routing behaviour, for reference |

A GPL program adds its licence, a pinned source and build script, and a `THIRD_PARTY_NOTICES.md` entry. Once one ships, the installers are no longer "GPL-free", though GIS Studio's own code stays MIT OR Apache-2.0.

## Recommended order (most useful per unit of work)

1. **WhiteboxTools tools already shipped:** viewshed, cost distance and path, distance and allocation, curvature, TPI, wetness, HAND, stream order, LiDAR to DEM, focal statistics. About 30–40 useful tools, mostly ToolSpec wiring.
2. **SAGA's command-line program, as a separate GPL program:** hundreds of terrain and grid tools in one bundled program, checked for speed and packaging first.
3. **GDAL utilities:** viewshed, if WhiteboxTools' doesn't fit, plus proximity, sieve and TRI/TPI.
4. **Textbook tools with no dependency:** kernel density, mean centre, standard distance, directional ellipse, average nearest neighbour, Ripley's K, tabulate area, profile, cut and fill.
5. **`petgraph` + `fast_paths` network analysis:** routes, service areas and closest facility. ArcGIS Pro sells this as an extension, so it's a strong selling point.
6. **`h3o` hex binning, `spade` natural neighbour, `linfa` supervised classification and DBSCAN.**
7. **PySAL rewrites:** LISA, GWR, spatial regression.
8. **Point clouds:** `las` / `laz` with WhiteboxTools LiDAR.

Each would be a short Opus check of licence and method, then Sonnet packages adding ToolSpecs with tests. Placed in the order as Phase T in `docs/MASTER_PLAN.md`: after shape editing, before the Census importer (user, 2026-11-06).
