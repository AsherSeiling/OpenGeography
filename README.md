# GIS Studio

A free, open-source, node-based GIS for geoscientists and government staff.
Office / ArcGIS Pro-style interface, fast geoprocessing on DuckDB Spatial, and a
visual Model (node editor) for chaining tools like Buffer → Clip.

> Working name. Done: map in any coordinate system, layers, attribute table and
> attribute history between model steps, the Model, rasters, geodesic measuring
> and land navigation, print layouts (PDF, PNG, SVG), and data downloads from
> Census TIGER and USGS The National Map (vector data and 3DEP elevation).
> Next: rasters drawn in any coordinate system, layout templates and map series,
> Census data joins, NAIP and NASA data, and the Topo Map Builder.
> Current status and plan: [PROJECT_STATUS.md](PROJECT_STATUS.md) and [docs/MASTER_PLAN.md](docs/MASTER_PLAN.md).

## Run it

Prerequisites: Rust (`brew install rustup && rustup default stable`), Node 22+,
and DuckDB's library (`brew install duckdb` on macOS; see `.cargo/config.toml`
for other platforms, or add `--features bundled` to compile DuckDB from source).

```bash
npm install
npm run tauri dev
```

Drag shapefiles, GeoPackages, GeoJSON, KML, CSV (with lat/lon columns) or
GeoParquet onto the window, use **Map → Add Data**, or download data:

- **Map → Sample Data (TIGER)**: Census boundaries, roads and water for any state
  or county (tracts, block groups and blocks carry 2020 population).
- **Map → National Map**: USGS streams, lakes and watersheds (NHD, WBD), roads,
  trails, railroads and airports, structures (schools, hospitals, fire and police),
  geographic names (GNIS) and federal lands, for the map view or a layer's area,
  clipped to it.
- **Map → Elevation (3DEP)**: USGS elevation, then Contours / Hillshade / Slope.

Long work (downloads, imports, model runs, map tiles) shows a progress bar in the
dialog and the status bar; downloads can be cancelled.

**Data → Sources & Citations** lists every data source the project has used —
files, TIGER, each National Map product, 3DEP and basemaps — with a citation,
an annotation and the layers it was used for. Local files start as incomplete
citations you fill in. Export the list as an annotated bibliography (Markdown)
or BibTeX. The log is saved with the project.

**Map → Coordinate System** sets the map's coordinate system, and the map is
drawn in it: UTM, State Plane, Albers, polar stereographic and the rest. Identify,
select and measure work in that system; the status bar shows the true scale at
the map centre. Features outside a system's area of use are reported, not
silently dropped.

**Insert → New Layout** makes a print page: map frames in any coordinate system
and scale, legend, scale bar, north arrow (true north), text, a sources block,
graticule and grid, rasters and USGS basemaps. Export to PDF (vector), PNG or SVG.
Fonts are bundled, so a page looks the same on every computer.

Each layer has its own node tree (select it in Contents): Layer Input → tools →
Output. The Output redraws the layer without changing the original data;
**Save as new layer** keeps a copy. The attribute table edits like a spreadsheet
(type to edit, Enter/Tab to move, Ctrl+Z to undo; right-click a column header
for Calculate Field / Calculate Geometry / Rename / Delete). The table can also
show any step of a node tree and compare two steps (rows added, removed and
changed), so you can see what each tool did to the data.

Errors open a pop-up with a plain message and the details folded away (Copy,
Dismiss, Reload). If one pane fails, only that pane shows "Reopen"; the rest of
the app keeps working.

### Run the UI in a browser (development/testing)

```bash
cargo run --release -p devserver   # engine over HTTP on 127.0.0.1:7878
npm run dev                         # open http://localhost:1420
```

## Build a self-contained installer

macOS (Apple Silicon), on a Mac with Homebrew:

```bash
brew install gdal duckdb dylibbundler
scripts/package-macos.sh
```

This produces `target/release/bundle/GIS-Studio-<version>-macos-arm64.dmg`.
GDAL, PROJ data, DuckDB and its spatial extension are bundled inside the app
(about 190 libraries), so users install nothing else. Check an installed app
with `GIS\ Studio.app/Contents/MacOS/gis-studio --self-test`.
The app is ad-hoc signed; for distribution without Gatekeeper warnings,
sign it with an Apple Developer ID and notarize it.

Windows and Linux installers are built by `.github/workflows/build.yml`.

## Test

The test plan is `docs/TEST_PLAN.md` (one row per case for every feature); open
bugs are in `docs/BUGS.md`. One script runs the automated part and writes a
summary to `test-results/<time>/summary.md`:

```bash
scripts/run-test-plan.sh --quick      # type check, UI tests, engine tests (what CI runs)
scripts/run-test-plan.sh              # + every ignored engine test, one by one (network, performance, known bugs)
scripts/run-test-plan.sh --e2e        # + browser end-to-end tests (Playwright, headless Chrome)
scripts/run-test-plan.sh --package    # + build the Mac app and run its --self-test
```

The summary ends with the manual cases that need a person (file dialogs,
installers, other operating systems). Single suites:

```bash
cargo test -p engine                                  # engine: CRS, tools, tiles, layouts, history, editing, projects
npx vitest run && npx tsc -p tsconfig.json            # UI tests and type check
scripts/e2e.sh test-results/e2e                       # browser end-to-end only
cargo run --release -p engine --bin bench_tiles       # map performance gate
```

## Tools

Vector, table and raster tools run natively in GIS Studio (no QGIS or other
GIS needed). Many mirror QGIS Processing algorithms by name and behavior so
QGIS users feel at home; each such tool cites the QGIS algorithm id and
documentation in its source (`crates/engine/src/nodes/`). They are original
implementations on DuckDB Spatial and GDAL; no QGIS code is used, so the
project stays MIT/Apache-licensed.

## Layout

| Path | What |
|---|---|
| `crates/engine` | Rust engine: DuckDB storage, CRS handling, vector tiles, model executor |
| `crates/engine/src/nodes/` | One file per geoprocessing tool. Add a tool = add a file + one registry line |
| `src-tauri` | Desktop shell: IPC commands and the `gis://` tile protocol |
| `crates/devserver` | Same engine over HTTP, for browser-based development |
| `ui/src` | React UI: ribbon, dockable panes, map (OpenLayers), Model (React Flow), layouts, error pop-up |
| `crates/engine/src/layout/` | Print layouts: page model, body-aware geodesy, SVG → PDF/PNG |
| `e2e/` | Browser end-to-end tests (Playwright) |
| `docs/` | Plans, decisions, test plan, bug list, CI log; start with `docs/MASTER_PLAN.md` |
| `scripts/build-crs-list.sh` | Regenerates the EPSG/ESRI catalog from PROJ's `proj.db` |

## Design notes

- **Coordinate systems are never guessed.** Layers without a CRS are flagged and
  not drawn until you Define Projection. Tools reproject mismatched inputs and
  say so. Buffer distances are true ground distances; Calculate Geometry is
  geodesic.
- **Map speed.** The map only receives vector tiles. Large layers get an overview
  pyramid; zooms 0–8 are pre-built in the background, deeper zooms are built on
  demand (p95 ≈ 20 ms for 700K polygons, ≈ 9 ms for 5M points on an M-series Mac).
- **Model caching.** Each step's output is cached by its inputs and parameters,
  so editing one step re-runs only what's downstream. The cache is saved with the
  project, which is what lets the attribute table compare any two steps.
- **Map in any coordinate system.** The engine cuts tiles on a grid for the map's
  coordinate system and keeps a display copy of each layer projected from its
  original data, so even polar maps work. Every number you see or store is
  computed by PROJ in the engine, not approximated in the browser.
- **Print layouts are engine-rendered.** Pages are drawn to SVG and then to PDF or
  PNG, so a printed map has a true scale in its own coordinate system.

## Open-source software and credits

GIS Studio is built on the open-source projects below. Thank you to their
authors. The complete list — every Rust crate, every JavaScript package in the
UI, and every native library bundled in the macOS app, each with its version and
license — is in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)
(regenerate with `python3 scripts/third_party_notices.py`; it also ships inside
the app).

### Core engine

| Software | Used for | License |
|---|---|---|
| [GDAL](https://gdal.org) | Reading/writing vector and raster formats, warping, DEM tools, contours | MIT |
| [PROJ](https://proj.org) | Coordinate reference systems and transformations | MIT |
| [GEOS](https://libgeos.org) | Geometry operations (inside DuckDB Spatial and GDAL) | LGPL-2.1-or-later |
| [DuckDB](https://duckdb.org) + [DuckDB Spatial](https://github.com/duckdb/duckdb-spatial) | Layer storage, SQL geoprocessing, vector tiles | MIT |
| [duckdb-rs](https://github.com/duckdb/duckdb-rs), [georust/gdal](https://github.com/georust/gdal) | Rust bindings to DuckDB and GDAL | MIT |
| [Tauri](https://tauri.app) | Desktop shell | Apache-2.0 OR MIT |
| [svg2pdf](https://github.com/typst/svg2pdf), [resvg](https://github.com/linebender/resvg) | Layout export: SVG to vector PDF and to PNG | MIT OR Apache-2.0 |
| [geographiclib-rs](https://github.com/georust/geographiclib-rs) | Geodesics for layout scale bars, north arrows and frames | MIT |
| [ureq](https://github.com/algesten/ureq), [serde](https://serde.rs), [anyhow](https://github.com/dtolnay/anyhow), [parking_lot](https://github.com/Amanieu/parking_lot), [lru](https://github.com/jeromefroe/lru-rs), [png](https://github.com/image-rs/image-png), [sha2](https://github.com/RustCrypto/hashes), [tiny_http](https://github.com/tiny-http/tiny-http) | HTTP downloads, JSON, errors, locking, caches, PNG tiles, model cache keys, dev server | MIT and/or Apache-2.0 |

### User interface

| Software | Used for | License |
|---|---|---|
| [OpenLayers](https://openlayers.org) + [ol-mapbox-style](https://github.com/openlayers/ol-mapbox-style) | The map, in any coordinate system, and its layer styles | BSD-2-Clause |
| [React](https://react.dev) | UI framework | MIT |
| [Fluent UI React](https://github.com/microsoft/fluentui) + [Fluent icons](https://github.com/microsoft/fluentui-system-icons) | Office-style controls and icons | MIT |
| [React Flow (@xyflow/react)](https://reactflow.dev) | The Model (node editor) | MIT |
| [dockview](https://dockview.dev) | Dockable panes | MIT |
| [TanStack Virtual](https://tanstack.com/virtual) | Fast attribute table | MIT |
| [Zustand](https://github.com/pmndrs/zustand), [zundo](https://github.com/charkour/zundo) | State and undo | MIT |
| [GeographicLib (geographiclib-geodesic)](https://geographiclib.sourceforge.io) | Geodesic distance, azimuth and area | MIT |
| [proj4js](https://github.com/proj4js/proj4js), [mgrs](https://github.com/proj4js/mgrs) | UTM and MGRS conversions in the UI | MIT |
| [geomagnetism](https://github.com/naturalatlas/geomagnetism) | Magnetic declination (World Magnetic Model) | Apache-2.0 |
| [Vite](https://vite.dev), [Vitest](https://vitest.dev), [TypeScript](https://www.typescriptlang.org), [Playwright](https://playwright.dev) | Build and tests (not shipped) | MIT / Apache-2.0 |

### Bundled data

The [EPSG Geodetic Parameter Dataset](https://epsg.org) (© IOGP, used under its
[terms of use](https://epsg.org/terms-of-use.html)) inside PROJ's database; PROJ
and GDAL support data (MIT); World Magnetic Model 2025 coefficients (public domain);
[Noto Sans](https://notofonts.github.io) Regular, Bold and Italic for print layouts
(SIL Open Font License 1.1, © The Noto Project Authors).

### References for methods

- GDAL/OGR contributors (2026). *GDAL/OGR Geospatial Data Abstraction software Library*. Open Source Geospatial Foundation. https://gdal.org
- PROJ contributors (2026). *PROJ coordinate transformation software library*. Open Source Geospatial Foundation. https://proj.org
- GEOS contributors (2026). *GEOS computational geometry library*. Open Source Geospatial Foundation. https://libgeos.org
- Raasveldt, M., & Mühleisen, H. (2019). DuckDB: an embeddable analytical database. *Proceedings of the 2019 International Conference on Management of Data (SIGMOD '19)*, 1981–1984. https://doi.org/10.1145/3299869.3320212
- Karney, C. F. F. (2013). Algorithms for geodesics. *Journal of Geodesy, 87*(1), 43–55. https://doi.org/10.1007/s00190-012-0578-z — geodesic measuring, buffers and areas.
- NOAA National Centers for Environmental Information & British Geological Survey (2024). *The US/UK World Magnetic Model for 2025–2030*. https://www.ncei.noaa.gov/products/world-magnetic-model — magnetic declination and G-M angle.
- Headquarters, Department of the Army (2013). *TC 3-25.26: Map Reading and Land Navigation*. — resection, intersection, back azimuths, mils and the G-M angle as taught for military map reading.
- QGIS Development Team (2026). *QGIS Geographic Information System*. QGIS Association. https://qgis.org — many tools mirror QGIS Processing algorithms by name and behavior (each cites its QGIS algorithm in `crates/engine/src/nodes/`); no QGIS code is used.
- Hinnant, H. *chrono-Compatible Low-Level Date Algorithms*. https://howardhinnant.github.io/date_algorithms.html — date conversion for National Map attributes.

Data downloaded through the app is credited separately, per project, in **Data → Sources & Citations**.

### Licensing note for installers

Release installers for macOS and Windows are built in CI with conda-forge's
`libgdal-core` 3.12 and its LGPL SpatiaLite build, which leave out the three
GPL-only libraries some GDAL builds include (poppler for PDF, x265 for HEIF,
librttopo for SpatiaLite topology). A license gate (`scripts/license-gate.sh`)
fails the release build if any of them appears. Local macOS builds made with
Homebrew's GDAL still include them and are not for distribution (the packaging
script warns). The Linux AppImage is built the same way as
macOS and Windows (conda-forge GDAL, kept private inside the image) and also
passes the license gate. GIS Studio's own code is MIT OR Apache-2.0.

License: MIT OR Apache-2.0.
