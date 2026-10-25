# Known bugs

Bugs that are not yet fixed. Each one has a test that fails today and is marked `#[ignore = "BUG: …"]`; remove the `#[ignore]` when you fix the bug (run all of them with `cargo test -p engine -- --ignored <test name>`).
Severity: **High** = crash, data loss or a wrong result without a warning · **Medium** = wrong behaviour with a workaround · **Low** = rough edge.
Known performance and platform issues that are not bugs in the code live in `PROJECT_STATUS.md` ("Known issues").

## Found by tests

Found 2026-10-25 while writing `docs/TEST_PLAN.md`'s tests. Nothing here was fixed by that work.

| ID | Sev. | Summary | Test |
|---|---|---|---|
| B-T5 | High | Warp to an invalid CRS crashes the whole process (SIGSEGV) | `rasters::warp_to_an_invalid_crs_is_an_error_not_a_crash` |
| B-T7 | High | Warp of a raster with no CRS "succeeds" and labels the untouched grid with the target CRS | `rasters::warping_a_raster_of_unknown_crs_is_refused_not_guessed` |
| B-T1 | Medium | Export can overwrite the layer's own source file | `formats::exporting_never_overwrites_the_layers_own_source_file` |
| B-T4 | Medium | Cancel takes ~12 s to stop a geodesic Buffer (1.5 M points) or a Dissolve (200 k polygons) | `cancel::cancel_geodesic_buffer`, `cancel::cancel_dissolve` |
| B-T2 | Low | A layer can be renamed to a blank name | `tables::renaming_a_layer_to_blank_is_refused` |
| B-T3 | Low | Rectangle select with a zero-size box selects nothing, even the point under it | `tables::zero_size_box_selects_the_point_under_it` |
| B-T6 | Low | Identify on a projected raster errors instead of returning "no value" for a point outside the CRS's domain | `rasters::identify_far_from_a_projected_raster_is_just_empty` |
| B-T8 | Low (latent) | `Task::part(i, n)` with `i >= n` reports a progress fraction above 100 % | `progress_sources::a_part_index_past_the_end_still_reports_at_most_100_percent` |

### B-T5 — Warp to an invalid CRS crashes the app (High)
- **Reproduce:** add any raster layer; in its node tree add *Warp (Reproject Raster)*, set *Target coordinate system* to `EPSG:99999999` (or any text PROJ cannot read, or empty) and run. Test: `cargo test -p engine --test rasters -- --ignored warp_to_an_invalid_crs`.
- **Expected:** the Warp step shows an error ("Unknown or invalid coordinate system …") and the app keeps running.
- **Actual:** GDAL prints `Invalid SRS for -t_srs` and the process dies with SIGSEGV.
- **Suspect:** `crates/engine/src/raster.rs::warp` (and the same pattern in `translate`, `dem`): `GDALWarpAppOptionsNew` returns NULL for a bad option and the code then calls `GDALWarpAppOptionsSetProgress(opts, …)` on the NULL pointer. Check `opts.is_null()` first and return `last_error(…)`; also validate the CRS with `crs::validate` in `qgis_raster.rs::Warp::build`.

### B-T7 — Warp of a raster with no CRS invents one (High)
- **Reproduce:** import a GeoTIFF that has no CRS (the layer shows no coordinate system), run *Warp* to `EPSG:4326`. Test: `--test rasters -- --ignored warping_a_raster_of_unknown_crs`.
- **Expected:** an error telling the user to Define Projection first (as Buffer, Project, Near and Spatial Join do for vectors).
- **Actual:** the step succeeds; the output keeps the original grid (x = 600000 …, pixel 30) but is labelled `EPSG:4326`, i.e. metres reinterpreted as degrees, with no warning. Breaks the "explicit CRS, never guess" rule.
- **Suspect:** `crates/engine/src/nodes/qgis_raster.rs::Warp::build` has no `need_crs` check (the vector tools use `need_crs`). Clip, Contours, Polygonize etc. should be checked for the same gap.

### B-T1 — Export onto the source file overwrites it (Medium)
- **Reproduce:** import `a.gpkg` (2 features), set a definition query `id = 1`, add an Export node writing GPKG to the same `a.gpkg`, run. Test: `--test formats -- --ignored exporting_never_overwrites`.
- **Expected:** refuse ("this is the layer's source file") or at least ask; `CLAUDE.md` says never overwrite source data.
- **Actual:** the file is silently replaced by the exported (filtered) content; the original 2 features are gone.
- **Suspect:** `crates/engine/src/nodes/export.rs::build` never compares `path` with any layer's `source` (`LayerInfo.source`) or the existing file.

### B-T4 — Cancel is slow on single-call geometry work (Medium)
- **Reproduce:** run `cargo test -p engine --test cancel -- --ignored cancel_geodesic_buffer cancel_dissolve` (1.5 M points geodesic Buffer; 200 k polygons Dissolve), or in the app generate that much test data and press Cancel 0.3 s after the run starts.
- **Expected:** stops in about a second (the planar Buffer on 3 M points does, and so do Near, Voronoi and Spatial Join).
- **Actual:** the step stops 11.8–12.5 s after Cancel; the progress bar shows "Cancelling…" meanwhile.
- **Suspect:** DuckDB interrupts only between work units; per-feature geodesic projection (`nodes/buffer.rs`) and the single `ST_Union_Agg` of Dissolve (`nodes/dissolve.rs`) are long uninterruptible calls. Same cause as `PROJECT_STATUS.md` known issue 1; the numbers are new. Chunk the work or run it on a separate connection that can be dropped.

### B-T2 — Blank layer name (Low)
- **Reproduce:** `Engine::rename_layer(id, "   ")`; test `--test tables -- --ignored renaming_a_layer_to_blank`. In the app: rename a layer in Contents to spaces.
- **Expected:** refused, or the old name kept. **Actual:** the layer name is stored blank (an invisible row in Contents and in citations).
- **Suspect:** `crates/engine/src/lib.rs::rename_layer` does no validation.

### B-T3 — Zero-size selection box (Low)
- **Reproduce:** `Engine::select_box(point_layer, [x, y, x, y])` where (x, y) is a point's location; test `--test tables -- --ignored zero_size_box`.
- **Expected:** the point is selected (a click with the rectangle tool is a zero-size box). **Actual:** empty selection.
- **Suspect:** `crates/engine/src/table.rs::select_box` builds `ST_MakeEnvelope(x0, y0, x0, y0)`, a degenerate polygon that `ST_Intersects` does not match. Pad the box by a tiny tolerance or use a point/line when width or height is 0. (The UI may never send such a box; confirm in `MapView.tsx` after M3.)

### B-T6 — Raster identify far from the raster errors (Low)
- **Reproduce:** import a UTM 14N GeoTIFF; `Engine::raster_value(id, 0.0, 0.0)`; test `--test rasters -- --ignored identify_far`. In the app: Identify with the pointer on a point far from the raster's UTM zone.
- **Expected:** `Ok(None)`. **Actual:** `Err("Invalid coordinate range while transforming points … Point outside of projection domain")` (and GDAL prints `ERROR 1`).
- **Suspect:** `crates/engine/src/raster.rs::sample`: treat a PROJ domain error as "no value".

### B-T8 — Progress above 100 % (Low, latent)
- **Reproduce:** `Task::start("x").part(3, 0); set(1.0)` (or `part(i, n)` with `i >= n`); test `--test progress_sources -- --ignored a_part_index_past_the_end`.
- **Expected:** fraction ≤ 1. **Actual:** 4.0. No caller does this today; `part` should clamp `i` to `n - 1`.
- **Suspect:** `crates/engine/src/progress.rs::Task::part`.

## Observations (not bugs, worth knowing)
- `NEAR_DIST` (Near tool) is in the units chosen for the search radius, not always metres. Covered by `tools::near_and_join_nearest_report_distance_and_respect_radius`.
- `tiger_import` and `tnm_import` check dataset ids one at a time: with `["tracts", "bogus"]` the first dataset is downloaded and added before the second id is rejected. The UI never sends an unknown id. Covered (id first) by `downloads::a_refused_download_adds_no_layers_citations_or_lingering_tasks`.
- `pyToSql` does not check that an expression is complete or that field names exist; the engine's SQL parser reports those errors. Covered by `queryEdge.test.ts`.
- The layout test binary has crashed once with SIGSEGV on macOS CI (`PROJECT_STATUS.md` known issue 4). While writing the raster tests the same signal was reproduced locally, but it was B-T5, not that race.
