# GIS Studio

A free, open-source, node-based GIS for geoscientists and government staff.
Office / ArcGIS Pro-style interface, fast geoprocessing on DuckDB Spatial, and a
visual Model (node editor) for chaining tools like Buffer → Clip.

> Working name. Milestone 1 of the plan: map, layers, attribute table, coordinate
> systems, and the Model. Layouts (M2), rasters + National Map / NASA / Census
> data nodes (M3) and the Topo Map Builder (M4) come next.

## Run it

Prerequisites: Rust (`brew install rustup && rustup default stable`), Node 22+,
and DuckDB's library (`brew install duckdb` on macOS; see `.cargo/config.toml`
for other platforms, or add `--features bundled` to compile DuckDB from source).

```bash
npm install
npm run tauri dev
```

Drag shapefiles, GeoPackages, GeoJSON, KML, CSV (with lat/lon columns) or
GeoParquet onto the window, use **Map → Add Data**, or download Census
boundaries/roads/water for any state or county with **Map → Sample Data (TIGER)**.

Each layer has its own node tree (select it in Contents): Layer Input → tools →
Output. The Output redraws the layer without changing the original data;
**Save as new layer** keeps a copy. The attribute table edits like a spreadsheet
(type to edit, Enter/Tab to move, Ctrl+Z to undo; right-click a column header
for Calculate Field / Calculate Geometry / Rename / Delete).

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

```bash
cargo test -p engine                                  # engine: CRS, tools, tiles, editing, projects
cargo test -p engine tiger -- --ignored               # live Census TIGERweb download (needs internet)
npm test                                              # UI: symbology, scale, CRS search
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
| `ui/src` | React UI: ribbon, dockable panes, map (MapLibre), Model (React Flow) |
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
  so editing one step re-runs only what's downstream.

License: MIT OR Apache-2.0.
