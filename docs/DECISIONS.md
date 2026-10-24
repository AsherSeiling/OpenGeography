# Decision log

Newest first. One entry per decision that isn't obvious from the code; details live in the linked plan files. Read this only when a task touches the area. `PROJECT_STATUS.md` keeps the current state and the next action.

## Layouts (Phase C) — `docs/LAYOUTS_PLAN.md`
- **2026-10-23 C2.1 renderer core** (`crates/engine/src/layout/`):
  - **Modules:** `model.rs` is the L2 model plus a `LayerStyle` mirror. `geo.rs` holds `Body`, `FrameCrs`, the scale factor k, convergence and extent. `frame.rs` turns a layer into styled SVG paths: DuckDB transform → clip → simplify at 0.1 mm. `svg.rs` does fixed-precision numbers. `export.rs` does PDF (svg2pdf) and PNG (resvg).
  - **Geodesy:** k and convergence use the frame CRS's *own* geographic CS, not WGS 84; a datum step perturbed k by 2.5e-6. The `Body` supplies only the ellipsoid. In non-conformal CRSs, k is the grid-east scale, which is the scale bar's direction.
  - **Styling:** opacity uses `fill-opacity`/`stroke-opacity`, so the PDF has no transparency groups. UI px → mm at 96 dpi. Graduated classes follow MapLibre `step`.
  - **Dependencies:** svg2pdf 0.13, resvg 0.45 (pinned to svg2pdf's usvg), geographiclib-rs 0.2; Opus licence review passed. Noto Sans (OFL) is compiled in; system fonts are never used.
  - **Open item:** the P3 golden SVG SHA must match on Linux/Windows CI (conda PROJ).
- **2026-10-23 C1 design**:
  - The engine renders each page to SVG, then PDF and PNG; MapLibre can't draw a true CRS.
  - Fonts are bundled only. Frames take an explicit CRS.
  - Scale bar and north arrow are geodesic at the frame centre, through a `Body` (Earth only until F1).
  - Layouts are stored in `__layouts`.

## Attribute history (Phase B) — `docs/ATTRIBUTE_HISTORY_PLAN.md`
- **2026-10-23 B2.3 table pane UI** (`ui/src/panes/table/`):
  - **Modes:** node mode is read-only. Diff mode's chips filter the grid. The node card menu has View table and Compare with input.
  - **Saving:** each node's `lastOutput` is saved in project.json. `pruneNodeCache(live, pinned)` runs on save and when a tree is removed.
  - **Lost diffs:** a diff is dropped on close, node switch or compare change. A diff lost to reopen/Save As re-runs once, then shows a prompt; the grid is never blank.
  - **Known gap:** undoing a tree edit can drop `lastOutput`. The node then shows "not run" until the next run, which is a cache hit.
- **2026-10-23 B2.2 diff + node rows** (`history.rs`):
  - **Diff tables:** each call gets a unique `__diff_<a>_<b>_<key>_<n>` table. They're never indexed, and are swept on open and new project.
  - **Paging:** node-row `fids` are row positions. Layer Input views page by `row_number()`.
  - **Speed:** 1M rows × 20 fields key diff: 0.44 s.
- **2026-10-23 B2.1 persisted node cache**:
  - **Index:** `__node_<hash>` tables are indexed in `__node_cache`, written in the CREATE's transaction.
  - **On open:** tables from another `engine_version` (`CARGO_PKG_VERSION+CACHE_FORMAT`), broken ones and orphans are dropped. Bump `graph::CACHE_FORMAT` when a tool's SQL changes meaning.
  - **Pruning:** `prune_node_cache(live, pinned)` drops non-live tables, then applies a 2 GB budget; views count 0 bytes. Raster entries stay session-only.
- **2026-10-23 B1 design**:
  - Snapshots are the cache tables.
  - The diff key is the user's key, else an auto-detected unique key, else a content-hash multiset diff.
  - Geometry is never compared across CRSs.

## Engine and tools
- **2026-10-23 Setting validation:** `nodes::validate_params` rejects non-numeric Number values and Enum values outside the options. Empty means unset.
- **2026-10-23 Overlay tools:**
  - Union and Symmetrical Difference share `union.rs` (`minus_sql`/`overlay_ctes`). Singlepart To Multipart is `collect`.
  - Concave hull is a shape in Minimum Bounding Geometry.
  - Delaunay goes through GDAL/GEOS and is capped at 200k features.
- **2026-10-23 Select By Location** (Map → Selection): shares `nodes::select_by_location::location_test` with the model node. Distance is geodesic.
- **2026-10-23 Model-run cancellation:**
  - A watcher thread calls DuckDB `interrupt()` repeatedly until the run returns, because DuckDB clears the flag at each query start.
  - Result writes (`set_result`, `upsert_layer`) run in one transaction, so a cancelled write keeps the previous result.
- **2026-10-23 Provenance:** Output and Add to Map record the sources of every upstream Layer input, in the result's transaction.
- **2026-10-23 Geodesic speed:** aeqd centres snap to 0.25°: about 30× faster, with < 0.1 m error on 50 km buffers.
- **2026-10-23 Toolchain pins:**
  - GDAL 3.12 everywhere (gdal crate 0.19 can't use 3.13): conda-forge `libgdal-core=3.12` in CI, Homebrew 3.12.3 locally.
  - DuckDB 1.5.6.
  - Never `brew upgrade gdal duckdb`.
  - The spatial extension ships gzip-compressed.

## Layouts
- **2026-10-23 Layout preview transport (C2.2):** the engine returns the preview PNG base64-encoded inside the command's JSON (`render_layout_preview` → `{ png, pngBytes, warnings }`) instead of a `gis://` URL. Measured through the dev server (debug build): Letter landscape, 60 polygons, 96 dpi: 61 KB PNG, 33 ms round trip. A URL would need the layout and styles in a query string or a server-side cache keyed by hash, for no visible gain at this size. Revisit if a page with rasters (C2.4) makes previews more than ~1 MB. Adds `base64` as a direct engine dependency (already in the tree; MIT OR Apache-2.0).
- **2026-10-23 Layout undo (C2.2):** `store/layout.ts` keeps the open layout in `doc` (zundo, like `model.ts`) and stashes each layout's past/future when another tab becomes active, so history is per layout. A drag or arrow nudge is one step (`beginStep`); typing in one field is one step (`burst`).
- **2026-10-23 Layout persistence (C2.2):** layouts save to `__layouts` through `save_layout` 500 ms after an edit and before every project save; they ride along in `project.duckdb`. Tabs return with the dock layout (`layout:<id>` panels, `layoutId` param). Layers in a frame follow the map's order/visibility; a node-tree result replaces its layer (same id swap as on screen).
- **2026-10-23 Export (C2.2):** `export_layout` is a cancellable `progress::Task` ("Exporting layout — name"); Cancel is polled between layers and before encoding; the file is written as `name.ext.part` and renamed. It returns the render warnings. PNG is 300 dpi for now; the browser build asks for the path with a prompt.

## UI
- **2026-10-23 Node editor:**
  - Blender keys work only over the editor (`nodes/keymap.ts`, rebindable).
  - Inserts are one step and type-checked.
  - Nodes show changed/stale markers; mute = bypass in `graphPayload`.
- **2026-10-23 Catalog pane:** has a "This project" view. TIGER downloads by year. The bibliography exports automatically (APA/MLA/Chicago, BibTeX, RIS).

## Platforms and release
- **2026-10-23 Linux AppImage:** private libs with unique `gs_` sonames (`5fd5c23`). Linux builds with conda-forge GDAL 3.12 and passes the strict licence gate. Local Homebrew macOS builds still warn; CI release builds are GPL-free.
