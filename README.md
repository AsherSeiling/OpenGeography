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
GeoParquet onto the window, or use **Map → Add Data**.

### Run the UI in a browser (development/testing)

```bash
cargo run --release -p devserver   # engine over HTTP on 127.0.0.1:7878
npm run dev                         # open http://localhost:1420
```

## Test

```bash
cargo test -p engine                                  # engine: CRS, tools, tiles, projects
npm test                                              # UI: symbology, scale, CRS search
cargo run --release -p engine --bin bench_tiles       # map performance gate
```

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
