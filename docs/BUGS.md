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
| B-T4 | Medium | Cancel takes ~12 s to stop a geodesic Buffer (1.5 M points) or a Dissolve (200 k polygons) | `cancel::cancel_geodesic_buffer`, `cancel::cancel_dissolve` |
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

### B-T4 — Cancel is slow on single-call geometry work (Medium)
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

Found 2026-10-25 while automating `docs/TEST_PLAN.md` §14 (`e2e/*.spec.ts`). Each has a `test.fixme("… BUG …")` in the spec; remove the `fixme` when fixed.

| ID | Sev. | Summary | Test |
|---|---|---|---|
| E2E-1 | Low | Applying a definition query logs React's "two children with the same key" error: `MapNotices` uses the notice text (`Projecting <layer name>…`) as the key, and two projections of one layer (before and after the query) are listed at once. Can duplicate or drop a notice line | `table.spec.ts` "M-12 BUG E2E-1" (the real M-12 test filters this one message) |
| E2E-2 | Low | Adding a path that does not exist reports "Could not read /path: Query returned no rows" instead of saying the file is missing | `misc.spec.ts` "M-1 BUG E2E-2" |

Not new: M-13's cancel on a large geodesic Buffer takes ~12 s (B-T4); the e2e test allows 30 s.
