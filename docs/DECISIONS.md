# Decision log

Newest first. One entry per decision that isn't obvious from the code; details live in the linked plan files. Read this only when a task touches the area. `PROJECT_STATUS.md` keeps the current state and the next action.

## Map view — `docs/MAP_PROJECTION_PLAN.md`
- **2026-10-23 Projected map view (design):** OpenLayers (BSD-2) replaces MapLibre for every CRS. The existing style JSON is reused through `ol-mapbox-style`. Web Mercator tiles stay byte-identical. Other CRSs use an engine tile grid from the CRS's area of use, and lazily built `__disp_<layer>_<crsKey>` tables projected from the source geometry, so the poles work. Features outside the area of use are reported, not drawn. proj4 in the UI maps only pixels to the map CRS; every shown or stored number goes through the engine. Planetary D2 (Mercator plus a pole spike) is superseded; there's no 3D globe.

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
- **2026-10-23 Surround elements (C2.3):**
  - **Scale bar:** segment length is the largest 1-2-5 value that fits the element (width − 8 mm); the bar is `ground · k / scale` long, with k = east scale factor at the frame centre (`geo::factors`), so for Albers it is the true east–west distance. The optional "1:N" label is not drawn.
  - **North arrow:** rotation = true-north bearing from grid up (`factors.true_north_deg` = −convergence) + the frame's page rotation, both clockwise. The compass star uses fixed ratios, not sin/cos, so its bytes cannot differ between libms.
  - **Legend:** one row per class (categorized: classes + "Other", the colour the frame gives unmatched features; graduated: "< b0", "b0 – b1", "≥ bn"), a bold heading above a layer's classes. Only layers the frame actually drew (visible, vector or raster, with a CRS) appear, top first. It does not clip: an overflowing legend is drawn and reported ("does not fit its height").
  - **Text widths** come from `ttf-parser` on the bundled Noto Sans (advances, no kerning) for ellipsizing and wrapping. `ttf-parser` was already in the tree through resvg/fontdb (MIT OR Apache-2.0); it is now a direct dependency.
  - **Sources:** `render_svg` takes a `cites: &dyn Fn(&str) -> Vec<String>`; the engine builds it from `sources::list` (layer id → "Author, Title (year)", the title cut at ": " or " — "). Derived layers already carry their inputs' sources. Frames are rendered before the other elements so surround elements can read what each frame drew; a surround element whose frame is hidden is reported and not drawn.
  - **UI:** a new frame also gets a Sources line under it (L6) and the frame is 9.5 mm shorter. Properties show positions to 0.1 mm and CRS coordinates to 0.01 without changing stored values.
- **2026-10-23 Graticule, grid, rasters, basemaps (C2.4):**
  - **Graticule:** meridians and parallels are sampled at 64 points and transformed to the frame CRS, so conics curve; the auto interval is the largest of 30°…0.00001° that gives ≥ 3 lines across the smaller span; more than 200 lines, or a frame over a pole or the antimeridian, is reported and skipped. Labels come from `Body::format_lon/lat`; the grid's labels go to the top/right when a graticule is also on. Vertices are written at 0.001 mm, so lat/lon fidelity is 1e-6° up to 1:50,000 (the P16 test frame); at smaller scales the rounding is coarser than 1e-6° by design.
  - **Grid:** straight lines at multiples of the interval in CRS units; refused for a geographic frame.
  - **Rasters:** `raster::render_extent(info, crs, extent, w_px, h_px)` is the shared renderer (`render_tile` calls it for 3857, bytes unchanged). Layout images are one `<image>` per raster layer at the frame's pixel size for `RenderRequest.raster_dpi` (default and maximum 300; the preview/PNG export pass their dpi, min 72). Hillshade pixel size is Mercator-cos(lat) for 3857, the CRS unit for projected CRSs, degrees × the CRS's ellipsoid for geographic ones.
  - **Basemaps:** `layout/basemap.rs` opens the National Map tile service with GDAL's WMS/TMS driver (level 16, EPSG:3857) and warps the frame area into the frame CRS (RGBA PNG). OSM and unknown ids are refused when the layout is saved (tile policy). A basemap that can't be fetched is a warning, not a failed export. Each drawn basemap is recorded once in the project's sources (`sources::record_once`, key `basemap:<id>`) and cited by the Sources element. `Engine::render_layout_svg_with` lets tests supply the tile source; the one network test is `#[ignore]` (`p18_network_usgs_topo_in_utm`, passes locally).
  - **Cost to know:** a preview with a basemap re-fetches tiles on every edit (4–5 s for one 100 km frame over the network). Revisit with a tile cache if it bites.

- **2026-10-23 Map display in any CRS (M1):**
  - **Key and grid:** `crs_display_info(crs, fallbackExtent?)` returns key = first 12 hex of SHA-256 of the sorted-key PROJJSON (`3857` when that equals EPSG:3857's), the proj4 string, and the grid: Web Mercator = the XYZ grid; geographic = 2:1 degree grid (res0 = 180/256); projected = square around the area of use (64 points per edge, transformed with the CRS's own geographic CS, points PROJ can't project skipped), res0 = side/256, z 0–22, origin top-left. No area of use: the caller's fallback extent + 10 % (folded into the key), else an error; geographic CRSs fall back to the world. A CRS proj4 can't express, or a non-degree angular unit, is an error.
  - **Display table** `__disp_<layer>_<key>` (`__fid`, `__d`, `__x0`, `__y0`, `__sz`, R-tree, overview tables above 50k rows via the shared `layers::build_overview_tables`): the layer's `geom` goes source → lon/lat (EPSG:4326), is clipped there to the area-of-use rectangle (two halves across the antimeridian), kept only as the source's kind of geometry (a polygon touching the edge isn't drawn as a line), then projected. The clip is in lon/lat, not in the source CRS as the plan says, so an area of use that is undefined in a projected source CRS can't fail; the cost is that clip edges are straight in lon/lat. Only vertices are transformed (no densify; DuckDB Spatial has no segmentize), as elsewhere. Non-finite or >5e8 results are dropped and counted. `dropped` = features with geometry minus rows kept.
  - **Transaction:** the table, its index, overviews and the `__disp_index` row are one `atomically` transaction inside `history::cancellable`, so Cancel leaves nothing. The index stores the layer version; a table whose version differs from the layer's is never served. `save_meta` (every layer change) cancels builds from other versions and drops stale tables, best effort; `remove_layer` drops all; project open sweeps stale and orphan tables. Budget: its own 2 GB (`display::BUDGET`), separate from the node-snapshot 2 GB, so the worst case is ~4 GB of caches per project (accepted in Opus review); LRU by `last_used` (touched by `display_status`, not per tile).
  - **Tiles:** `tiles::build_tile` and `build_tile_display` share one `mvt` query (3857 SQL unchanged; a test compares features and size to a copy of the old builder, since the thinned features come out of a GROUP BY in no fixed order). Attributes and the definition query come from a join to the layer table on `__fid`, only when a tile needs them. Route `/tiles/<layer>/<crsKey>/{z}/{x}/{y}`; the old 5-part path still means Web Mercator. A tile whose table isn't built is a 404 (never cached). The key→CRS map is filled by `crs_display_info`, so the UI must call it first.
  - **Commands:** `build_display` starts the cancellable "Projecting <layer> for display" task in a thread and returns the status; `display_status` = { ready, building, dropped, error }. Perf (release, M-series Mac, `p6_one_million_points`): 1M points → display table 3.8 s; dense tiles 3.7 ms (z2) and 2.6 ms (z4) through the overview tables. Not in M1: `basemapOffsetM` (plan R1, M2), raster tiles per CRS (M4).

## UI
- **2026-10-23 Node editor:**
  - Blender keys work only over the editor (`nodes/keymap.ts`, rebindable).
  - Inserts are one step and type-checked.
  - Nodes show changed/stale markers; mute = bypass in `graphPayload`.
- **2026-10-23 Catalog pane:** has a "This project" view. TIGER downloads by year. The bibliography exports automatically (APA/MLA/Chicago, BibTeX, RIS).

## Platforms and release
- **2026-10-23 Linux AppImage:** private libs with unique `gs_` sonames (`5fd5c23`). Linux builds with conda-forge GDAL 3.12 and passes the strict licence gate. Local Homebrew macOS builds still warn; CI release builds are GPL-free.
