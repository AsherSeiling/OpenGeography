# Phase H — Advanced analysis: design (2026-10-29)

Decision (user, 2026-10-29): **reuse permissively licensed libraries directly** instead of reimplementing from reference. Where the only source is GPL (QGIS, GRASS, SAGA) the algorithm is still reimplemented from its published description and its id is cited; no GPL code is bundled or linked. Project licence stays MIT OR Apache-2.0.

## Sources, in order of preference

| Order | Source | Licence | Used for | How |
|---|---|---|---|---|
| 1 | GDAL (already linked, 3.12) | MIT/X | IDW, linear (TIN), nearest, moving average (`gdal_grid`); contours; viewshed; proximity; sieve; fill-nodata | `GDALGridCreate` / `gdal` crate, same pattern as `raster_ops.rs` (progress hook + cancel) |
| 2 | DuckDB Spatial (already linked) | MIT | Zonal statistics, spatial weights by distance/contiguity, joins | SQL |
| 3 | WhiteboxTools | MIT | Hydrology: breach/fill depressions, D8/D-infinity flow direction, flow accumulation, watersheds, stream extraction | **Not published as a crate** (checked crates.io 2026-10-29). Options below |
| 4 | Small hand-written code from textbook formulas | n/a (formulas are not copyrightable) | Moran's I, Getis-Ord Gi*, Geary's C, kriging variogram fit + solve, NDVI-style indices, k-means / maximum likelihood | Rust, tested against published worked examples |

## Open decision (needs the user): how to ship WhiteboxTools

- **A. Sidecar executable.** Build `whitebox_tools` from a pinned git tag in CI for each OS, bundle it in `resources/`, run it as a child process with progress parsed from stdout, cancel by killing the process. Pros: full tool set, MIT, no engine dependency churn. Cons: one extra ~20–30 MB binary per platform, a CI build step, a process boundary.
- **B. Git dependency on the library crates.** Link its raster/hydrology code into the engine. Pros: no sidecar. Cons: pulls a large dependency tree from git, its raster I/O duplicates GDAL, version churn.
- **C. Port only the ~6 hydrology algorithms** (cited, MIT attribution kept). Pros: small, one binary, GDAL I/O. Cons: most work of the three.

Recommendation: **A** for hydrology only; everything else uses sources 1, 2 and 4. If the user prefers fewer moving parts, **C**.

## Work breakdown

| Step | Content | Source | Tests |
|---|---|---|---|
| H1 | Interpolation: IDW, TIN (linear), nearest, moving average from a point layer + field → raster; extent/cell size/CRS explicit | GDAL grid | known-surface recovery, CRS matrix row, cancel |
| H2 | Kriging (ordinary) with variogram fit | hand-written | textbook worked example, residual checks |
| H3 | Spatial statistics: Moran's I, Getis-Ord Gi* hot spots, Geary's C, zonal statistics | DuckDB + hand-written | published examples, permutation test seeded |
| H4 | Hydrology: fill depressions, flow direction, flow accumulation, watersheds, streams | WhiteboxTools (A) or port (C) | synthetic DEMs with known answers (cone, ramp, pit) |
| H5 | Remote sensing: band indices (NDVI, NDWI, NBR) via the raster calculator, unsupervised k-means classification | existing calculator + hand-written | known-pixel tests |

Each step: branch `claude/<step>`, ToolSpec in `nodes/`, added to the CRS matrix, cases in `docs/TEST_PLAN.md`, citation in `sources.rs`, cancel and progress, merge `--no-ff` with `[skip ci]`.

## Rules carried over
Explicit CRS (geodesic distance/area where meaningful, cell size in the layer's units stated), report dropped features/NoData counts, never overwrite source data, readable errors.
