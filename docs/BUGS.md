# Known bugs

Bugs that are not yet fixed. Each one has a test that fails today and is marked `#[ignore = "BUG: …"]`; remove the `#[ignore]` when you fix the bug (run all of them with `cargo test -p engine -- --ignored <test name>`).
Severity: **High** = crash, data loss or a wrong result without a warning · **Medium** = wrong behaviour with a workaround · **Low** = rough edge.
Known performance and platform issues that are not bugs in the code live in `PROJECT_STATUS.md` ("Known issues").

## Found by tests

Found 2026-10-25 while writing `docs/TEST_PLAN.md`'s tests. Nothing here was fixed by that work.

| ID | Sev. | Summary | Test |
|---|---|---|---|
| B-T5 | High | **Fixed** —  Warp to an invalid CRS crashes the whole process (SIGSEGV) | `rasters::warp_to_an_invalid_crs_is_an_error_not_a_crash` |
| B-T7 | High | **Fixed** —  Warp of a raster with no CRS "succeeds" and labels the untouched grid with the target CRS | `rasters::warping_a_raster_of_unknown_crs_is_refused_not_guessed` |
| B-T1 | Medium | **Fixed** —  Export can overwrite the layer's own source file | `formats::exporting_never_overwrites_the_layers_own_source_file` |
| B-T4 | Medium | **Fixed** —  Cancel takes ~12 s to stop a geodesic Buffer (1.5 M points) or a Dissolve (200 k polygons) | `cancel::cancel_geodesic_buffer`, `cancel::cancel_dissolve` |
| B-T2 | Low | **Fixed** —  A layer can be renamed to a blank name | `tables::renaming_a_layer_to_blank_is_refused` |
| B-T3 | Low | **Fixed by M3** (`select_rect`): rectangle select with a zero-size box selects the point under it | `tables::zero_size_box_selects_the_point_under_it` (no longer ignored) |
| B-T6 | Low | **Fixed** —  Identify on a projected raster errors instead of returning "no value" for a point outside the CRS's domain | `rasters::identify_far_from_a_projected_raster_is_just_empty` |
| B-T8 | Low (latent) | **Fixed** —  `Task::part(i, n)` with `i >= n` reports a progress fraction above 100 % | `progress_sources::a_part_index_past_the_end_still_reports_at_most_100_percent` |

### B-T5 — Warp to an invalid CRS crashes the app (High) — FIXED
Fix: every `*AppOptionsNew` result is null-checked in `raster.rs` (`translate`, `warp`, `dem`, rasterize); Warp also validates the target with `crs::validate`.
- **Reproduce:** add any raster layer; in its node tree add *Warp (Reproject Raster)*, set *Target coordinate system* to `EPSG:99999999` (or any text PROJ cannot read, or empty) and run. Test: `cargo test -p engine --test rasters -- --ignored warp_to_an_invalid_crs`.
- **Expected:** the Warp step shows an error ("Unknown or invalid coordinate system …") and the app keeps running.
- **Actual:** GDAL prints `Invalid SRS for -t_srs` and the process dies with SIGSEGV.
- **Suspect:** `crates/engine/src/raster.rs::warp` (and the same pattern in `translate`, `dem`): `GDALWarpAppOptionsNew` returns NULL for a bad option and the code then calls `GDALWarpAppOptionsSetProgress(opts, …)` on the NULL pointer. Check `opts.is_null()` first and return `last_error(…)`; also validate the CRS with `crs::validate` in `qgis_raster.rs::Warp::build`.

### B-T7 — Warp of a raster with no CRS invents one (High) — FIXED
Fix: `need_raster_crs` (nodes/mod.rs) in Warp, Clip Raster by Extent, Sample Raster Values and Zonal Statistics. Contours, Polygonize and the terrain tools do not reproject and carry the (absent) CRS through unchanged.
- **Reproduce:** import a GeoTIFF that has no CRS (the layer shows no coordinate system), run *Warp* to `EPSG:4326`. Test: `--test rasters -- --ignored warping_a_raster_of_unknown_crs`.
- **Expected:** an error telling the user to Define Projection first (as Buffer, Project, Near and Spatial Join do for vectors).
- **Actual:** the step succeeds; the output keeps the original grid (x = 600000 …, pixel 30) but is labelled `EPSG:4326`, i.e. metres reinterpreted as degrees, with no warning. Breaks the "explicit CRS, never guess" rule.
- **Suspect:** `crates/engine/src/nodes/qgis_raster.rs::Warp::build` has no `need_crs` check (the vector tools use `need_crs`). Clip, Contours, Polygonize etc. should be checked for the same gap.

### B-T1 — Export onto the source file overwrites it (Medium) — FIXED
Fix: `nodes/export.rs::refuse_source_file` compares the canonicalized output path with every layer's `source` and raster path.
- **Reproduce:** import `a.gpkg` (2 features), set a definition query `id = 1`, add an Export node writing GPKG to the same `a.gpkg`, run. Test: `--test formats -- --ignored exporting_never_overwrites`.
- **Expected:** refuse ("this is the layer's source file") or at least ask; `CLAUDE.md` says never overwrite source data.
- **Actual:** the file is silently replaced by the exported (filtered) content; the original 2 features are gone.
- **Suspect:** `crates/engine/src/nodes/export.rs::build` never compares `path` with any layer's `source` (`LayerInfo.source`) or the existing file.

### B-T4 — Cancel is slow on single-call geometry work (Medium) — FIXED
Fix (`nodes/batching.rs`, `Output::Script`): Cancel now stops a 1.5 M-point geodesic Buffer and a 200 k-polygon Dissolve in ~0.1 s (was ~12 s). The cause of the Buffer delay was not the size of the call but PROJ setup: every row group held features from all over the layer, so every thread built hundreds of projections behind a lock. The layer is now processed sorted by its projection centre (≈2× faster too) and the row order restored afterwards. Dissolve (and Buffer with Dissolve) does partial unions per spatial tile, one statement each, then pairwise unions of neighbouring tiles. Layers up to 50 k (Buffer) / 20 k (Dissolve) features use the old single query.
- **Reproduce:** run `cargo test -p engine --test cancel -- --ignored cancel_geodesic_buffer cancel_dissolve` (1.5 M points geodesic Buffer; 200 k polygons Dissolve), or in the app generate that much test data and press Cancel 0.3 s after the run starts.
- **Expected:** stops in about a second (the planar Buffer on 3 M points does, and so do Near, Voronoi and Spatial Join).
- **Actual:** the step stops 11.8–12.5 s after Cancel; the progress bar shows "Cancelling…" meanwhile.
- **Suspect:** DuckDB interrupts only between work units; per-feature geodesic projection (`nodes/buffer.rs`) and the single `ST_Union_Agg` of Dissolve (`nodes/dissolve.rs`) are long uninterruptible calls. Same cause as `PROJECT_STATUS.md` known issue 1; the numbers are new. Chunk the work or run it on a separate connection that can be dropped.

### B-T2 — Blank layer name (Low) — FIXED
- **Reproduce:** `Engine::rename_layer(id, "   ")`; test `--test tables -- --ignored renaming_a_layer_to_blank`. In the app: rename a layer in Contents to spaces.
- **Expected:** refused, or the old name kept. **Actual:** the layer name is stored blank (an invisible row in Contents and in citations).
- **Suspect:** `crates/engine/src/lib.rs::rename_layer` does no validation.

### B-T3 — Zero-size selection box (Low) — FIXED by M3, test enabled
- **Reproduce:** `Engine::select_box(point_layer, [x, y, x, y])` where (x, y) is a point's location; test `--test tables -- --ignored zero_size_box`.
- **Expected:** the point is selected (a click with the rectangle tool is a zero-size box). **Actual:** empty selection.
- **Suspect:** `crates/engine/src/table.rs::select_box` builds `ST_MakeEnvelope(x0, y0, x0, y0)`, a degenerate polygon that `ST_Intersects` does not match. Pad the box by a tiny tolerance or use a point/line when width or height is 0. (The UI may never send such a box; confirm in `MapView.tsx` after M3.)

### B-T6 — Raster identify far from the raster errors (Low) — FIXED
- **Reproduce:** import a UTM 14N GeoTIFF; `Engine::raster_value(id, 0.0, 0.0)`; test `--test rasters -- --ignored identify_far`. In the app: Identify with the pointer on a point far from the raster's UTM zone.
- **Expected:** `Ok(None)`. **Actual:** `Err("Invalid coordinate range while transforming points … Point outside of projection domain")` (and GDAL prints `ERROR 1`).
- **Suspect:** `crates/engine/src/raster.rs::sample`: treat a PROJ domain error as "no value".

### B-T8 — Progress above 100 % (Low, latent) — FIXED
- **Reproduce:** `Task::start("x").part(3, 0); set(1.0)` (or `part(i, n)` with `i >= n`); test `--test progress_sources -- --ignored a_part_index_past_the_end`.
- **Expected:** fraction ≤ 1. **Actual:** 4.0. No caller does this today; `part` should clamp `i` to `n - 1`.
- **Suspect:** `crates/engine/src/progress.rs::Task::part`.

## Observations (not bugs, worth knowing)
- `NEAR_DIST` (Near tool) is in the units chosen for the search radius, not always metres. Covered by `tools::near_and_join_nearest_report_distance_and_respect_radius`.
- `tiger_import` and `tnm_import` check dataset ids one at a time: with `["tracts", "bogus"]` the first dataset is downloaded and added before the second id is rejected. The UI never sends an unknown id. Covered (id first) by `downloads::a_refused_download_adds_no_layers_citations_or_lingering_tasks`.
- `pyToSql` does not check that an expression is complete or that field names exist; the engine's SQL parser reports those errors. Covered by `queryEdge.test.ts`.
- The layout test binary has crashed once with SIGSEGV on macOS CI (`PROJECT_STATUS.md` known issue 4). While writing the raster tests the same signal was reproduced locally, but it was B-T5, not that race.

## For M3 (files owned by the map session)

Nothing found. While cycling the map CRS (EPSG:3857 → 32615 → 5070 → 4326 → 3395 → ortho/laea custom → 3031/3413 → invalid ids set straight into the store, with layers loaded and a layout tab open), `MapView.tsx` never threw: invalid ids show the status message "The map can't be drawn in …" and keep the last good CRS. Checked at commit time of this branch; re-run after M3 lands (`window.__gis.useApp.getState().setMapCrs({id, name})` in the browser build).

## Fixed on `claude/errors-crash-fixes-0cffd7`

| Crash | Cause | Fix |
|---|---|---|
| Layout tab → "Maximum update depth exceeded" after opening a project with 2+ layouts | `LayoutView` selected `s.id === layoutId ? s.selected : []`; a new `[]` per call makes zustand loop forever for every layout tab that isn't the open one | `selectionOf()` returns a shared empty array |
| Frame CRS / "Set from Map View" with no layout open threw `Cannot read … of null` | `useLayouts.getState().doc!` | `frameById()` returns null; the user gets a message |
| Scale 0/NaN, w/h ≤ 0, NaN centre written into a layout | store `update` took any patch | `cleanPatch()` drops invalid values; `setPage` refuses 0/NaN pages |
| Older or damaged layout (no `page`, no `center`, null elements) | editor read fields directly | `normalizeLayout()` on load; `FrameFields` tolerates a missing centre |
| Page size 0 → fit zoom `Infinity` | `fitZoom` unguarded | non-finite → default zoom |
| Invalid map CRS accepted silently | `pickMapCrs` set it without checking | `setMapCrsChecked()` validates with the engine first |
| Damaged saved camera / dock layout aborted project open | uncaught in `restoreState` | wrapped; status message |
| Export with no map frame | engine error only in the status bar | explained up front |

## Found by e2e

Found 2026-10-25 while automating `docs/TEST_PLAN.md` §14 (`e2e/*.spec.ts`). Each had a `test.fixme("… BUG …")` in the spec; both are fixed and enabled (`MapNotices` keys by a stable id; `Engine::import` says "File not found").

| ID | Sev. | Summary | Test |
|---|---|---|---|
| E2E-1 | Low | **Fixed** —  Applying a definition query logs React's "two children with the same key" error: `MapNotices` uses the notice text (`Projecting <layer name>…`) as the key, and two projections of one layer (before and after the query) are listed at once. Can duplicate or drop a notice line | `table.spec.ts` "M-12 BUG E2E-1" (the real M-12 test filters this one message) |
| E2E-2 | Low | **Fixed** —  Adding a path that does not exist reports "Could not read /path: Query returned no rows" instead of saying the file is missing | `misc.spec.ts` "M-1 BUG E2E-2" |

| U-5 | High | **Fixed (Q3, 2026-10-28)** — Layout editor: the Properties pane went blank after clicking into it and editing a value; edits were dropped until the element was reselected. **Cause (confirmed):** `App.tsx` called `useLayouts.open(null)` whenever the dock's active panel was not a layout tab. Clicking into the Properties pane makes it the active panel, so the open layout was closed: `doc` became `null`, `selected` was cleared (the layout's effect then set Properties to `null` = blank hint), and `update()` returned early with no document (edit silently dropped). Reselecting an element re-activated the layout, which is why it worked again. Hypotheses H1–H7 were not the cause; the cause was a new one (H8). **Fix:** non-layout panels leave the layout open (`followActivePanel`), closing a layout tab closes the layout (`layoutPanelClosed`), and a layout tab clears only its own Properties target. Tracer: `?trace=layout` (LP-0). | `e2e/layout-props.spec.ts` LP-5 (fails 1.0 m timeout without the fix, passes with it); `store/layout.test.ts` LP-1, LP-3 |
| D-1 | High | Downloading 3DEP elevation for a large area fails or causes problems (one request for the whole box; service limits unknown; silent coarsening; no tiling, retry or resume). **Guard added (Q3, 2026-10-28); tiled download still open (D0).** The guard (`tnm::plan_elevation`) sizes the download on the ellipsoid (D-H6), tells the user when the cell size was coarsened (D-H4) and refuses, in plain words and before any request, boxes wider than 6° / beyond 84° latitude (D-H3) or needing cells coarser than 90 m. Service limits (D-H1, D-H2, D-H5) are not measured yet: DEM-0/DEM-1 need the network and belong to D0. Hypotheses D-H1…D-H6 and cases DEM-0…DEM-13 in `docs/TEST_PLAN.md` §10A. Guard in Q3, tiled download in D0. | — |

Not new: M-13's cancel on a large geodesic Buffer takes ~12 s (B-T4); the e2e test allows 30 s.

## Reported by the user

| ID | Sev. | Summary | Fix |
|---|---|---|---|
| U-1 | Medium | The layout editor didn't show the map's basemap: a new map frame always started with `basemap: "none"`, and "Set from Map View" didn't copy it | Fixed 2026-10-25: new frames and "Set from Map View" take the map view's basemap (`printBasemap`; OpenStreetMap prints as none, with a message, because its tile policy forbids it). Test: `layout.test.ts` "map frame basemap follows the map view". |
| U-2 | Medium | Importing TIGER tracts sometimes failed with "TIGERweb returned something that isn't JSON" | Fixed 2026-10-25. A tract page was one 16–22 MB response (13–17 s), and a cut-off response or a busy-server HTML page failed to parse, with no detail. Now each request retries up to 3 times, a page that keeps failing halves (1000 → 500 → 250 features), and the error names what came back (a web page and its title, a response that ended early, or a snippet). Test: `tiger::tests::bad_responses_are_described_not_just_not_json`; live tests `tiger_sample_data_downloads` and `tiger_year_list_and_availability` pass. |
| U-3 | High | National Map import (federal lands, clipped) failed with "TopologyException: side location conflict at -119.1866 38.0327": USGS sent invalid polygons and the clip's `ST_Intersection` threw, aborting the whole import | Fixed 2026-10-25: `clip_layer_to` repairs invalid features first (`ST_MakeValid`, same dimension), makes the clip shape valid, runs in one transaction, and a dataset whose clip still fails goes to the dialog's "failed" list with a readable message while the others import. Test: `clip_tests::clipping_repairs_invalid_polygons_instead_of_failing`; the reported area (federal lands, ~25,000 km² around the error point) now imports 58 features. |
| U-4 | High | Project → Open failed with "JSON.stringify cannot serialize cyclic structures" | Fixed 2026-10-25: the ribbon passed its click event into each command (`onClick={cmd.run}`). Since `openProject(path?)` gained a path parameter for the Catalog, the event was taken as the folder path and sent to the engine. The ribbon now calls `run()` with no arguments, covering every command, and `openProject` accepts only a string path. Test: `ui/src/openProject.test.ts`. |
| U-6 | Medium | **Open, found 2026-10-28 while checking L1.** After New Project (or opening another project) a layer that gets the same id and version as one in the previous project (`L1`, version 2) is treated as already projected in a non-Mercator map CRS, so its tiles fail with "hasn't been projected for display in this coordinate system yet" and the map stays blank for that layer. Cause: `MapView.tsx` `displayReady` (key `id\|version\|crsKey`) is never cleared when the project changes. Fix idea: clear it (and `building`) on new/open project, or include a project counter in the key. | to add: e2e New Project then add a layer in UTM |
| E2E-3 | Low | `misc.spec.ts` M-25 (progress UI, cancel) failed once in a full run on 2026-10-28 (waiting for the "cancelled" status, 60 s) and passed on the next two runs with no change to that code. Flaky; watch it. | `misc.spec.ts` M-25 |
| C-1 | High | **Open, found by R1 (2026-10-28).** DuckDB Spatial's `ST_Transform` picks a different datum operation than PROJ's best (PROJ 9.8.1 `cs2cs`): WGS 84 ↔ NAD83 (and ETRS89) is treated as no change (up to ~2 m off), and WGS 84/NAD83 → NAD27 differs from the grid result by ~2 m (e.g. 35.2 N, −97.5: engine NAD83 35.200084663 vs 35.200066504). Same-datum projections match to 1 mm. Fix: R3 (use a PROJ operation chosen per point, record its accuracy, say so in the result, check the grids in `--self-test`). | `crs_reference.rs` r_a6_…_across_datums, r_d1, r_d2, r_d3 (ignored) |
| C-2 | Medium | **Open, found by R1.** `ST_Transform` returns `(inf, inf)` with no error for points a projection cannot show (Lambert Conformal Conic of Texas Central at the south pole), so a non-finite number can reach a result. Fix: R4 (RD-6: count the point, never keep inf/NaN). | `crs_reference.rs` r_j1 (ignored) |
| C-3 | Medium | **Open, found by R1.** The geodesic area of a polygon spanning the whole world (edges on ±180° and ±90°) is measured as 0 instead of 510 065 621.7 km². Fix: R4 (measure with the edges handled, or refuse in words). | `crs_reference.rs` r_j8 (ignored) |
| C-4 | High | **Fixed (R2, 2026-10-28)** — found by `crs_import::r_e4`. A GeoPackage with no SRS (srs_id 0 or −1) was imported as an "Undefined geographic SRS" with an unknown datum and treated as a real CRS, so the map and tools used it without asking (breaks I1, never guess). It now stays undefined with the usual warning. | `crs_import.rs` r_e4 |
| C-5 | Low | **Fixed (R2)** — found by `crs_import::r_c3`. A CSV whose "latitude" column holds values beyond ±90 (columns swapped) imported silently with garbage positions. It now warns that latitude and longitude look swapped. | `crs_import.rs` r_c3 |
| G1 | High | **Closed (R2)** — CRS strings were compared as text (`geom_in`, Select By Location, result CRS names), so an EPSG CRS and its ESRI `.prj` twin counted as different and tools reprojected for nothing. All three now use `crs::same`; a guard test fails if a CRS string comparison reappears in the engine. | `crs_identity.rs` |

