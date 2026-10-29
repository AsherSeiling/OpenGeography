# Test plan

One row per case for every feature in `docs/ROADMAP.md`. Last updated 2026-10-27 (macOS arm64; §2A added, not yet run).

**Full plan:** the edge-case, failure-path and error-reporting plan for every feature is in [`docs/TEST_PLAN_FULL.md`](TEST_PLAN_FULL.md) (cases in `docs/test-plan/`); it references the IDs below.

**Coverage column**
- `auto: <file>::<test>` — an automated test covers it. Rust files are `crates/engine/tests/<file>.rs` (`unit:` = a `#[cfg(test)]` module in `crates/engine/src/`). TypeScript files are `ui/src/**/<file>.test.ts` with the `describe` name after `>`.
- `auto (ignored): …` — automated, but not run by default (`network` = needs internet, `perf` = slow, `BUG` = fails today, see `docs/BUGS.md`). Run them with `cargo test -p engine --test <file> -- --ignored <name>`.
- `manual M-n` — needs a person; steps are in §14.
- `gap` — nothing covers it yet.

**Run everything** (one command; the output is one summary in `test-results/<time>/summary.md`):

| Sequence | Command | When | Time (M-series Mac) |
|---|---|---|---|
| 1. Static + UI + engine (what CI runs) | `scripts/run-test-plan.sh --quick` | every change | ~6 min cold, ~1 min warm |
| 2. + every ignored engine test, one by one (bug, network, perf) | `scripts/run-test-plan.sh` (`--no-network` offline) | before a merge batch | +10–15 min (perf runs in release) |
| 3. + browser end-to-end (manual cases a script can do) | `scripts/run-test-plan.sh --e2e` | before a release | +2 min |
| 4. + packaged app and `--self-test` | add `--package` | once per phase | +6 min |
| 5. Manual cases that need a person | the checklist at the end of the summary | before a release tag | — |

A bug test that starts passing means the bug is fixed: unignore it and update `docs/BUGS.md`. One test at a time: `cargo test -p engine --test <file> -- --ignored --exact <name>`.

**Standard checks for a tool** (§7 applies them to each): output field names, a known value, the CRS of the result equals the first input's CRS, an input without a CRS is refused with a readable message (not guessed), bad settings give an error message without internal table names, and a layer in another CRS is reprojected and flagged (`reprojected`).

---

## 1. Import / export

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| 1.1 | Each vector format imports with geometry type, count, CRS, extent, fields and no warnings | GeoJSON, GPKG, shapefile+prj, FlatGeobuf, 2 polygons in EPSG:4326 | `Polygon`, 2, `EPSG:4326`, extent −98…−97.8 | auto: formats::each_vector_format_imports_with_crs_geometry_and_citation |
| 1.2 | CRS read from file | GPKG in EPSG:26914 | `EPSG:26914`, extent in lon/lat | auto: engine::imports_crs_from_file |
| 1.3 | Shapefile without .prj | `.shp` only | `crs = None`, warning, nothing drawn, suggestions offered, Define Projection clears the warning | auto: engine::shapefile_without_prj_is_flagged_not_guessed |
| 1.4 | Multi-layer GPKG | 2 layers (point, line) | 2 layers, right geometry types, one citation used twice | auto: formats::multi_layer_geopackage_gives_one_layer_each_and_mixed_geometry_is_labelled |
| 1.5 | Mixed geometry types | GeoJSON with a point and a line | `Mixed`, 2 features | auto: formats::multi_layer_geopackage_gives_one_layer_each_and_mixed_geometry_is_labelled |
| 1.6 | CSV with lat/lon columns | `Latitude,Longitude` | Points, `EPSG:4326`, warning that WGS 84 was assumed | auto: formats::csv_with_coordinates_becomes_points_and_says_which_datum_it_assumed |
| 1.7 | CSV with x/y only | `x,y` | Points, CRS undefined, warning | same |
| 1.8 | CSV without coordinates; TSV | `name,count` | `Table`, no CRS, no extent; TSV with lat/lon gives points | same |
| 1.9 | GeoParquet round trip | export then import | geometry, 2 rows, CRS 26914 kept | auto: formats::geoparquet_round_trip_keeps_geometry_and_crs |
| 1.10 | Raster import (GeoTIFF) | 200×200 DEM | `Raster`, CRS, min/max, tile is PNG, identify gives value | auto: engine::rasters_contours_hillshade_slope; rasters::raster_layers_report_stats_and_draw_tiles |
| 1.11 | Unicode values and field names | `Ålesund – 東京 ✓`, field `nombre año` | read, edit and show unchanged | auto: formats::non_ascii_attribute_values_and_names_survive_import_and_editing |
| 1.12 | Null geometry features | GeoJSON with `geometry: null` | kept (2 features), extent ignores them | auto: formats::features_with_null_geometry_import_and_are_counted |
| 1.13 | Zero-feature file | empty FeatureCollection | empty layer or an explained error | auto: formats::zero_feature_files_import_as_empty_layers |
| 1.14 | Bad files | missing path, empty file, junk text, truncated JSON, junk .shp/.gpkg, `.docx` | `Err` each; no layer, no citation; engine still works | auto: formats::bad_files_are_reported_not_crashed_on |
| 1.15 | Corrupt raster | text saved as `.tif` | `Err` | auto: rasters::raster_layers_report_stats_and_draw_tiles |
| 1.16 | Every export format writes a readable file | GPKG, Shapefile, GeoJSON, FlatGeobuf, GeoParquet from a UTM layer | 2 features back, CRS kept (GeoJSON: some CRS), coordinates unmoved | auto: formats::every_export_format_writes_a_file_that_reads_back |
| 1.17 | CSV export | polygon layer | header has `geom`, WKT text, one line per feature | same |
| 1.18 | Export a table without geometry; unwritable folder | CSV table | file written; missing folder → Error with message | auto: formats::exporting_a_table_without_geometry_works_and_missing_folders_are_reported |
| 1.19 | Reserved `fid` column on export | layer with `fid` | written as `fid_src`, no failure | auto: engine::export_handles_gdal_reserved_id_columns_and_keeps_crs |
| 1.20 | Export only selected features | fids list on Layer Input | only those rows | auto: engine::definition_query_and_selected_only_export |
| 1.21 | Export never overwrites the layer's own source | export to the imported path | refused | auto (B-T1 fixed): formats::exporting_never_overwrites_the_layers_own_source_file |
| 1.22 | Add Data dialog: pick several files at once, a file with spaces/accents in the path, cancel the dialog | see M-1 | layers appear in the chosen order; cancel changes nothing | manual M-1 |
| 1.23 | Export Features dialog: format list, overwrite prompt, selected-only box | see M-2 | file opens in QGIS/ArcGIS with the same CRS | manual M-2 |
| 1.24 | Drag a file from Finder/Explorer onto the app | see M-3 | adds as layer(s) | manual M-3 |

## 2. Coordinate reference systems

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| 2.1 | Catalog (7,195 entries) usable in the engine | every picker code | PROJ accepts each | auto (ignored, perf): engine::every_catalog_crs_is_usable |
| 2.2 | Picker search, location filter, kind filter, State Plane lookup, UTM zone | `26914`, `epsg:26914`, point in Fiji across ±180°, `Atlantis` | exact code first; antimeridian handled; empty list for nonsense | auto: crs.test.ts > CRS catalog; crsEdge.test.ts > CRS search / UTM zone choice at the edges |
| 2.3 | UTM zone at limits | lon ±180, 0, −0.0001; equator; southern; NAD83 vs WGS 84 | zone 1/60/31/30; north at lat 0; `327xx` south | auto: crsEdge.test.ts; unit: crs::utm_zones |
| 2.4 | Layer without a CRS is never measured, drawn or reprojected | shapefile without .prj | area/length error mentions "coordinate system"; tiles empty; copy stays undefined | auto: crs_edge::layer_with_no_crs_refuses_measurement_and_projection_instead_of_guessing |
| 2.5 | Tools refuse a CRS-less layer | Buffer, Project, Near, Spatial Join | Error, message contains "coordinate system" | auto: tools::layer_without_crs_is_refused_not_guessed |
| 2.6 | Raster tools refuse a CRS-less raster | Warp | Error | auto (B-T7 fixed; Clip/Sample/Zonal too: rasters::raster_tools_that_compare_with_vectors_refuse_a_raster_of_unknown_crs): rasters::warping_a_raster_of_unknown_crs_is_refused_not_guessed |
| 2.7 | Define Projection | valid / empty / `EPSG:0` / `EPSG:99999999` / text / `+proj=bogus` | valid sets CRS and clears the warning; invalid → Err and the layer stays undefined | auto: crs_edge::define_projection_rejects_bad_definitions |
| 2.8 | Suggestions by coordinate range | degrees / UTM-like / State-Plane-like / tiny | WGS84…, `UTM`, `STATEPLANE`, none | auto: crs_edge::suggestions_follow_the_coordinate_ranges; unit: crs |
| 2.9 | Geographic vs projected agree | same 1 km² square in EPSG:4326 and UTM 14N | areas within 0.2 %; planar buffers within 1 % | auto: crs_edge::geographic_and_projected_copies_of_a_shape_measure_the_same |
| 2.10 | US survey feet are not metres | EPSG:2277 (ftUS) vs 32139 (m); 100 ft square; 100 usft buffer | x_ft = x_m / 0.3048006096; 10,000 sq ft = 929.03 m²; buffer ≈ 3.12·r² | auto: crs_edge::us_survey_feet_are_not_treated_as_metres |
| 2.11 | CRS with no area of use | bare PROJ string `+proj=aeqd …` | display info needs a fallback extent ("area of use" in message); with fallback `area_of_use_known = false`; transforms, area and k still correct | auto: crs_edge::crs_without_an_area_of_use_needs_a_fallback_extent_and_still_works |
| 2.12 | Geographic CRS grid, antimeridian, poles, NaN, identity | EPSG:4267; ±179.999°; (0,90); NaN | k = 1; finite coordinates; error or number, never a panic | auto: crs_edge::geographic_crs_grid_and_unit_edge_cases |
| 2.13 | Datum shift NAD27→NAD83; UTM round trip | known point | NADCON magnitude; < 1 mm round trip | auto: engine::nad27_to_nad83_shift_matches_nadcon_magnitude, utm_round_trip |
| 2.14 | Mixed CRS inputs in a tool | WGS 84 mask clipping a UTM layer | output in the first input's CRS; `reprojected` flag | auto: tools::mixed_crs_inputs_are_reprojected_and_flagged; engine::clip_keeps_only_inside_and_reprojects_clip_layer |
| 2.15 | Display grids per CRS (Mercator, UTM, polar, geographic), stable keys, bad code | EPSG:3857/26915/3031/4326/99999999 | square grids, origin top-left, key stable, error for bad code | auto: display::display_info_grids |
| 2.16 | Scale factor k | UTM central meridian | 0.9996 | auto: display::display_info_grids; unit: layout::geo p4_utm15_factors_match_proj, factors_match_proj_for_albers_and_state_plane_feet |
| 2.17 | UI scale ↔ resolution at limits | NaN, zero, feet CRS, k = 0 | inverse pairs; no throw | auto: scaleEdge.test.ts; map/crs.test.ts |
| 2.18 | Map CRS picker (dialog), map redraws, scale readout unchanged on switching CRS | see M-4 | ground scale within 0.5 % | manual M-4 |

## 2A. Coordinate systems — hardening cases (Phase R, `docs/CRS_PLAN.md`)

Written 2026-10-27; **all `planned` until their session runs** (R1–R7). They extend §2, which stays as is. "Zoo" = the 18 test CRSs in `CRS_PLAN.md` §4; tolerances are `CRS_PLAN.md` §3 RD-4 (1 mm projected, 1e-8° geographic, unless a row says otherwise). Expected values come from analytic facts, `cs2cs` output in `crates/engine/tests/fixtures/crs/reference.json`, or GeographicLib, never from the engine itself. A case that finds a bug stays as an `#[ignore]` test with a `C-n` row in `docs/BUGS.md` until fixed. Files: `crs_reference.rs`, `crs_matrix.rs` (Rust, `crates/engine/tests/`), `ui/src/**/crsZoo.test.ts`, `e2e/crs.spec.ts`.

### A. Transform accuracy

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-A1 | UTM central meridian | lat 0, lon −93 → EPSG:32615 | 500000.000, 0.000 | auto: crs_reference::r_a1 |
| R-A2 | Web Mercator half world; latitude limit | lat 0, lon 180 → 3857; lat 85.0511288, lon 180 | x = 20037508.343, y = 0; y = ±20037508.34 (±1 cm) | auto: crs_reference::r_a2 |
| R-A3 | Polar stereographic | lat −90 → 3031; lat −71, lon 0 → 3031; lat 90 → 3413 | (0, 0); (0, 2082760.109); (0, 0) | auto: crs_reference::r_a3 |
| R-A4 | Albers origin | lat 23, lon −96 → 5070 | 0.000, 0.000 | auto: crs_reference::r_a4 |
| R-A5 | Round trip over the zoo | 25 points per CRS (centre, corners and edges of its area of use) → every other zoo CRS → back | ≤ 1 mm / 1e-9° | auto: crs_reference::r_a5 (inside each CRS's area; 1 mm / 1e-9°, NAD27 pairs 3 mm / 1e-8°) |
| R-A6 | Generated reference set | ≥ 500 points × zoo pairs from `cs2cs` | within tolerance; test log prints the PROJ version | auto: crs_reference::r_a6_…_same_datum; across datums ignored, bug C-1 |
| R-A7 | Geodesic anchors | meridian from equator to pole; 1° of longitude on the equator; WGS 84 surface | 10 001 965.729 m (±0.01); 111 319.491 m (±0.001); 510 065 621.7 km² (±0.001 %) | auto: crs_reference::r_a7 (GeographicLib) |
| R-A8 | Other-body anchors (PROJ only) | `IAU_2015` Mars sphere, Moon sphere | 1° equatorial = 59 274.70 m / 30 323.35 m; engine geodesics land with F1 | auto (PROJ level): crs_reference::r_a8 (Mars and Moon spheres by PROJ string; Earth → Mars refused); engine geodesics in F1 |
| R-A9 | `transform_point` command parity | 20 zoo points through Tauri-shaped call, devserver route and `api.transformPoint` | identical to R-A6; bad CRS → readable error, no panic | engine path covered by every crs_reference test (transform_point); devserver route and vitest in R6 |

### B. Identity and parsing

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-B1 | Equivalent spellings are one CRS | `EPSG:4326`, ` epsg:4326 `, `urn:ogc:def:crs:EPSG::4326`, WKT1, WKT2, PROJJSON, `+proj=longlat +datum=WGS84 +no_defs`, `OGC:CRS84`; the same for EPSG:26915 | equal `crs::key`; no `reprojected` flag when mixed in a tool; coordinates untouched | auto: crs_identity::r_b1 |
| R-B2 | Different CRSs are never equal | 4326/4269, 4267/4269, 26915/32615, 2277/32139, 2277/international-foot twin, 3857/3395 | keys differ; mixing reprojects and flags | auto: crs_identity::r_b2 |
| R-B3 | ESRI flavour recognised | `.prj` of `NAD_1983_UTM_Zone_15N` | key equals EPSG:26915's; the layer shows the EPSG code | auto: crs_identity::r_b3 |
| R-B4 | CRS label everywhere | zoo CRSs, WKT-only CRS | name, authority code if known, linear/angular unit, datum, area of use; "Custom" for a bare PROJ string | auto: crs_identity::crs_info_describes_codes_and_custom_definitions (code, unit, datum, area, custom string, error) |
| R-B5 | Bad definitions | empty, spaces, `EPSG:0`, `EPSG:99999999`, `+proj=bogus`, truncated WKT, 100 kB text, `EPSG:4326'); DROP TABLE x;--` | `Err` with a readable message; no panic; no SQL executed; layer unchanged | auto: crs_identity::r_b5 (the 100 kB definition is cut to 80 characters in the message) |
| R-B6 | Engineering / local CRS | `LOCAL_CS["site"]`, WKT with `ENGCRS` | treated as undefined (I1) with a message; never as metres on Earth | planned R2 |
| R-B7 | Key stability | same PROJ string with parameters in another order; two runs; Windows/Linux | same key | auto: crs_identity::r_b7 |
| R-B8 | Datum ensemble | WGS 84 realisations (G1150, G1762) vs EPSG:4326 | same key (RD-2 default); mixing does not reproject | auto: crs_identity::r_b8 (a custom +ellps/+towgs84 string stays "different": safe direction) |
| R-B9 | Cross-body guard | Earth layer + `IAU_2015:49900` | PROJ's "ellipsoid does not belong to the same celestial body" surfaced as a sentence; no result | planned R2 |

### C. Axis order and units

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-C1 | East-first everywhere | one point imported from GPKG, shapefile, GeoJSON, FlatGeobuf, GeoParquet, CSV in EPSG:4326, 4269, 3035 (north/east), 2193 (north/east) | stored x = east/lon, y = north/lat; extent, map position and `transform_point` match `cs2cs` | auto: crs_reference::r_c1 (GPKG, shapefile, GeoJSON, FlatGeobuf × 4326, 4269, 3035, 2193); GeoParquet and CSV to add in R2 |
| R-C2 | GeoJSON order | RFC 7946 file; legacy file with `crs` member `urn:ogc:def:crs:EPSG::4326` and lon,lat values; legacy `EPSG:3035` member | lon,lat read as lon,lat; projected members honoured; none flipped | auto: crs_import::r_c2 |
| R-C3 | CSV columns | `Longitude,Latitude`; `Latitude,Longitude`; values with lat > 90 in the "latitude" column | correct either way; swapped values warn "looks like latitude and longitude are swapped" | auto: crs_import::r_c3 (swapped columns now warn, C-5) |
| R-C4 | Linear units | metre, ftUS (2277) and international-foot CRS: 1000-unit square; length, area, planar buffer, scale bar | ftUS = 1200/3937 m, international = 0.3048 m; the two differ by 2 ppm and tests tell them apart (1 000 000 ft → 2 ft) | auto: crs_reference::r_c4 |
| R-C5 | Angular units | geographic CRS in grads / arc-seconds | map refuses with a message (existing 2.12); tools convert, or refuse with a message | planned R4 (tool matrix refuses or converts) |
| R-C6 | Prime meridian | EPSG:4807 NTF (Paris), grads: lon 0 grad | lon 2.337229167° E in 4326 (verify with `cs2cs` when pinning) | auto: crs_reference::r_c6 (pinned from cs2cs; the NTF datum shift is included) |
| R-C7 | Longitude outside ±180 | points at lon 190 and −190, then 540 | import warns; Project to UTM zone 1 treats 190 as −170; extent and tiles equal those of −170 | auto: crs_reference::r_c7 |

### D. Datum shifts and grids

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-D1 | NAD27 → NAD83 uses the grid | 35.2 N, −97.5 E | 35.200066504, −97.500312182 (≈ 7.4 m N, 28.4 m W) | auto: crs_reference::r_d1 (5 cm; the NADCON operations differ by 1.3 cm) |
| R-D2 | WGS 84 → NAD27 | same point | 35.199935272, −97.499683431 | auto: crs_reference::r_d2 |
| R-D3 | WGS 84 → NAD83 operation | same point | 35.200001669, −97.499995543 (the 2.0 m Oklahoma operation) within 5 mm | auto: crs_reference::r_d3 |
| R-D4 | Missing grid is detected | a subprocess with `PROJ_DATA` pointing at a folder without grids; NAD27 → NAD83 | `Err` or a result flagged "ballpark: may be off by up to ~100 m"; never a silent number | auto: crs_reference::r_d4 (child process with an empty PROJ_DATA; the report says so in words) |
| R-D5 | Accuracy is reported | NAD27 → NAD83 (CONUS); WGS 84 → NAD83; ballpark case | record `{from, to, operation, accuracyM}`: ≈ 0.15 m; 2.0 or 4.0 m by area; `"ballpark"` | partial: crs_tools::r_d5 (a datum change is named in the tool result); per-operation accuracy needs the PROJ API, not available through GDAL (decision R3) |
| R-D6 | Same-datum projection is exact | 4269 ↔ 26915, 4326 ↔ 32615 | accuracy `"exact"`; round trip ≤ 1 mm | auto: crs_reference::r_d6 |
| R-D7 | One operation per point | a layer straddling two operation areas (e.g. Oklahoma and Maine) | every vertex equals its own `cs2cs` answer | auto: crs_reference::r_d7 (each point equals its own cs2cs answer within the datum tolerance) |
| R-D8 | Vertical / Z policy | compound CRS layer, Z values, through Project and Buffer | Z unchanged, the result says "vertical datum not transformed"; M dropped with a message (RD-8) | auto: crs_reference::r_d8 (Z and M are carried through unchanged: RD-8 changed in R3) |

### E. Import, export, project round trip

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-E1 | Format × CRS round trip | export then import in GPKG, shapefile, FlatGeobuf, GeoParquet (not GeoJSON/CSV) × EPSG:4326, 26915, 5070, 2277, 3031, a PROJ string | same key; coordinates ≤ 1 mm / 1e-9° | auto: crs_import::r_e1 (GPKG, shapefile, FlatGeobuf × 6 CRSs; GeoParquet to add) |
| R-E2 | Shapefile `.prj` flavours | ESRI WKT; OGC WKT; custom WKT; garbled `.prj`; no `.prj` | recognised and keyed; garbled → undefined with a warning; none → undefined (existing 1.3) | auto: crs_import::r_e2 |
| R-E3 | GeoJSON export policy | UTM layer → GeoJSON | default: reprojected to 4326 with a message (RD-7); option "keep layer CRS" writes the legacy member; round trip ≤ 1 mm | auto: crs_export::r_e3 (GeoJSON reprojected to WGS 84 with a message; option keeps the layer CRS and writes the crs member) |
| R-E4 | GPKG SRS variants | `srs_id` 4326; custom definition without authority; 0 / −1 (undefined); two layers in different CRSs | custom kept as its WKT; 0/−1 → undefined with a warning (not 4326); each layer its own CRS | auto: crs_import::r_e4 (undefined stays undefined, C-4) |
| R-E5 | Project save / reopen | project with layers, map CRS and layout frames in 5 zoo CRSs; reopen; committed fixture project from another OS | same keys, same display, no re-reprojection | planned R2; R7 for other OS |
| R-E6 | CSV with X/Y | `x,y` in UTM; user picks the CRS at import | points land at `cs2cs` positions; no CRS chosen → undefined | planned R6 |
| R-E7 | Raster import flavours | GeoTIFF with EPSG code; WKT only; modified definition with an EPSG code; world file + `.prj`; COG in 3857; rotated geotransform | key equals the file's real CRS (modified definition kept); rotated/sheared → handled or refused with a message | planned R5 (risk G6) |
| R-E8 | Raster export keeps CRS | Warp output exported | CRS equal; pixel size and origin unchanged | planned R5 |

### F. Tools over the zoo (metamorphic; `crs_matrix.rs`)

For each case the fixture shapes (points, a 2 km line, a 1 km² polygon, a polygon with a hole, a multipart) are stored in each zoo CRS and run through the tool; the result is converted to EPSG:4326 and compared with the run in the reference CRS (EPSG:5070 for areas, geodesic for distances).

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-F1 | Guard: no tool skips CRS testing | every tool in `tool_specs` | each is in the matrix or in `CRS_EXEMPT` with a written reason; a new tool without either fails CI | auto: crs_matrix::every_registered_tool_is_in_the_matrix_or_excluded_with_a_reason |
| R-F2 | Measurements | Add Area / Length / Perimeter, geodesic, shapes in 12 CRSs | equal across CRSs (length 1e-5, area 1e-4) and equal to GeographicLib | auto: crs_matrix (calculate_geometry area and length, total geodesic measures equal across 8 CRSs: 1e-4 / 1e-4); GeographicLib comparison for lines in crs_reference r_j2 |
| R-F3 | Geodesic buffer | point and line, r = 10 km and 100 km, at lat 0, 45, 80, −80 in 8 CRSs | every vertex is r ± 0.1 % from the source (GeographicLib); result in the layer's CRS; error bound documented for NAD83/NAD27 data (G3) | planned R4 |
| R-F4 | Near, join by nearest, summarize | pairs of layers in two CRSs | distances equal GeographicLib; nearest is the same feature in every CRS | auto: crs_matrix (near, join_nearest, summarize_within, spatial_join, select_by_location) |
| R-F5 | Overlay with mixed CRSs | Clip, Intersect, Union, Difference, Symmetrical Difference, Erase: input A in CRS x, B in CRS y | result CRS = A's; `reprojected` true exactly when keys differ; area equals the same run in 5070 within 0.01 % (+ curved-edge allowance, written in the test) | planned R4 (extends 2.14) |
| R-F6 | Distance parameters | Simplify, Densify, Smooth, Grid, Hex, Points along lines, Buffer (planar) in 4326 and in 2277 | distances are in the unit the spec says (RD-9); a "CRS units" parameter on a geographic layer is refused with a message | planned R4 (risk G10) |
| R-F7 | Project tool | to each zoo CRS and back | geometry ≤ 1 mm; features outside the target's area of use counted and named in the message | partial: crs_tools::r_j1 (poles counted and left out, message); crs_reference r_a5 round trips; area-of-use naming in R6 |
| R-F8 | Select / join by location | layers in different CRSs | same feature set in every CRS pair (planar predicate in the first input's CRS; distance geodesic) | planned R4 |
| R-F9 | Raster × vector tools | Zonal statistics, Clip Raster, Sample: zones in a different CRS than the raster | equal to the run with aligned CRSs (edge cells aside, stated in the test) | planned R4/R5 |
| R-F10 | Result CRS rule | every tool | output CRS = first input's unless the tool documents otherwise; enforced by the matrix | auto: crs_matrix (the result keeps the first input's CRS for 33 recipes) |
| R-F11 | Dropped features reported | out-of-domain points (lat 89.9° in 3857, > 90° in a bad field, lon far from a Lambert origin) through each tool that transforms | a `dropped` count and a message; no silent empty geometry (RD-6) | partial: crs_tools::r_j1 (Project; C-2 fixed); other transforming tools use geom_in, which now drops non-finite results |
| R-F12 | Cache follows the CRS | Define Projection on a layer after a model run | downstream nodes re-run (no stale cache hit); attribute history shows the change | planned R4 (risk G12) |

### G. Rasters

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-G1 | Raster CRS recognised | R-E7 files | `crs_of` returns a PROJ-valid string whose key equals the file's | auto: crs_raster::r_g1 (modified definition kept, C-7) |
| R-G2 | Warp across the zoo | 100×100 plane `z = a·x + b·y` in UTM 15N → 4326, 5070, 3031, 2277 | 20 sample points equal the analytic plane within RD-4 | auto: crs_raster::r_g2 (UTM 15N plane → 4326, 3857, 5070, 32615, 25 points each) |
| R-G3 | Warp geometry | same raster | corners round-trip within 1 pixel; target resolution given in the target's unit (m, ° or ftUS) | partial: crs_raster::r_g2 (values); corner round trip and target-unit resolution to add |
| R-G4 | Identify / sample from another CRS | point given in 4326 on a UTM raster, and the reverse | value equals the native lookup; point outside the domain = no value (B-T6) | auto: crs_raster::r_g4 (+ P18 at the pole: crs_raster::p18) |
| R-G5 | Terrain in a geographic CRS | analytic 30° slope in a 4326 DEM | slope 30° ± 0.5°; aspect correct; hillshade matches the projected run | auto: crs_raster::r_g5 (30° north- and east-facing planes at 45° N, aspect; G11 fixed) |
| R-G6 | Contours / polygonize | DEM in 4326 and in 2277 | CRS carried; interval in elevation units; vertical unit stated as unknown | to automate (contours/polygonize CRS carry) |
| R-G7 | Raster and vector line up | raster of a known rectangle and a vector of the same rectangle, same CRS, `render_extent` in the zoo | edges within 1 px | to automate with M4 (render_extent alignment) |
| R-G8 | Clip raster by vector in another CRS | polygon in 4326, raster in UTM | extent = polygon's transformed bounds, ≤ 1 pixel | to automate (clip raster by vector in another CRS) |
| R-G9 | Rotated / sheared geotransform | rotated GeoTIFF through Warp, Clip, Identify | correct or refused with a message; never an axis-aligned reading | to automate (rotated geotransform; geographic terrain refuses a rotated raster in words) |
| R-G10 | Raster edge cases | no CRS (existing), zero-size, all NoData, CRS-less world file | readable errors; no crash | partial: crs_raster::r_g10 (no CRS refused in words; all NoData imports with a warning, C-8); zero-size and CRS-less world file to add |

### H. Map view (vitest, e2e, engine)

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-H1 | CRS switching over the zoo | 3 layers, every zoo CRS drawable | centre within 1 px, scale within 0.5 % (extends P9) | planned R6: e2e/crs.spec.ts |
| R-H2 | Readout vs engine | 10 vertices per CRS | lon/lat equal `transform_point` to 1e-7° (extends P8) | planned R6 |
| R-H3 | Outside the area of use | world layer in UTM 15N, NZTM, 3031 | notice with the engine's dropped count; a chip "data extends outside this CRS's area of use" | partial R6: status bar "outside area of use" notice (outsideArea, statusBar.test.ts); data-extent chip planned (risk G15) |
| R-H4 | Identify / select at the poles | EPSG:3031 around the South Pole; EPSG:3413 | the right features (P18) | planned R5 (engine), R6 (UI) |
| R-H5 | Basemap datum chip | map CRS 26715 / 26915 / 4326 | ≈ 15.7 m chip / none / none (extends P9b) | auto: existing M2 test; planned R6 for the zoo |
| R-H6 | Unsupported CRS | CRS proj4 can't express; custom string without area of use | previous CRS kept with a message; fallback extent used | planned R6 (extends 2.11) |
| R-H7 | Measure in the zoo | 10 km line in 6 CRSs | 10 km ± 1 m geodesic (extends P12) | planned R6 |
| R-H8 | Create features | click in CRS A, store in a layer in CRS B | ≤ 1 mm (extends P11) | planned R6 |
| R-H9 | CRS picker and custom entry | search by code, name, "UTM 15N", State Plane by state, by location; paste an EPSG code, WKT, PROJ string; invalid text | the picked CRS shows name, units, datum, area of use before Apply; invalid text is rejected with the engine's message; recents kept | auto: statusBar.test.ts (describeCrs), crs_identity::crs_info_*, e2e/crs-ui.spec.ts (custom entry preview + error); recents: crsRecents.test.ts, e2e/crs-ui.spec.ts |
| R-H10 | Layer source panel | layer in 26915 on a 3857 map | shows name, code, unit, datum, and "drawn in map CRS (reprojected)" | auto: crs_ui::r_h10_same_crs_by_meaning; Coordinate System tab shows code, unit, datum, area of use and "drawn in the map CRS (reprojected)" (manual check M-28) |
| R-H11 | Tissot indicatrix layer (Map tab, optional) | circles on a 15° lon/lat grid in UTM 15N, Albers 5070, Web Mercator, 3031 | each ellipse's axes equal PROJ `proj_factors` (h, k, convergence) within 0.1 %; in 5070 every circle has area ratio 1.0 ± 0.001; in 3031 circles at 71°S have scale 1.0; in 3857 the scale grows as 1/cos φ; circles that can't be projected are skipped and counted | auto: crs_ui::r_h11_* (Albers area ratio 1 ± 0.001 incl. polygon-vs-cap correction, Web Mercator k = 1/cos φ, 3031 sphere formula, unprojectable points counted, input checks), e2e/crs-ui.spec.ts. Note: area ratio is projected-circle area ÷ geodesic-circle area, not k_east × k_north (grid axes are not principal directions off the central meridian) |
| R-H12 | Distortion readout in the status bar | pointer over 10 positions per CRS | shows scale factor k and convergence equal to the engine's `factors` (same numbers as the layout scale bar); throttled like the coordinate readout (≤ 110 calls per 5 s sweep) | auto: crs_identity::map_factors_match_utm_theory (k = 0.9996, γ = Δλ sin φ), e2e/crs-ui.spec.ts; status bar throttled at 120 ms; same engine `factors` as the layout scale bar |

### W. World view (Phase R8, RD-10)

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-W1 | Whole-Earth grid for a conic | Lambert Conformal Conic (parallels 33°, 45°, lon_0 −96), Albers 5070, world view | grid extent covers the image of the globe; no feature dropped for "outside the area of use"; the south limit is reported | auto: world_view::r_w1 (grid ≫ area-of-use grid, different key, limit and note stated) |
| R-W2 | Curved edges | the 45°N parallel and a meridian from 0° to 80° in a conic | densified: max deviation of the drawn polyline from the exact curve < 1 px at z2 (not a straight chord) | auto: world_view::r_w2 (a 2-vertex edge across 180° gets ≥ 150 vertices, all on the 45th parallel) |
| R-W3 | Cut meridian | world polygon set (Eurasia, Antarctica, Fiji) in a conic with lon_0 = −96 and = 150 | polygons are cut at lon_0 ± 180° with no wrap-around streaks; areas of the pieces sum to the original (±0.01 % in an equal-area CRS) | auto: world_view::r_w3 (land cut at lon_0 ± 180 for lon_0 −96 and 150: no piece spans the map, equal-area totals agree within 0.2 % and match the geodesic area within 1 %) |
| R-W4 | Apex and poles | conic centred on the north: south pole; azimuthal south polar: north pole; Mercator: both poles | clipped at a stated limit with a notice, finite coordinates only (extends R-J1) | partial: world_view::r_w4 (UTM/ortho refused with a sentence; Mercator ±85, polar stereographic +30, conic limit stated; Web Mercator and geographic unchanged); apex finite-coordinate check via r_w1 |
| R-W5 | Graticule | every 15°, in 6 projection families | each line intersection equals `cs2cs` to 1 mm; lines curved; labels at the edges | partial: world_view::r_w5_w6_w7 (24 meridians, parallels stop at the conic limit, equator and tropics); cs2cs intersection check planned |
| R-W6 | Standard parallels and central meridian highlighted | conic (33°, 45°), lon_0 −96 | highlighted lines lie where scale k = 1 (Tissot circles are round and unit-sized there, R-H11) | auto: world_view::r_w5_w6_w7 (standard parallels 33°/45°, central meridian −96, k = 1 on the standard parallel) |
| R-W7 | Globe outline | each family | closed ring, projected from the CRS's valid domain; equal-area CRS outline area equals the sphere's surface scaled correctly | partial: edge-of-map lines (cut meridian and latitude limits) in the reference-lines layer; area check planned |
| R-W8 | Bundled world outline | 1:110m coastline in 6 projections | present, cited in Sources (Natural Earth, public domain); the file is ≤ 300 kB; no network call | auto: world_view::r_w8 (138 kB ≤ 300 kB, cited as Natural Earth, public domain, no network), e2e/world.spec.ts |
| R-W9 | Projection builder | conic with parallels 20°/60°, lon_0 10°; move the central meridian by drag | valid PROJ string with a stable key (R-B7); the map re-centres within one redraw; invalid parallels (equal and opposite, outside ±90°) rejected with a message | auto: projectionBuilder.test.ts (string, stability, rejected parallels), e2e/world.spec.ts (dialog, disabled Use button) |
| R-W10 | Distortion colouring | Albers, Lambert conformal, Mercator | Albers: area scale 1.0 everywhere; Lambert: angular distortion 0 everywhere; Mercator: area scale 1/cos² φ; values equal `proj_factors` within 0.1 % | deferred (optional distortion colouring; Tissot layer covers the readout) |
| R-W11 | Performance | world view of a 1 M-point layer in a conic | display table build < 15 s (release); pan stays interactive (no frame > 200 ms) | planned (ignored perf test) |
| R-W12 | Browser check | Lambert conformal conic and Albers, world view; world centred on Fiji | screenshots: cone shape, curved parallels, straight meridians converging at the pole, outline closed, no streaks | manual, planned R8 || manual: Lambert conformal conic world view checked by screenshot 2026-10-28 (cone shape, curved parallels, converging meridians, cut at 84°E, Antarctica below the stated 51°S limit); Albers and Fiji-centred still to look at |

### I. Layouts

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-I1 | Scale bar and north arrow over the zoo | frames in 10 CRSs at 1:24 000 and 1:1 000 000 | bar equals geodesic ground distance within 0.1 %; north arrow equals PROJ convergence within 0.001° (extends P4, P11) | planned R6: layout.rs |
| R-I2 | Graticule labels | UTM edge zone, Lambert, polar, geographic | five intersections per frame match `cs2cs` to 1e-6° (extends P16) | planned R6 |
| R-I3 | Antimeridian and pole frames | UTM 60/1 boundary; 3031 | graticule skipped or drawn per rule, with a warning; no NaN in the SVG | planned R6 |
| R-I4 | Set from Map View across a datum change | map CRS switched 26915 → 26715 (NAD27) | the frame keeps the same ground; centre matches `cs2cs` | planned R6 |
| R-I5 | CRS statement | text token `{crs}`; a new `{datum}` and `{units}` | name + code, datum, unit; "Custom CRS" for bare PROJ strings | auto: crs_ui::r_i5_crs_statement_for_layout_text; tokens {crs} {datum} {units} |
| R-I6 | Determinism | golden SVG for 3 zoo CRSs on all CI platforms | same bytes or documented tolerance | planned R7 (extends C2.1 open item) |

### J. Edge cases and robustness

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-J1 | Poles | point at lat ±90 through every zoo CRS | finite numbers or a counted drop; never NaN / inf in an output | ignored, bug C-2: crs_reference::r_j1 |
| R-J2 | Antimeridian | line 179° → −179°; polygon across 180°; Fiji | geodesic length 2° (not 358°); polygon valid; tiles split correctly | auto: crs_reference::r_j2 |
| R-J3 | Out of domain | Mercator at lat 89.9°; UTM at 85° N; Lambert far from origin | `Err` or a counted drop with a message | auto: crs_reference::r_j3 |
| R-J4 | Weird numbers | NaN, ±inf, 1e300, −0.0, subnormals | no panic; clear result | auto: crs_reference::r_j4 |
| R-J5 | Z and M through tools | 3D lines/points through Project, Buffer, Clip | per RD-8 | planned R3 |
| R-J6 | Empty and null geometries | reprojected | stay null / empty; counted | auto: crs_reference::r_j6 |
| R-J7 | Precision | 1e-9° steps in 4326; 1 mm steps in UTM; export and reimport | preserved to 1e-9° / 1 mm | auto: crs_reference::r_j7 |
| R-J8 | World-sized shapes | whole-world polygon in 4326 → 3857, 5070, 3031 | area 510 065 621.7 km² ± 0.001 % in geodesic measure; projection clipped and reported | ignored, bug C-3: crs_reference::r_j8 |
| R-J9 | Beyond a CRS's recommended area | layer spanning UTM zones 14–16 viewed in zone 15 | drawn; the area-of-use chip appears; scale factor reported at the edge | planned R5 |

### K. Platforms, concurrency, performance

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| R-K1 | Reference suite on all platforms | CI macOS, Windows, Linux | all R-A/R-D pass with one tolerance table; log prints PROJ and GDAL versions | planned R7 (risk G13) |
| R-K2 | Bundled data is the data used | packaged app `--self-test` | prints `proj.db` version and the datum grids found; asserts R-D1 on the bundled data | planned R3 (self-test), R7 (3 OS) |
| R-K3 | Concurrency | 16 threads × reproject 100 k points + tiles in 3 CRSs, 20 repeats | no crash; results equal the single-thread run | planned R7 (shares M4 P17) |
| R-K4 | Performance | 1 M points 4326 → UTM; 100 k polygons × 50 vertices Project | < 3 s; < 20 s (release); baseline file, fail on > 25 % slowdown | planned R7 (ignored, perf) |
| R-K5 | Cancel | Project of 3 M points, Cancel | stops within 2 s; layer unchanged | planned R7 (extends §13) |
| R-K6 | Offline PROJ | grid missing, no network | no network request is made (`PROJ_NETWORK` off); same message as R-D4 | planned R3 |

### Manual cases (steps added to §14 by their sessions)

| ID | Check | Coverage |
|---|---|---|
| M-27 (R6) | **Cross-check with another GIS.** Export 10 transformed points (zoo spread) as CSV and as GeoPackage; open both in QGIS or ArcGIS; the CRS is recognised by name (no "unknown") and the coordinates match to 1 mm | manual, planned R6 |
| M-28 (R6) | **Picker flows.** Search, filters, custom WKT / PROJ / EPSG entry, invalid entry, recents; the preview matches `projinfo` | manual, planned R6 |
| M-29 (R6) | **Print scale.** Export a 1:24 000 PDF in UTM; print at 100 %; a ruler measures the scale bar and a 1 km grid square | manual, planned R6 |
| M-30 (R7) | **Windows and Linux reference run.** Packaged app: `--self-test`, then R-D1 through the transform dialog; open the committed fixture project; compare numbers with macOS | manual, planned R7 |

## 3. Map view

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| 3.1 | Web Mercator tiles are byte-identical to the pre-M1 tiler | 3 layers, zooms 0–6 | equal bytes | auto: display::p1_web_mercator_tiles_are_byte_identical |
| 3.2 | UTM tile places a point within a pixel | point at known UTM coordinates | pixel error < 1 | auto: display::p2_utm_tile_places_the_point_within_a_pixel |
| 3.3 | Polar tile has a finite polygon around the pole | EPSG:3031 | finite, closed | auto: display::p3_polar_tile_has_a_finite_polygon_around_the_pole |
| 3.4 | Features that cannot be projected are counted | world layer in UTM | `dropped > 0` reported | auto: display::p4_world_layer_in_utm_reports_dropped_features |
| 3.5 | Editing drops display tables; edit cancels a running build | add field / delete rows | no stale tables | auto: display::p5_editing_a_layer_drops_its_display_tables, p5_an_edit_cancels_a_running_build |
| 3.6 | 1 M points tile build | 1 M points | completes, tile non-empty | auto (ignored, perf): display::p6_one_million_points |
| 3.7 | Dense tiles stay small | 500 k points z0 | < size cap | auto: engine::dense_tiles_stay_small |
| 3.8 | Tile for undefined CRS / deleted layer | no-CRS layer; removed layer | empty tile; error | auto: engine::shapefile_without_prj_is_flagged_not_guessed; tables::layer_removal |
| 3.9 | Raster tile and elevation identify | DEM | PNG; summit > 90 m | auto: engine::rasters_contours_hillshade_slope |
| 3.10 | Symbology builder: kinds per geometry, categorized/graduated fall-backs, quotes/unicode in class values, odd widths, `styleFields` | see styleEdge | no throw; Table → nothing | auto: style.test.ts; styleEdge.test.ts > buildLayers |
| 3.11 | Colour ramps for any n | n = 0, 1, −3, NaN, 2.5; 1-colour ramp | valid hex, dark end for n ≤ 1 | auto: styleEdge.test.ts > rampColors |
| 3.12 | Scale text parsing / formatting | `1:24,000`, `50k`, `2.5M`, `0.5`, `1e5`, `NaN`, 400 digits | 24000, 50000, 2.5 M; rejects the rest; `Infinity` formatted `1:∞` | auto: styleEdge.test.ts > parseScale edge inputs |
| 3.13 | Selection highlight in OpenLayers has no feature-state expressions | style for OL | none | auto: style.test.ts > OpenLayers variant |
| 3.14 | Pan / zoom with mouse and touchpad, zoom to layer, full extent | see M-5 | smooth, no blank tiles | manual M-5 |
| 3.15 | Basemaps (USGS Topo / Imagery / Relief / OSM / none) draw, attribution shown, datum offset note for NAD27 | see M-6 | tiles load; citation appears in Sources | manual M-6 |
| 3.16 | Coordinates readout follows the pointer in the map CRS (throttled) | see M-7 | updates ≤ 110 per 5 s sweep; ends on last position | auto: readout.test.ts; manual M-7 |
| 3.17 | Dark/light theme, panes dock / float / reset | see M-8 | layout restores | manual M-8 |
| 3.18 | Frame rate and benchmark dialog | see M-9 | numbers shown; no crash | manual M-9 |

### 3A. Feature labels on the map (L1, `docs/LAYOUTS_PLAN.md`)

| ID | Check | Expected | Coverage |
|---|---|---|---|
| L1-P1a | Polygon label anchors: a polygon cut by tile edges is labelled once (Web Mercator z2 and UTM z4, z5); a polygon whose anchor lies exactly on a tile edge belongs to one tile | exactly 1 anchor across all tiles; 4 polygon pieces in Web Mercator | auto: display::l1_p1_polygon_label_anchors_are_unique_across_tile_edges_in_web_mercator_and_utm |
| L1-P1b | Label text: empty and NULL draw nothing; numbers lose float noise; points label to the right, lines along the line, polygons centred; polygons read anchor tiles, points and lines their own | as stated | auto: map/labels.test.ts |
| L1-P2 | Labels hide outside the scale range (zoomed-out and zoomed-in limits), read at draw time; limits parse from "250000", "1:250,000", blank | as stated | auto: map/labels.test.ts |
| L1-P3 | Settings survive save and reopen (style JSON round trip); projects saved before L1 have labels off; the tile carries the label field only while labels are on | as stated | auto: map/labels.test.ts; e2e labels.spec.ts; engine fixture layer_style.json |
| L1-P4 | Labels draw in Web Mercator and UTM 14N; the label layer is removed when labels go off | label layer present on both grids, anchor tiles non-empty | auto: e2e/labels.spec.ts; visual check in UTM 14N 2026-10-28 |
| L1-E1 | Label a layer with a NULL-heavy field, a 10 000-feature layer, a field with very long text, a layer edited while labels are on | no error; labels follow the edit; tile size stays bounded | to automate |
| L1-E2 | Properties → Labels section: on/off, field, size, colours, halo, both scale limits, clearing a limit | each control changes the map and persists | manual (M-8-style pass) |
| L1-E3 | Label a layer whose display in a CRS is still building | labels appear when the symbols do | to automate |

## 4. Selection, identify, measure

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| 4.1 | Select modes new / add / remove / toggle / subset | ids lists, duplicates, ids not present, empty list, none selected yet | set arithmetic correct; empty stays empty | auto: selection.test.ts; appStore.test.ts > selection edge cases |
| 4.2 | Large selection | 200 000 ids | works, fast | auto: appStore.test.ts |
| 4.3 | Select by attributes | `pop >= 20`, `name IS NULL`, OR, nothing, bad column, syntax error, unknown layer | exact fid lists; errors with messages | auto: tables::select_by_attribute_respects_filters_and_reports_bad_ones |
| 4.4 | Definition query | bad / blank / matching nothing | bad rejected and layer unchanged; blank = none; `query_count`; applies to table, selection, unique values, box select | auto: tables::definition_queries_reject_bad_input_and_apply_everywhere; engine::definition_query_and_selected_only_export |
| 4.5 | Select by rectangle | box over 2 of 3 points; whole world; empty area; box touching an edge; line | exact fids | auto: tables::box_select_covers_touching_and_whole_world_and_degenerate_boxes |
| 4.6 | Zero-size box | click-sized box on a point | selects the point | auto: tables::zero_size_box_selects_the_point_under_it (B-T3 fixed by M3) |
| 4.7 | Select by location (map command and tool) | points vs polygons; distance; invert; selected-only "by" | exact fids; both definition queries apply | auto: engine::select_by_location_on_the_map; tools::select_by_location_tool_known_result |
| 4.8 | Identify | click on a point, near miss at high/low zoom, inside a polygon, overlap, out-of-range lon/lat, table layer | nearest first; tolerance grows with zoom; no panic | auto: tables::identify_hits_the_nearest_feature_within_a_few_pixels, identify_and_selection_ignore_layers_without_a_crs_or_geometry |
| 4.9 | Identify on a raster | pixel / NoData / outside | value / none / none | auto: rasters::raster_layers_report_stats_and_draw_tiles; far away: (B-T6 fixed) rasters::identify_far_from_a_projected_raster_is_just_empty |
| 4.10 | Feature attributes popup values, NULLs | fid 1 | strings, `None` for NULL | auto: tables::cell_edits_convert_types_and_explain_failures |
| 4.11 | Geodesic measure of one feature | polygon 0.01°², line 0.1°, point | area ±1 %, perimeter, length, centroid; right fields only | auto: tables::measure_feature_is_geodesic_and_type_aware; engine::geodesic_buffer_near_and_measure |
| 4.12 | Extent / zoom to selection | all, one, empty, missing fid | bounds; empty → none | auto: tables::extents_for_selections |
| 4.13 | Measure tool maths (distance, azimuth, area) | JFK→LHR, poles, antipodes, antimeridian, closed ring both windings | GeographicLib values; finite | auto: geodesy.test.ts; geodesyEdge.test.ts |
| 4.14 | Measure sketch state | add points, finish, clear | double-click ends; next click starts a new line | auto: navStore.test.ts |
| 4.15 | Unit formatting | m, mi, usft, ha, acres; NaN, ∞, unknown unit | readable text, no throw | auto: geodesyEdge.test.ts > formatting |
| 4.16 | MGRS / lat-lon parsing | `14S PF 12345 67890`, lower case, odd digit counts, `91, 0`, `NaN, NaN` | exact lon/lat or a readable error | auto: geodesyEdge.test.ts > position parsing; geodesy.test.ts |
| 4.17 | North references and mils | true/grid/magnetic conversions, mils, convergence sign, WMM2025 declination | inverse pairs; 0–360 | auto: geodesy.test.ts, geodesyEdge.test.ts > azimuth conversion |
| 4.18 | Resection / intersection | 3 landmarks; parallel rays | fix + error triangle; readable message | auto: geodesy.test.ts > intersection and resection |
| 4.19 | Tool buttons: Select (Shift adds, Ctrl toggles), Select by Rectangle, Identify popup, Measure (double-click ends, Esc clears), Measure Area, Land Nav panels | see M-10 | as in tooltips | manual M-10 |

### 4A. Split by Attribute (B6, `docs/DECISIONS.md`)

| ID | Check | Expected | Coverage |
|---|---|---|---|
| B6-1 | Groups: one per distinct value, NULL as "(no value)", row counts, definition query honoured | exact counts; hidden rows not split | auto: split::b6_groups_*, b6_a_definition_query_* |
| B6-2 | Layers target: one new layer per value; CRS and citations kept; no row lost or doubled; input untouched | sum of rows = input rows | auto: split::b6_layers_one_per_value_* |
| B6-3 | Files target: one file per value (GeoPackage, Shapefile); file names safe on every OS; `_2` on clashes, also between case variants; an existing file is never overwritten; CRS written | as stated | auto: split::b6_files_*, b6_shapefile_*, b6_safe_names |
| B6-4 | Limits: more than 200 outputs need a confirmation, more than 1 000 are refused, nothing is made before the user confirms | readable messages | auto: split::b6_limits_confirm_and_cap; vitest splitDialog.test.ts |
| B6-5 | Cancel: layers made so far are removed, the task disappears, the engine still works | nothing left behind | auto: split::b6_cancel_* |
| B6-6 | Dialog in the app: field picker, counts shown before writing, layers and files targets, folder required | as stated | auto: e2e/split.spec.ts |
| B6-7 | Edge cases: a 1 000-value field into files; a value with a leading dot or only spaces; read-only folder; the folder is the source file's folder (the source is not touched); a table without geometry; a layer with a node-tree result | to automate |

### 4B. Category drawing order (B7, `docs/DECISIONS.md`)

| ID | Check | Expected | Coverage |
|---|---|---|---|
| B7-1 | Reorder: move one class, clamp at the ends, reverse; inputs not changed | as stated | auto: map/categoryOrder.test.ts |
| B7-2 | Map: each category's rank is its style zIndex (top of the list highest, "everything else" 0); the shared base style is not changed; follows a reorder | as stated | auto: map/categoryOrder.test.ts; visual check of two overlapping polygons, both orders, 2026-10-28 |
| B7-3 | Layouts: unique values draw "everything else" first, then the list from the bottom up; graduated and single styles keep their order | the first class is on top | auto: layout::b7_* (goldens `p3`, `all_elements`, `l2_p1` regenerated for the new order) |
| B7-4 | Symbology list: ▲ ▼ buttons (disabled at the ends), drag handle, Reverse order; the order is saved in the style, one entry per value | as stated | auto: e2e/category-order.spec.ts |
| B7-5 | Legend follows the list order | same order as the list | auto: layout::p13 (legend lists classes in order); to automate after a reorder |
| B7-6 | Edge cases: drag and drop by mouse (only buttons are tested), a project saved before B7 opens in its value order, a classified field with 60+ values (the list scrolls), a category present in no feature | to automate |

## 5. Attribute table and editing

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| 5.1 | Paging, sort asc/desc with NULLs last, filter, selected-only, empty selection | limit 0 → 1, 1 M → capped, offset past end, unknown sort field, bad filter | exact pages; errors for bad sort/filter | auto: tables::paging_sorting_and_filtering |
| 5.2 | Add field: all 5 types; bad / reserved / duplicate names (case-insensitive); odd names | `""`, `geom`, `__fid`, `NAME` | rejected with no change; odd names round trip | auto: tables::field_edits_reject_bad_names_and_types_and_leave_the_layer_alone |
| 5.3 | Rename / delete field errors | missing, onto an existing, to `geom` | rejected | same |
| 5.4 | Cell edit conversions and messages | date, boolean, integer, double; `yesterday`, `maybe`, `1.5.2`, `1,5` | "not a valid … value"; failed edit changes nothing | auto: tables::cell_edits_convert_types_and_explain_failures |
| 5.5 | Empty text: numeric → NULL, text → empty string; explicit NULL | `""` | as stated | same |
| 5.6 | Quotes / SQL in text cells | `O'Brien "x"; DROP TABLE y` | stored as data | same |
| 5.7 | Edit geometry column or missing field / fid | `geom`, `ghost`, fid 9999 | rejected / no change | same |
| 5.8 | Edits mirror into overview tables and tiles | 60 000 polygons | tile reads edited values | auto: engine::spreadsheet_style_attribute_editing |
| 5.9 | Calculate Field on selection / all / empty selection; NULL propagation; division by zero; bad expression / target | `pop / 2` on [1,3]; `1/(pop-10)`; `pop +` | counts; unselected stay NULL; errors leave fields unchanged | auto: tables::calculate_field_handles_selection_nulls_and_division |
| 5.10 | Calculate Field type inference and explicit type | text expr; `Some("text")` | VARCHAR | same |
| 5.11 | Calculate Geometry units | area in sqft/sqm/acres, length m | acres = m² / 4046.8564 | auto: tools::calculate_field_and_geometry_known_values; engine::spreadsheet_style_attribute_editing |
| 5.12 | Delete rows | none, missing ids, duplicates, all rows | exact counts; idempotent; empty layer OK | auto: tables::deleting_rows_is_exact_and_idempotent |
| 5.13 | Node-tree results and unknown layers are read-only / error | edit a result layer | error "node-tree result…" | auto: tables::node_results_and_unknown_layers_cannot_be_edited |
| 5.14 | Rename / remove layer | blank name; remove drops tables and uses | name kept; layer gone | auto: tables::layer_removal; (B-T2 fixed) tables::renaming_a_layer_to_blank_is_refused |
| 5.15 | Unique values, quantile breaks | NULLs, limit 0/1, text field, classes 0/1/3 | counts add up; empty for 0/1; median 2.5 | auto: tables::unique_values_and_breaks_edge_cases |
| 5.16 | Copy layer / layer from selection | fids subset | row count; inherits sources | auto: engine::data_sources_are_logged_with_citations; progress_sources::derived_layers_cite_every_input_once |
| 5.17 | Spreadsheet UI: edit a cell with Enter/Tab/Esc, sort by header, resize columns, paste a column, filter box | see M-11 | values saved; Esc cancels | manual M-11 |
| 5.18 | Attribute dialogs: Add Field, Calculate Field, Calculate Geometry, Select by Attributes | see M-11 | errors shown inline | manual M-11 |
| 5.19 | Definition query dialog (Builder / Python / SQL tabs) | see M-12 | tabs translate between each other | manual M-12 |
| 5.20 | Query builder → SQL | quotes, injection text, odd field names, numeric checks, empty rows | safe SQL; row-numbered errors | auto: builder.test.ts; queryEdge.test.ts > builder edge cases |
| 5.21 | Python → SQL | `and/or/not`, `in`, `None`, `startswith`, `!FIELD!`, quotes, empty, brackets, 2 000 terms | translated; unbalanced brackets / bad characters → `QueryError`; empty → `""` | auto: pyToSql.test.ts; queryEdge.test.ts > Python filter translation |

## 6. Node editor (Model)

| ID | Check | Input | Expected | Coverage |
|---|---|---|---|---|
| 6.1 | One tree per layer, fixed Layer Input/Output, switching keeps edits, deleted layer drops tree | open/close trees | restored; history cleared | auto: nodeEditor.test.ts > switching layer trees; modelStore.test.ts > trees |
| 6.2 | `strip` leaves no selection/measure state; `load` restores | tree with `selected`, `measured` | not saved | auto: modelStore.test.ts > trees |
| 6.3 | One connection per input port; other ports untouched | connect twice | last wins | auto: modelStore.test.ts > editing the graph |
| 6.4 | Port type rules | all `accepts()` combinations; `kindAfter`; raster→vector; cycles | table in code; readable reason text | auto: modelStore.test.ts > port type compatibility; nodeEditor.test.ts > connections |
| 6.5 | Fixed nodes cannot be removed; edges of removed nodes go | remove Layer Input/Output | kept | auto: modelStore.test.ts |
| 6.6 | Add node applies defaults; unknown tool adds a bare node | `buffer`, `mystery` | as stated; project dirty | auto: modelStore.test.ts |
| 6.7 | Splice a node onto a connection (one undo step, types checked both sides, refusal changes nothing) | A→B | A→node→B | auto: nodeEditor.test.ts > connections and insertion |
| 6.8 | Delete with reconnect, copy/paste across trees, make links, duplicate | selections | as Blender | auto: nodeEditor.test.ts > graph operations |
| 6.9 | Undo grouping (typing in a setting is one step; bursts across nodes) | many keystrokes | one step | auto: nodeEditor.test.ts > undo grouping |
| 6.10 | Mute = bypass; chain of muted nodes; no feed; fixed never muted; other ports dropped | 2 muted in a row | direct link | auto: nodeEditor.test.ts > mute and change tracking; modelStore.test.ts > bypassing muted nodes |
| 6.11 | Payload to engine: extra Layer Input uses the layer's shown result; deleted layer unchanged; null handle → "" | results map | as stated | auto: modelStore.test.ts > what is sent to the engine |
| 6.12 | Change signatures: settings, upstream, layer version, edge order, cycles, missing | edit param | stale marker only when needed | auto: nodeEditor.test.ts; modelStore.test.ts > change signatures |
| 6.13 | Output fields before running (carry-through, named, prefixed, conditional, `keepParam`, `noneIf`, `layerParam`) | per tool rules | equal to the engine's real outputs | auto: nodeEditor.test.ts > output fields; modelStore.test.ts > output fields before running; engine::declared_output_fields_match_real_outputs |
| 6.14 | Recording run outputs is not an undo step or an edit | `setLastOutputs` | no history, not dirty | auto: modelStore.test.ts |
| 6.15 | Executor: topo order, cache hit, cache key, failure skips downstream, missing input, bad expression | chain of 3 | second run `Cached`; errors readable | auto: engine::model_reuses_cached_steps, missing_input_and_bad_expression_report_errors; history::h3_hash_is_stable_across_param_key_order |
| 6.16 | Output replaces the layer in place; Save as new layer; original untouched | Output node | `result_of`, copy independent | auto: engine::layer_tree_output_replaces_display_and_can_be_saved_as_new_layer |
| 6.17 | Parameter validation | non-number, bad enum | readable errors | auto: engine::bad_settings_are_rejected_with_a_readable_message |
| 6.18 | Every tool spec is well-formed | all | unique ids; enum defaults are options; `fieldOf` names a port | auto: tools::every_tool_spec_is_well_formed |
| 6.19 | Keyboard shortcuts: no default conflicts; every default round-trips through `eventCombo`; digits/space/numpad; lone modifiers ignored | all actions | as stated | auto: keymap in nodeEditor.test.ts; keymapStore.test.ts > default bindings / combo handling |
| 6.20 | Rebinding persists, steals combos, resets, survives corrupt / unavailable storage | `mute → X` | saved; `delete` loses X | auto: keymapStore.test.ts > saved bindings |
| 6.21 | Run model with progress "Step n of N" and Cancel | see M-13 | progress bar; Cancel stops | manual M-13 |
| 6.22 | Drag nodes, box select (Shift+drag), Ctrl+right-drag cut, Shift+A menu with search & keyboard, drop a node on a link, drag a layer from Contents / Catalog onto the canvas | see M-14 | as in the Shortcuts dialog | manual M-14 |
| 6.23 | Shortcuts dialog: rebind, conflict message, reset | see M-15 | conflicts named | manual M-15 |
| 6.24 | Layer deleted while a tree references it | delete a layer used by another tree | run says "Layer no longer exists"; other trees flagged | auto: nodeEditor.test.ts > drops the tree of a layer that was deleted; manual M-16 for the flag |

## 7. Tools (`crates/engine/src/nodes/`)

Every row: output fields, known result, CRS preserved (`crs` = input's), and the refusal paths from §2.5. All fixtures are UTM 14N unless stated (see the header of `tests/tools.rs`).

| Tool (spec id) | Known-result case | Coverage |
|---|---|---|
| Layer Input (`input_layer`), Output (`layer_output`), Add to Map (`add_to_map`) | tree result replaces display; copy independent; cites all inputs | auto: engine::layer_tree_output_replaces_display_and_can_be_saved_as_new_layer, model_results_cite_every_input_layer |
| Filter (`filter`) | `pop > 15` → 2 rows; `pop > 1000` → 0; bad SQL → Error without `__node_` in the message | auto: tools::filter_and_summary_statistics |
| Buffer (`buffer`) | planar 10 m on points → area ≈ 312 m² (3.1214 r²), fields kept, dissolve → 1 row and no fields; geodesic 100 m → 31 214 m² ±3 %; `usft` converted; ground-true on geographic data | auto: tools::buffer_planar_dissolve_and_geodesic_areas; engine::buffer_is_true_ground_distance_on_geographic_data, geodesic_buffer_is_true_distance_with_shared_projections; crs_edge::us_survey_feet_are_not_treated_as_metres |
| Clip (`clip`) | two squares ∩ mask → 2 pieces, total 10 000 m², input fields only | auto: tools::clip_erase_intersect_known_areas_and_crs |
| Erase (`erase`) | 30 000 − 10 000 → 20 000 m², 3 rows | same |
| Intersect (`intersect`) | 2 rows, 10 000 m², fields of both inputs | same |
| Union (`union`) | mask inside input → total 30 000 m² | auto: tools::union_and_symmetrical_difference_areas; engine::union_symmetrical_difference_and_collect |
| Symmetrical Difference (`symmetrical_difference`) | 20 000 m² | same |
| Dissolve (`dissolve`) | none → 1 row 30 000 m²; by `name` → 2 rows, `a` = 20 000 | auto: tools::dissolve_and_collect_group_by_field |
| Collect (`collect`) | by `name` → 2; none → 1 | same |
| Merge (`merge`) | 3 + 1 → 4 rows; union of fields | auto: tools::attribute_join_and_merge; engine::new_geoprocessing_tools_run |
| Explode (`explode`) | two-part feature → 2 rows of 100 m² | auto: tools::simplify_explode_repair_delaunay_voronoi |
| Repair (`repair`) | bow-tie → valid, same row count | same |
| Simplify (`simplify`) | 1 m wiggle vanishes at 5 m (2 vertices), kept at 0.1 m | same |
| Project (`project`) | UTM → 4326: lon −98.1…−97.7; back within 1 mm | auto: tools::project_changes_crs_and_round_trips |
| Spatial Join (`spatial_join`) | keep-all → 3 rows, 1 NULL join; inner → 2; contains → 2 | auto: tools::spatial_join_predicates_and_keep_all |
| Join Field (`attribute_join`) | left → 3 rows, 2 labels; inner → 2 | auto: tools::attribute_join_and_merge |
| Calculate Field (`calculate_field`) | `pop*2` → sum 120; fields `[id,name,pop,TWICE]` | auto: tools::calculate_field_and_geometry_known_values |
| Calculate Geometry (`calculate_geometry`) | 30 000 grid m² → 30 024 m² ellipsoidal; length 200/0.9996; acres factor | same |
| Select by Location (`select_by_location`) | 2 inside, 1 outside (invert) | auto: tools::select_by_location_tool_known_result; engine::new_geoprocessing_tools_run |
| Summarize Within (`summarize_within`) | COUNT 1, 1, 0; sum field | auto: tools::summarize_within_counts_and_sums |
| Summary Statistics (`summary_statistics`) | 1 row; by case → 2 rows | auto: tools::filter_and_summary_statistics |
| Near (`near`) | NEAR_DIST 0 inside; ≈ 1204 m to corner (unit follows radius); beyond radius → NULL | auto: tools::near_and_join_nearest_report_distance_and_respect_radius |
| Join Nearest (`join_nearest`) | radius 0.5 km: keep-all 3, inner 2 | same |
| Fishnet (`fishnet`) | clip ≤ no clip; hexagons | auto: tools::fishnet_grid_cells |
| Feature to Point (`feature_to_point`) | centroid of square = its centre; POINT | auto: tools::feature_to_point_and_bounding_geometry |
| Bounding Geometry (`bounding_geometry`) | envelope 40 000; hull 35 000 m² | same |
| Concave Hull (`concave_hull`) | ratio 1 = convex hull 47 500 m² | same |
| Delaunay (`delaunay`) | 3 points → 1 triangle, 47 500 m² | auto: tools::simplify_explode_repair_delaunay_voronoi |
| Voronoi (`voronoi`) | 3 cells carrying point attributes | same |
| Polygons to Lines / Lines to Polygons | 3 rings; round trip back to 30 000 m² | auto: tools::lines_and_vertices_tools |
| Extract Vertices (`extract_vertices`) | `VERTEX_INDEX` last field | same |
| Points to Path (`points_to_path`) | 1 line of length 100 + √(850²+950²) | same |
| Points Along Lines (`points_along_lines`) | ≥ 10 points for 600 m at 50 m | same |
| Line Intersections (`line_intersections`) | 2 points at (X0+50, Y0+50) | auto: tools::line_intersections_finds_the_crossing |
| Random Points in Polygons (`random_points`) | 5 per polygon = 15 | auto: tools::point_utilities_random_duplicates_nulls |
| Delete Duplicates / Remove Null Geometries | 4 identical → 1; NULL geometry dropped | same |
| Export (`export`) | file reads back with CRS | auto: tools::export_writes_a_file_that_reimports_with_its_crs; formats::* |
| Contours (`contours`) | ramp 0–39 m, interval 10 → 3 lines (10, 20, 30); base 5 → 4; interval 100 → 0; interval 0 / −5 → Error | auto: rasters::contours_follow_the_interval_base_and_index_settings; engine::rasters_contours_hillshade_slope |
| Hillshade (`hillshade`) | flat surface at 45° = 255·sin 45° ≈ 180 for every azimuth; z factor changes shading | auto: rasters::slope_aspect_and_hillshade_of_a_plane_match_the_formulas |
| Slope (`slope`) | plane rising 1 m per 30 m pixel: 1.909° / 3.333 % | same |
| Aspect (`aspect`) | surface facing west = 270° | same |
| Terrain Ruggedness (`ruggedness`) | flat TRI = 0; plane TPI = 0 | same |
| Clip Raster by Extent (`clip_raster`) | 300 m box → ≈ 10×10 px, origin within a pixel, 30 m pixels | auto: rasters::clip_and_warp_keep_the_grid_honest |
| Warp (`warp`) | → EPSG:4326: degrees pixel size, centre value kept | same |
| Warp error paths | invalid CRS; raster without CRS | auto (B-T5, B-T7 fixed): rasters::warp_to_an_invalid_crs_is_an_error_not_a_crash, warping_a_raster_of_unknown_crs_is_refused_not_guessed, raster_helpers_report_bad_gdal_options_instead_of_crashing, raster_tools_that_compare_with_vectors_refuse_a_raster_of_unknown_crs |
| Polygonize (`polygonize`) | two classes + NoData hole → 2 polygons | auto: rasters::polygonize_sampling_and_zonal_statistics_handle_nodata |
| Sample Raster (`sample_raster`) | class values; NoData and outside → NULL; no row dropped | same |
| Zonal Statistics (`zonal_statistics`) | mean ignores NoData (391 px); all-NoData zone → NULL/NaN | same |
| Raster port given a vector | readable error | auto: rasters::a_vector_layer_in_a_raster_port_fails_with_a_reason; engine::rasters_contours_hillshade_slope |
| QGIS algorithm ids cited in each tool's doc comment | `// QGIS: … (native:…)` present, no GPL code | manual M-17 (code review) |
| Tools missing from the roadmap (calculator, reclassify, mosaic, rasterize, hydrology, interpolation, spatial statistics, remote sensing) | — | gap (not built) |

## 8. Attribute history ("git for tables")

| ID | Check | Coverage |
|---|---|---|
| 8.1 | Node snapshots survive save → close → reopen → Save As; run is all `Cached` afterwards | auto: engine::h1_h7_snapshots_survive_save_close_reopen_and_save_as; project::everything_survives_save_close_open |
| 8.2 | Snapshots from another engine version are dropped on open; broken views / orphans dropped | auto: engine::h2_…, h4_… |
| 8.3 | Hash stable across parameter key order | auto: engine::h3_hash_is_stable_across_param_key_order |
| 8.4 | Prune drops non-live tables; budget never evicts pinned | auto: engine::h5_prune_drops_non_live_and_budget_never_evicts_pinned |
| 8.5 | A failed step leaves no table or index row | auto: engine::h6_failed_step_leaves_no_table_and_no_index_row |
| 8.6 | Key diff: counts, changed cells incl. NULLs; schema diff (added / removed / retyped, case-insensitive); auto-key choice; content diff is a multiset; vertex move is a change; CRS mismatch skips geometry | auto: history::h8–h12 |
| 8.7 | Paging of node rows is stable for tables and Layer Input views | auto: history::h13_node_rows_page_stably_for_tables_and_layer_input_views |
| 8.8 | Diff tables dropped on open / new project; ids unique per call | auto: history::diff_tables_are_dropped_on_open_and_new_project, diff_ids_are_unique_per_call_and_drops_are_independent |
| 8.9 | Diff is a cancellable task | auto: history::h15_diff_runs_as_a_cancellable_task_and_cancel_stops_it |
| 8.10 | 1 M-row key diff speed | auto (ignored, perf): history::h14_million_row_key_diff_performance |
| 8.11 | UI helpers: picker order and badges, default compare input, prune payload, diff totals, recoverable errors | auto: history.test.ts |
| 8.12 | Table pane: node picker, diff bar chips (added / removed / changed), node-card menu, "Output" default | manual M-18 |
| 8.13 | Undoing a node edit keeps `lastOutput` (B5: `lastOutputs` map outside undo history; old projects migrate on load) | edit, run, undo, redo | node still has its table | auto: modelStore.test.ts > "B5: undo and redo keep…", "B5: load() moves…" |

## 9. Layouts

| ID | Check | Coverage |
|---|---|---|
| 9.1 | PNG renders; PDF is vector with embedded text; SVG deterministic; golden file | auto: layout::p1_png_renders, p2_pdf_is_vector_with_embedded_text, p3_deterministic_and_golden, all_elements_page_is_deterministic_and_golden |
| 9.2 | Missing CRS errors; layers without a CRS reported; hidden layers; unknown kinds | auto: layout::p5_…, hidden_layers_and_unknown_kinds |
| 9.3 | Clipping to the frame; graduated steps | auto: layout::clips_to_the_frame_and_follows_graduated_steps |
| 9.4 | Style structs mirror the UI (`LayerStyle`) | auto: layout::layer_style_mirror_matches_the_ui_fixture; styleFixture.test.ts |
| 9.5 | 100 k polygons PNG at 150 dpi | auto (ignored, perf): layout::p6_100k_polygons_png_150dpi |
| 9.6 | No ellipsoid literals outside `geo.rs` | auto: layout::no_ellipsoid_literals_outside_geo_rs |
| 9.7 | Layouts stored in the project and survive reopening; validation (version, bad grid) | auto: layout::layouts_are_stored_in_the_project_and_survive_reopening, save_layout_validates; project::everything_survives_save_close_open (non-ASCII names) |
| 9.8 | Export SVG/PDF/PNG atomically; cancelled render writes nothing | auto: layout::export_writes_svg_pdf_png_atomically_and_preview_is_base64_png, cancelled_render_writes_nothing |
| 9.9 | Scale bar equals geodesic distance; north arrow follows convergence and rotation; scale-variation warning at 60°N Mercator | auto: layout::p11_…, p12_…, p15_… |
| 9.10 | Legend lists visible layers in draw order with class labels; reports when it does not fit; Sources cites derived layers; hidden frame's surround reported | auto: layout::p13_…, legend_reports_when_it_does_not_fit, p14_…, surround_of_a_hidden_frame_is_reported_not_drawn |
| 9.11 | Graticule follows the projection; grid lines straight at round coordinates; bad specs rejected | auto: layout::p16_…, grid_lines_…, model_rejects_bad_grid_specs_and_unprintable_basemaps |
| 9.12 | Raster is one image at ≤ 300 dpi; USGS basemap warped and cited | auto: layout::p17_…, p18_basemap_is_warped_to_the_frame_and_cited; (ignored, network) p18_network_usgs_topo_in_utm |
| 9.13 | Layout units: unit tests for text measure, bar plan, numbers, colour escapes, scale factors, only-earth body | unit: layout::* |
| 9.14 | Page sizes / orientation / custom size; `rounded`; standard scales; `nextId`; `frames()` | auto: layout/types.test.ts |
| 9.15 | Layout store: insert, move, resize, undo per layout, debounce save, duplicate / rename / delete | auto: store/layout.test.ts |
| 9.16 | Layout tab: draw/drag elements, rulers (not built), Set from Map View, Frame CRS, text tokens `{scale}`, export dialogs, dpi box (not built) | manual M-19 |
| 9.16a | L2 labels in layouts: points, lines, polygons labelled with the layer's `LayerStyle.label`; halo underlay; line labels turned to the line and never upside down | auto: layout::l2_labels_draw_for_points_lines_and_polygons_with_halos |
| 9.16b | L2-P1 deterministic, golden SVG hash (same bytes on every platform; the CI run is the check on Linux and Windows) | auto: layout::l2_p1_labels_are_deterministic_and_golden |
| 9.16c | L2-P2/P3 no two label boxes overlap (400 labels packed into 8 mm); every candidate is placed or counted; the frame reports "N labels of X didn't fit" | auto: layout::l2_p2_p3_*; e2e/labels.spec.ts |
| 9.16d | L2 scale range, unknown field, label off, hidden layer: no labels | auto: layout::l2_labels_follow_the_scale_range_*, l2_unknown_field_* |
| 9.16e | L2-P4 label text in the PDF is real text in an embedded font | auto: layout::l2_p4_pdf_label_text_is_real_text |
| 9.16f | L2 in the app: the layout preview changes when labels turn on | auto: e2e/labels.spec.ts (L2) |
| 9.16h | L3-P1 a moved label keeps its page position in the preview, the SVG and the PDF; the same edits give the same bytes; edits survive the layout JSON round trip | auto: layout::l3_p1_*, l3_labels_overrides_survive_*; labelEdits.test.ts |
| 9.16i | L3-P2 a hidden label is not drawn, is listed (hidden) so it can be brought back, and the automatic one does not return | auto: layout::l3_p2_* |
| 9.16j | L3-P3 pinned labels are never dropped (even off the frame); automatic labels avoid them; no two drawn boxes overlap | auto: layout::l3_p3_* |
| 9.16k | L3-P4 a per-layout style (size, colour, on/off, field) changes this layout only; a layout can label a layer the map does not | auto: layout::l3_edits_text_size_rotation_and_a_per_layout_style; labelEdits.test.ts |
| 9.16l | L3-P5 an edit for a deleted feature is ignored, reported, and removed by the UI | auto: layout::l3_p5_*; labelEdits.test.ts |
| 9.16m | L3-P6 undo and redo of a move (one step, selection kept) | auto: labelEdits.test.ts |
| 9.16n | L3 in the app: click selects a label, drag moves it, Properties edits text, Hide, Reset to automatic | auto: e2e/labels.spec.ts (L3) |
| 9.16o | L3 edge cases: a data re-import (fids change), two frames showing one layer, a locked frame, drag while a preview renders, rotate by handle (not built: rotation is a number in Properties) | to automate / not built |
| 9.16g | L2 edge cases: more than 50 000 features (warns), labels over a basemap, rotated frame, NULL-heavy field, multi-part lines, polygons cut by the frame | to automate |
| 9.16p | C3.0 basemap tiles cached on disk: a second render makes no network requests and gives the same picture; no cache folder = every open fetches again; the cache folder is per user and overridable | auto: basemap_cache.rs c3_0_* |
| 9.16q | C3.0 scale bar prints "1:N" under the bar when asked (off by default), warns when the bar is too short, flag survives the layout JSON | auto: layout::c3_0_scale_bar_can_print_the_ratio_under_the_bar |
| 9.16r | C3.0 Arrange: bring to front/back, forward/backward (moved ones keep their order), align left/centre/right/top/middle/bottom (to each other, or to the page margins for one), locked elements skipped, one undo step | auto: layout/arrange.test.ts; e2e/layout-polish.spec.ts |
| 9.16s | C3.0 rulers: mm ticks along the top and left, majors at least 60 px apart, no "-0", follow zoom and scroll | auto: layout/rulers.test.ts; e2e/layout-polish.spec.ts; visual check 2026-10-28 |
| 9.16t | C3.0 dpi box: 72–600 whole numbers only, remembered, applies to PNG and to images inside a PDF | auto: arrange.test.ts (parse); e2e/layout-polish.spec.ts; to automate an export at 150 dpi and read its pixel size |
| 9.16u | C3.1 P20: every bundled template validates, has the six elements inside its page, and renders (title token, scale bar ratio, no "too narrow/short" warnings) | auto: templates.rs p20_*, bundled_templates_are_unique_named_and_ordered |
| 9.16v | C3.1 user templates: save (name kept as typed, file name made safe), list after the bundled ones, load, replace on the same name, delete; bundled ones can't be deleted; ids can't leave the folder; an invalid layout or empty name is refused; a broken file is skipped and reported | auto: templates.rs user_templates_*, templates_refuse_*, a_broken_user_file_*, the_engine_keeps_user_templates_* |
| 9.16w | C3.1 in the app: New Layout → From template (frame takes the map's CRS and view; refused with words when the map has no CRS); Layout → Save as Template (replace asks first); Delete Template | auto: e2e/templates.spec.ts |
| 9.16x | C3.1 edge cases: a template saved from a layout with several frames; templates with a basemap; a read-only templates folder; a very long template name; two layouts from one template | to automate |
| 9.17a | C3.2 pages: ordered by the sort field (numbers as numbers, text without regard to case, NULL last, ties by feature id), the definition query limits them, readable errors for a missing layer, field, geometry or series, and an invalid series is refused by the layout JSON | auto: series.rs series_pages_*, series_errors_are_readable |
| 9.17b | C3.2 frames: a followed frame is centred on its feature, fits it with the margin in the tighter direction (scale a whole number, no looser than needed); fixed scale gives every page the same scale; a point index falls back to the series scale or 1:10 000 | auto: series.rs a_followed_frame_*; point fallback to automate |
| 9.17c | C3.2 tokens `{page_name}`, `{page_number}`, `{page_count}`, `{page_<field>}`; unchanged without a series page | auto: series.rs tokens_* |
| 9.17d | **P21** 10 counties → a valid 10-page Letter PDF with embedded fonts, pages in name order, title shows the county, frame spans the county | auto: series.rs p21_* |
| 9.17e | **P22** Cancel on page 3 leaves no PDF, no `.part` file and (PNG) an empty folder; the task disappears; the engine still works | auto: series.rs p22_* (run three times) |
| 9.17f | C3.2 PNG: one file per page named `<layout>_<NNN>_<name>.png`, all or nothing; SVG is refused for a series | auto: series.rs png_export_* |
| 9.17g | C3.2 in the app: Map Series dialog, Follows the map series, page stepping with a caption, As saved, export PDF/PNG, readable errors | auto: e2e/series.spec.ts; ui layout/series.test.ts |
| 9.17h | C3.2 edge cases: 500+ pages (refused), a layout with two following frames, a frame in another CRS than the index layer, a very long or duplicate page name, highlight of other features (not built) | to automate / not built |
| 9.17 | Map series, templates (C3), topo map builder (C4), GeoPDF | gap (not built) |

### 9A. Layout Properties pane goes blank / edits lost (bug U-5, diagnosis first)

User report 2026-10-27: in the layout editor, after selecting an element and editing its properties, the Properties pane often goes blank; the element must be selected again before a value change takes effect, sometimes several times. **Cause not known.** Reading the code (`ui/src/layout/LayoutView.tsx`, `ui/src/store/layout.ts`, `ui/src/layout/ElementProperties.tsx`, `ui/src/panes/PropertiesPane.tsx`) gives these hypotheses; the cases below separate them. Pane blank = `useApp.properties` is `null` (the "Select a layer…" hint) or the pane remounted.

| Hyp. | Idea | Where |
|---|---|---|
| H1 | Something clears `selected` (`select([])`) while the user is working: a pointer-down that reaches the canvas handler after the element's own handler, or on handles, locked elements or the page background. | `LayoutView.tsx` canvas `onPointerDown`; an effect sets `properties` to `null` when `selected.length != 1` |
| H2 | `open(id)` resets the selection (`selected: []`) whenever the active layout changes (a tab or dock activation, a second layout, a project event). | `layout.ts` `open`, `LayoutView.tsx` `activate` |
| H3 | Properties edit the *wrong document*: `update`/`edit` write to the open layout `doc`, while the pane reads `docs[layout]` when another layout is open, so the value is silently dropped until the element is reselected. | `ElementProperties.tsx` selector vs `layout.ts` `edit` |
| H4 | Another part of the app sets `properties` (layer or model selection, a run finishing, Contents click) and the layout effect doesn't re-assert it because `selected` didn't change. | `useApp.setProperties` callers |
| H5 | A re-render storm or remount (preview render, debounced save, undo-history step) resets the `Num` field state or the pane. | `Num` effect, `beginStep`/`burst`, preview effect |
| H6 | Undo/redo or `afterHistory` drops the selection after an edit. | `layout.ts` |
| H7 | A render error inside the pane is swallowed by the error boundary and shows nothing. | pane error boundary |

| ID | Check | Steps / input | Expected | Tests |
|---|---|---|---|---|
| LP-0 | **Tracer** (diagnosis tool, dev builds only; built Q3) | `?trace=layout` logs every change of `useApp.properties`, `useLayouts.selected` and `useLayouts.id` with a timestamp and the call stack, plus a mount counter for `LayoutElementProperties` and each `Num` | a trace the user can attach to a report; every blank moment has a named cause | built: `ui/src/layout/trace.ts` |
| LP-1 | Selection survives edits (store) | select one element; 30 `update` calls on x, y, scale with and without a burst key | `selected` unchanged; final value = last value; one undo step per burst | auto: store/layout.test.ts (LP-1) |
| LP-2 | Edit goes to the pane's own layout (H3) | layouts A and B; A open, select an element in A; then `open(B)`; call the pane's `set` for A's element | the edit lands in A or is refused with a message; never dropped silently | vitest (expected to fail if H3 is true) |
| LP-3 | `open()` and selection (H2) | `open(sameId)` keeps selection; `open(otherId)` clears it; reopening A restores a *sensible* selection (last selected element) | documented behaviour; the pane never shows a stale element | vitest |
| LP-4 | Pointer paths never deselect by accident (H1) | e2e: for each element kind (map frame, text, legend, scale bar, north arrow, sources) × locked / unlocked × click body / move handle / resize handle / rotate | exactly one element is selected after each click; `properties` target is that element; the canvas page-background handler does not run | e2e `layout-props.spec.ts` |
| LP-5 | Never blank while editing | e2e: select a map frame; type into Scale, Centre X, X, Width 5 times each, 100–900 ms apart; poll the pane every 50 ms | the empty-hint is never shown; every typed value is stored (compare with `useLayouts` state) | auto: e2e/layout-props.spec.ts (LP-5, one field, 3 edits; the 5-field/timing variant still to automate) |
| LP-6 | Edits while a preview renders (H5) | e2e: delay `render_layout_preview` by 3 s; edit 5 fields during the render | pane never blank; fields keep what was typed; the preview ends consistent with the last edit | e2e |
| LP-7 | Edits while a save is in flight (H5) | e2e: delay `save_layout` by 2 s; edit during it | same | e2e |
| LP-8 | Two layouts, tab switching (H2, H3) | e2e: two layout tabs; select in A, switch to B and back, click into the pane, edit | selection restored or cleanly empty with the hint; edits apply to the visible layout | e2e |
| LP-9 | Other selections steal the pane (H4) | e2e: with an element selected, click a layer in Contents, run a model, then click the element again and edit | pane follows the last click; no stale target; re-clicking the same element re-opens its properties (no need to select another first) | e2e |
| LP-10 | Number field behaviour | vitest on `Num`: type `1`, `12`, clear, `-`, `1.`, `1e3`, paste; echoed rounding (`decimals`) | text is never rewritten while typing; value commits only when valid; blur shows the rounded value | vitest `elementProps.test.tsx` |
| LP-11 | Undo/redo with focus in the pane (H6) | e2e: edit a value, press Ctrl+Z / Ctrl+Y inside the field | one burst undone; selection kept; pane not blank | e2e |
| LP-12 | Keys in the pane don't act on the canvas | e2e: Delete / Backspace / Arrow keys inside a Properties field | the element is not deleted or nudged | e2e |
| LP-13 | CRS change and Set from Map View | e2e: Change… CRS (dialog), then edit; Set from Map View, then edit | pane remains; fields show new values; edits stick | e2e |
| LP-14 | Remount budget (H5) | dev counter: mounts of `LayoutElementProperties` and each `Num` during 20 edits | zero remounts; at most 2 renders per edit | vitest + e2e |
| LP-15 | Error boundary (H7) | force a throw in a field renderer (test hook) | the pane shows its error box with the message, not blank | vitest |
| LP-16 | Regression | all of LP-4…LP-13 on the fixed build, 50 repeats in CI (flaky = fail) | 50/50 | e2e (repeat) |

**Status 2026-10-28:** cause found (H8, a new hypothesis: the dock closed the layout when Properties became active), fixed, regression tests LP-1, LP-3, LP-5; the other LP cases remain to automate. **Done when:** the tracer or a failing LP case names the cause; the fix has a regression test; the cause and fix are recorded in `docs/BUGS.md` (U-5) and `docs/DECISIONS.md`.

## 10. Downloads (TIGER, National Map, 3DEP)

| ID | Check | Coverage |
|---|---|---|
| 10.1 | Dataset tables: unique ids, labels, blocks / local roads need a county | auto: downloads::dataset_tables_are_consistent |
| 10.2 | TIGER requests validated before any request (dataset, state/county FIPS, vintage; path traversal text) | auto: downloads::tiger_requests_are_validated_before_any_download |
| 10.3 | National Map / 3DEP reject empty, inverted or NaN areas | auto: downloads::tnm_requests_reject_empty_or_garbage_areas |
| 10.4 | A refused download adds no layers, citations or lingering progress task | auto: downloads::a_refused_download_adds_no_layers_citations_or_lingering_tasks |
| 10.5 | Esri JSON → GeoJSON (holes, orphan holes, tiny rings, points, lines, nulls) | auto: downloads::esri_json_conversion_edge_cases; unit: tnm_vector |
| 10.6 | TIGER sample data: tracts, roads, blocks with POP100; years newest first; ACS year lacks blocks; older year labelled | auto (ignored, network): engine::tiger_sample_data_downloads; downloads::tiger_year_list_and_availability |
| 10.7 | National Map layers clipped to the box, geometry types right | auto (ignored, network): engine::national_map_vector_downloads |
| 10.8 | 3DEP elevation (small area, resolution ≥ 1 m, CRS UTM, citation) | auto (ignored, network): engine::download_3dep_elevation, downloads::elevation_download_small_area |
| 10.9 | Cancelling a National Map download imports nothing | auto (ignored, network): downloads::cancelling_a_national_map_download_leaves_nothing_behind |
| 10.10 | Dialogs: TIGER (state → county → years → datasets → progress bar → layers named with year/area), National Map (map view / layer extent / clip to layer), Elevation | manual M-20 |
| 10.11 | Offline behaviour: turn off Wi-Fi, start a download | readable error in the dialog, no layer | manual M-21 |

### 10A. Large-area DEM download (bug D-1, diagnosis first)

User report 2026-10-27: downloading a DEM for a large area seems not to work and causes problems (details not given). `crates/engine/src/tnm.rs::download_elevation` makes **one** `exportImage` request for the whole box, UTM zone of the box centre, at most 8000 px per side (8000 × 8000 × 4 bytes = 256 MB), 300 s timeout, and silently coarsens the resolution above that. Suspected causes (confirm with the cases below; none is verified):

| Hyp. | Idea |
|---|---|
| D-H1 | The service's own limit is lower than the 8000 px assumed (image size, or time), so a big request fails with a service error or a timeout. The real limit is not recorded anywhere. |
| D-H2 | One request means no retry, no resume, little progress, and a Cancel that is only noticed while streaming bytes (not while waiting for the service, up to 300 s). |
| D-H3 | The box is cut in one UTM zone chosen at its centre. A box wider than 6° (or crossing the equator, the poles, or ±180°) is badly distorted or filled with no-data. |
| D-H4 | The resolution is silently coarsened (`ceil(extent / 8000)`) without telling the user what they will get; the dialog hint says only "large areas use a coarser resolution". |
| D-H5 | A big raster is imported whole: overviews, the in-project copy, and memory (`raster::import`) may fail or take minutes with no progress. |
| D-H6 | The width/height estimate uses fixed 111 320 / 110 950 m per degree, which is wrong for large or polar areas. |

| ID | Check | Input | Expected | Tests |
|---|---|---|---|---|
| DEM-0 | **Reproduction ladder** (diagnosis tool) | real service: squares of 10, 50, 100, 200, 500, 1000 km at 30 m and at 90 m; log wall time, bytes, HTTP status, service message, resulting cell count and resolution | a table in `docs/DEM_DOWNLOAD.md` (new) naming where it first fails and why | engine test, ignored (network), prints the table |
| DEM-1 | Service limits are read, not guessed | `…/ImageServer?f=json` | `maxImageWidth`, `maxImageHeight`, supported formats, native resolutions are recorded; the code uses them | auto: dem_tiles::dem_1 (parser, fallback, tile edge ≤ 4000); real-service read still network |
| DEM-2 | Download plan (pure function; **built Q3**: `tnm::plan_elevation`, tests `downloads::dem_2_*`, tiles and bytes cap come with D0) | boxes from 1 km to 5000 km, resolutions 1–90 m, any latitude | `{ requested_res, used_res, cols, rows, tiles, bytes, coarsened }`; no tile above the service limit or 500 MB; `coarsened` true whenever `used_res > requested_res`; sizes within 2 % of a geodesic computation | auto: dem_tiles::dem_2_* (plan, grid snapped, tiles cover the grid, guard before any request) |
| DEM-3 | Mock service: failures | local HTTP server stands in for `exportImage` (service URL injectable): HTTP 500, 504, JSON error body with 200, truncated body, no `Content-Length`, trickling body, wrong content type | readable message each time ("USGS 3DEP returned … for tile 4 of 9"); temp files removed; no layer added; engine still works | auto: dem_tiles::dem_3 (HTTP 500/504, JSON error with 200, truncated, wrong type, not a raster, wrong size, unreachable) |
| DEM-4 | Tiling is seamless | mock serves the plane `z = a·x + b·y` for any tile; 3 × 3 tiles | the mosaic equals a single request to 1e-3 m; no gap or overlap rows at tile edges; no-data count = 0 | auto: dem_tiles::dem_4 (3 × 3 tiles equal the plane within F32 rounding, no no-data) |
| DEM-5 | Wide and special areas | boxes across a UTM zone edge (−96 … −90), across the equator, near 80° N, across ±180°, a hemisphere-sized box | one stated output CRS (policy decided in R0/D0: geographic for > 1 zone, else the local UTM zone); no wedge of no-data above 1 % of cells; correct zone numbers (lon 180 → zone 60) | auto: dem_tiles::dem_5 (EPSG:4326 for > 1 zone or beyond 84°, coarsened with a note, globe fits at ≈ 1 km) |
| DEM-6 | Cancel | cancel during tile 3 of 9, and during the wait for a tile | stops within 2 s; temp files removed; no layer | auto: dem_tiles::dem_6 (stops < 2 s during a tile and between tiles) |
| DEM-7 | Retry and resume | mock fails tile 5 twice then succeeds; another run fails tile 5 permanently | retried up to 3 times with back-off; finished tiles are not requested again; the permanent failure names the tile and keeps the finished ones for a re-run | auto: dem_tiles::dem_7 (retry 3 times, finished tiles kept, resume asks only for the rest) |
| DEM-8 | Memory and disk | 20 000 × 20 000 cells (mock, 1.6 GB of floats) | peak memory < 500 MB (streams to disk); free-space check before the download; clear message if short | partial: tiles stream to disk (one tile in memory, 64 MB at most); 20 000² perf run planned (ignored) |
| DEM-9 | Import cost | `raster::import` of an 8000 × 8000 F32 | progress reported; < 60 s (release); stored size reported; tiled/compressed output | planned (ignored perf) |
| DEM-10 | Dialog shows the truth | Elevation dialog: current view at 3 zoom levels, a 500 km layer extent | one engine command (`plan_elevation`) feeds the hint: size in MB, number of requests, resolution actually used, a warning when coarsened, and a confirm above 500 MB; the numbers equal the download's | auto: dem_tiles::dem_10 (confirm before any request), e2e/dem.spec.ts (plan hint, coarsening note, checkbox) |
| DEM-11 | Real service, large | 1° × 1° at 30 m; 5° × 5° at 90 m containing Mt Whitney (36.5785° N, −118.2923°, 4421 m) | succeeds; maximum elevation within 4300–4430 m; no-data < 1 %; < 10 min | ignored network test downloads::dem_11 (run on the maintainer machine) |
| DEM-12 | Real service, offline and down | no network; DNS failure; service returns 503 | "Could not reach …" within 30 s; no layer; progress bar clears | network + offline, extends 21 |
| DEM-13 | Result metadata | any successful job | CRS stated; NAVD88 metres; citation recorded once per job (not per tile); layer name has the area label | auto: dem_tiles::dem_13 (CRS, name, one citation per job) |

**Done when:** DEM-0's table shows the failure point; DEM-3…DEM-10 pass; DEM-11 passes against the real service on the maintainer's machine.

## 11. Sources and citations

| ID | Check | Coverage |
|---|---|---|
| 11.1 | Every import logs a citation (local file kind, incomplete); re-import adds a use, not a source; the title is the file name | auto: progress_sources::every_import_cites_its_file_and_a_reimport_adds_a_use_not_a_source; engine::data_sources_are_logged_with_citations |
| 11.2 | Edits survive re-import; Unicode / quotes round trip; ghost key ignored; remove source removes uses but keeps the layer | auto: progress_sources::editing_and_removing_citations; engine::data_sources_are_logged_with_citations |
| 11.3 | Copies inherit; derived layers cite every input once; replacing derived sources; no sources → none | auto: progress_sources::derived_layers_cite_every_input_once; engine::model_results_cite_every_input_layer |
| 11.4 | Built-in citations: basemaps, 3DEP, TNM, TIGER by year (2020 adds the Census counts; ACS has one), stable keys | auto: progress_sources::builtin_citations_are_complete_and_well_formed; engine::data_sources_are_logged_with_citations |
| 11.5 | Basemaps logged once however often used | auto: engine::data_sources_are_logged_with_citations |
| 11.6 | Layout Sources element cites drawn layers incl. derived | auto: layout::p14_sources_cite_every_drawn_layer_including_derived_ones |
| 11.7 | Citation formatting APA / MLA / Chicago, dates, missing parts (no author, no year, empty title) never print `undefined` | auto: cite.test.ts; citeEdge.test.ts > citations with missing parts |
| 11.8 | Exports: Markdown, HTML (escaped), BibTeX (unique keys), RIS; empty list; 500 sources fast; sorting rules | auto: cite.test.ts; citeEdge.test.ts > collections |
| 11.9 | Preferences (style, auto-export) persist; corrupt or unavailable storage → defaults | auto: prefs.test.ts |
| 11.10 | Sources pane: edit a citation, switch style, export files next to the project on Save, "incomplete" sorting | manual M-22 |
| 11.11 | Secrets (Census API key, Earthdata token) stored in the OS keychain only | gap — features not built (Census join ⬜) |

## 12. Project save / open

| ID | Check | Coverage |
|---|---|---|
| 12.1 | Round trip of layers (ids, names, CRS, fields, counts, definition queries), edits, rows, rasters (copied inside the project), sources (with the user's edits), layouts, UI state byte-for-byte (Unicode, escapes, 1e21), node cache (second run is `Cached`, same snapshot names) | auto: project::everything_survives_save_close_open |
| 12.2 | Edits after reopening are saved in place and survive another reopen | same |
| 12.3 | Empty project round trip | auto: project::an_empty_project_round_trips |
| 12.4 | Opening a non-project folder, a missing folder, a corrupt database: error, current session untouched, no project dir set | auto: project::opening_a_bad_folder_fails_cleanly_and_keeps_the_current_project |
| 12.5 | Save As over an existing project replaces it; New Project starts clean | auto: project::save_overwrites_a_previous_project_in_the_folder_and_new_project_starts_clean |
| 12.6 | Layer ids keep increasing after reopening (no collision) | auto: project::layer_ids_keep_increasing_after_reopening_so_they_never_collide |
| 12.7 | Project with rasters moves between folders | auto: engine::projects_carry_their_rasters_and_move_between_folders |
| 12.8 | Basic save / reopen | auto: engine::save_and_reopen_project |
| 12.9 | Local file sources are absolute paths (known limitation) | gap — documented in ROADMAP |
| 12.10 | UI: Ctrl+S / Save As dialog / Open dialog (choose a `.gproj` folder), dirty marker, unsaved-changes prompt on New / Open / quit, window title shows project name, bibliography files written on save | manual M-23 |
| 12.11 | Project written on macOS opened on Windows / Linux | manual M-24 |

## 13. Progress and cancel

| ID | Check | Coverage |
|---|---|---|
| 13.1 | Task lifetime = handle lifetime; unique ids across threads | auto: progress_sources::a_task_lives_exactly_as_long_as_its_handle, task_ids_are_unique_across_threads |
| 13.2 | Fractions clamp, never go back, NaN ignored, split into parts | auto: progress_sources::fractions_clamp_never_go_backwards_and_split_into_parts |
| 13.3 | Cancel flag: seen by that task only, "Cancelling…", stale handles are fine, context-wrapped error still recognised | auto: progress_sources::cancel_is_seen_only_by_cancellable_work_that_checks_and_only_for_that_task |
| 13.4 | Nested run flags restored | auto: progress_sources::raster_steps_stop_when_the_run_flag_is_set |
| 13.5 | Part index past the end | auto (B-T8 fixed): progress_sources::a_part_index_past_the_end_still_reports_at_most_100_percent |
| 13.6 | Model run cancelled mid-query (planar Buffer, 3 M points), result write all-or-nothing, engine reusable | auto: engine::model_run_can_be_cancelled_mid_query |
| 13.7 | GDAL raster steps stop on Cancel and clean up | auto: engine::cancelling_stops_gdal_raster_steps_and_cleans_up |
| 13.8 | Cancel for Near, Voronoi, Spatial Join (< 3 s) | auto (ignored, perf): cancel::cancel_near, cancel_voronoi, cancel_spatial_join |
| 13.9 | Cancel for geodesic Buffer and Dissolve (~12 s) | auto (B-T4 fixed, ignored for time): cancel::cancel_geodesic_buffer, cancel_dissolve, cancel_dissolve_during_the_tile_unions (each stops within 2 s); results equal the single query: batching::dissolve_in_tiles_equals_one_union, big_geodesic_buffer_equals_one_query, dissolved_geodesic_buffer_in_tiles_equals_one_query |
| 13.10 | Import in progress leaves a consistent engine | auto (ignored, perf): cancel::cancel_an_import_and_a_tile_build_leave_nothing_behind |
| 13.11 | Layout render and history diff cancel | auto: layout::cancelled_render_writes_nothing; history::h15_… |
| 13.12 | Display build cancelled by an edit | auto: display::p5_an_edit_cancels_a_running_build |
| 13.13 | Progress bar and Cancel button in the status bar; several tasks at once; error toast | manual M-25 |

## 14. Manual cases (exact steps)

Use the packaged app (`GIS Studio.app`) unless a case says "browser build" (`./target/release/devserver` + `npx vite`, <http://localhost:1420>). Test data: Data tab → Testing → *Generate Test Data* gives points/polygons without files; for files use `crates/engine/tests/fixtures/` or any shapefile.

Cases marked `auto: e2e …` run headless in `e2e/*.spec.ts` (Playwright + the devserver; `scripts/e2e.sh <out-dir>`, or `npx playwright test` with `E2E_VITE_PORT` set to a running vite). "e2e covers most of it" means the browser part is automated and the named rest stays on the checklist. The suite also fails on any uncaught console error or unexpected error pop-up. Browser: installed Google Chrome, else `/Applications/Chromium.app`, else `E2E_BROWSER_PATH`, else `npx playwright install chromium`.

- **M-1 Add Data dialog.** Map tab → Layer group → *Add Data*. (a) Select two files with Shift-click, press Open. Expect two layers, top one is the last file. (b) Choose a file in a folder whose name has spaces and `é`. (c) Press Cancel: no layer added, no error. (d) Choose a `.docx`: status bar shows an error, no layer.
- **M-2 Export Features.** Right-click a layer in Contents → *Export Features…* (or Data tab → Export). Choose GeoPackage, save over an existing file: the OS asks to overwrite. Open the result in QGIS: same feature count, CRS equals the layer's. Select 3 features first: the dialog defaults to *Selected features only (3)*; press *Choose file…*: the file has 3 features. With a definition query set, *All features* says "after the definition query" and the file has only matching features.
- **M-3 Drag and drop.** Drag a `.gpkg` from Finder/Explorer onto the map: layer added. Drag a `.shp` plus its `.dbf` together: one layer. Drag a folder: nothing added, no crash. Drag a layer from the *Catalog* pane onto *Contents* and onto the Model canvas: layer / extra Layer Input node.
- **M-4 Map CRS.** Map tab → Coordinate System. Search `26914`, pick UTM 14N: the map redraws, the status bar shows the CRS name. Pick *Web Mercator*, then a polar CRS (EPSG:3031) over a world layer: no blank map, a note says features are dropped. auto: e2e e2e/map.spec.ts::M-4
- **M-5 Navigation.** Scroll to zoom, drag to pan, double-click to zoom in, Full Extent returns. Right-click a layer in Contents → *Zoom To Layer*. With 1 M test points, panning stays smooth (Benchmark in View tab, see M-9). e2e covers most of it (e2e/map.spec.ts::M-5); still manual: the 1 M-point smoothness check.
- **M-6 Basemaps.** Map tab → Basemap gallery: USGS Topo, Imagery, Shaded Relief, OSM, None. Tiles load, attribution shows at the bottom right. Open *Sources*: the basemap appears once however often you switch. In a NAD27 map CRS the offset warning appears.
- **M-7 Coordinate readout.** Move the pointer over the map: the status bar shows coordinates in the map CRS. Move quickly: it keeps up and ends on the last position. auto: e2e e2e/map.spec.ts::M-7
- **M-8 Panes and theme.** View tab → Windows: toggle each pane, drag a pane to float and dock it, *Reset Panes* restores. View → Appearance: dark/light. e2e covers most of it (e2e/map.spec.ts::M-8); still manual: dragging a pane to float and dock it.
- **M-9 Performance.** View tab → Frame Rate shows FPS; Benchmark runs and prints numbers.
- **M-10 Map tools.** Select (Shift adds, Ctrl toggles), Select by Rectangle (drag), Identify (popup lists attributes, several features when overlapping), Measure (click points, double-click ends, the readout shows distance and azimuth; Esc clears), Measure Area, Resection and Intersection panels (enter landmarks, compare with `geodesy.test.ts` examples). auto: e2e e2e/tools.spec.ts::M-10 (2 tests)
- **M-11 Attribute table.** Open a layer's table (Feature Layer tab → Open Table). Double-click a cell, type, Enter saves, Esc cancels, Tab moves right. Sort by clicking a header (twice for descending). Type `pop > 20` in the filter box. Add Field / Calculate Field / Calculate Geometry from the ribbon: errors show in the dialog; the table refreshes. auto: e2e e2e/table.spec.ts::M-11
- **M-12 Definition query.** Feature Layer → Definition Query. In *Builder* add two rows with AND/OR; switch to *Python* and *SQL* tabs: text updates. Apply: table and map show only matches; clear restores. auto: e2e e2e/table.spec.ts::M-12
- **M-13 Run a model.** Analysis tab → Model. Add Buffer (Shift+A), connect Layer Input → Buffer → Output, press Ctrl+Enter. Progress shows "Step 1 of 3…". On a 3 M-point test layer press Cancel: stops within a few seconds, the layer shows its previous result. e2e covers most of it (e2e/model.spec.ts::M-13); still manual: Step text is checked, but the 3 M-point cancel-within-seconds run (the e2e run uses 250 k points and allows 30 s, see B-T4).
- **M-14 Node editor gestures.** Shift+A opens the Add menu at the pointer (type to search, arrows + Enter). Drag a node onto a connection: it is inserted; a node of the wrong kind is refused with a reason. Shift+drag box-selects; Ctrl(⌘)+right-drag across links cuts them; G moves, X deletes, Ctrl+X deletes with reconnect, Shift+D duplicates, Ctrl+C / V copy across layers' trees, M mutes, H collapses, F links, Home / . frames. auto: e2e e2e/model.spec.ts::M-14 (2 tests)
- **M-15 Shortcuts dialog.** Click the keyboard icon on the Model pane's toolbar; rebind Mute to X: the dialog names the conflict with Delete; Reset restores. auto: e2e e2e/model.spec.ts::M-15
- **M-16 Deleted layer.** Make a tree for layer B that uses layer A as an extra input; remove A; run B's tree: "Layer no longer exists", the node is flagged. auto: e2e e2e/model.spec.ts::M-16
- **M-17 QGIS references.** In `crates/engine/src/nodes/qgis_*.rs` every tool has a `// QGIS: … (native:id)` comment and no copied GPL code.
- **M-18 Attribute history UI.** Run a tree with Filter then Buffer. Open the Attribute Table: it shows the Output; open the node picker and choose Filter; the diff bar shows added / removed / changed chips; click a chip to filter rows; the node-card menu opens the table at that node. Save, close, reopen: pick the same node, rows load without re-running. auto: e2e e2e/history.spec.ts::M-18
- **M-19 Layout tab.** Insert tab → New Layout. Insert → Map Frame, Legend, Scale Bar, North Arrow, Text (`{scale}` replaced), Sources. Drag and resize; Ctrl+Z undoes. Map Frame → *Set from Map View*, *Frame CRS*, scale box (type `24000`). Export PDF / PNG / SVG via the dialogs; open the PDF (text selectable), print preview matches. Rename, duplicate, delete a layout from Contents. e2e covers most of it (e2e/layout.spec.ts::M-19); still manual: opening the PDF (selectable text) and print preview.
- **M-20 Download dialogs.** Data tab → *Sample Data (TIGER)*: pick Oklahoma → Cleveland → year Census 2020 → Tracts + Blocks → Download: progress bar, then layers named with the year and area. *National Map*: choose the map view, tick Streams and Highways, *clip to area*: layers stay inside the box. *Elevation (3DEP)*: small area, 10 m: raster appears with a hillshade-friendly colour ramp.
- **M-21 Offline.** Disconnect the network and start any download: the dialog shows "Could not reach …", no layer, progress bar disappears.
- **M-22 Sources pane.** View → Sources. Edit author/year of a local file, switch APA ↔ MLA ↔ Chicago, export Markdown/HTML/BibTeX/RIS and open them. After *Save*, the bibliography files appear beside the project when auto-export is on. e2e covers most of it (e2e/misc.spec.ts::M-22); still manual: exporting Markdown/HTML/BibTeX/RIS (native folder dialog) and the auto-export beside the project.
- **M-23 Save / Open.** Ctrl+S on a new project asks for a folder name (`.gproj`); title bar shows it; edit something: dirty marker; Open another project with unsaved changes: prompt. Quit with unsaved changes: prompt. e2e covers most of it (e2e/misc.spec.ts::M-23); still manual: Ctrl+S on a new project (native folder dialog) and the quit prompt in the desktop shell; the browser build covers discard, save, dirty marker and unload prompt.
- **M-24 Cross-platform.** Copy a saved `.gproj` (with a raster) to Windows and Linux; open it: layers, raster, layouts, history all load. Mark a platform verified in `ROADMAP.md` only after this ran.
- **M-25 Progress UI.** Start two downloads together: two bars; Cancel one; the other continues. A failing task leaves a readable message in the status bar. e2e covers most of it (e2e/misc.spec.ts::M-25); still manual: two downloads at once (network); the engine runs one model at a time.
- **M-26 Installers (A3).** Windows: run the NSIS and MSI installers on a clean machine, start the app, run `--self-test`; uninstall removes it. Linux: run the AppImage and install the `.deb`; start and `--self-test`. macOS: mount the DMG, drag to Applications, first launch with Gatekeeper, `--self-test` under `env -i`.

---

## Coverage summary (area → cases → new tests)

Counts are tests, not cases; "new" is added by the test-coverage work of 2026-10-25.

Totals, macOS arm64 (before → after): `cargo test -p engine` 95 passed / 8 ignored → 174 passed / 24 ignored (0 failed); `npx vitest run` 13 files, 86 tests → 26 files, 295 tests; `npx tsc` clean. The 16 new ignored engine tests are 3 network, 4 slow (cancel latency, import), and 9 that fail today because of the 8 bugs in `docs/BUGS.md`.

| Area | Cases (auto / manual / gap) | New automated tests |
|---|---|---|
| Import / export | 21 / 3 / 0 | `formats.rs` (11) |
| CRS | 17 / 1 / 0 | `crs_edge.rs` (7), `crsEdge.test.ts`, `scaleEdge.test.ts` |
| Layout Properties pane, bug U-5 (§9A) | 0 / 0 / 0; 17 diagnostic and regression cases planned in Q3 (LP-0…LP-16) | planned: `layoutStore.test.ts`, `elementProps.test.tsx`, `e2e/layout-props.spec.ts` |
| Large-area DEM download, bug D-1 (§10A) | 0 / 0 / 0; 14 cases planned in Q3 (guard) and D0 (tiled download) (DEM-0…DEM-13) | planned: `downloads.rs` + mock service, `ElevationDialog` tests |
| CRS hardening (§2A, Phase R) | 0 / 0 / 0; 112 planned in R1–R8 (A 9, B 9, C 7, D 8, E 8, F 12, G 10, H 12, I 6, J 9, K 6, W 12 = 108, plus 4 manual) | planned: `crs_reference.rs`, `crs_matrix.rs`, `crsZoo.test.ts`, `e2e/crs.spec.ts` |
| Map view | 15 / 5 / 0 | `styleEdge.test.ts`, `scaleEdge.test.ts` |
| Selection / identify / measure | 17 / 1 / 0 | `tables.rs` (selection part), `geodesyEdge.test.ts`, `navStore.test.ts`, `appStore.test.ts` |
| Table / editing | 19 / 3 / 0 | `tables.rs` (17), `queryEdge.test.ts` |
| Node editor | 21 / 4 / 0 | `modelStore.test.ts`, `keymapStore.test.ts`, `tools::every_tool_spec_is_well_formed` |
| Tools | 51 tools: 51 / 1 / 1 | `tools.rs` (22), `rasters.rs` (9) |
| Attribute history | 11 / 1 / 1 | none needed |
| Layouts | 15 / 1 / 1 | `layout/types.test.ts`; project round trip |
| Downloads | 9 / 2 / 0 | `downloads.rs` (8, 3 ignored) |
| Sources / citations | 10 / 1 / 1 | `progress_sources.rs` (4), `citeEdge.test.ts`, `prefs.test.ts` |
| Project save / open | 8 / 2 / 1 | `project.rs` (5) |
| Progress / cancel | 12 / 1 / 0 | `progress_sources.rs` (6, 1 ignored), `cancel.rs` (6, all ignored) |

## D1. Generic ArcGIS REST catalog (phase D1)

| Case | What | Expected | Status |
|---|---|---|---|
| D1-1 | Pasted service addresses | trailing `/`, `?f=json`, layer number, `/query` removed; non-https and non-ArcGIS refused in words | auto: arcgis::d1_service_addresses… |
| D1-2 | Catalog integrity | unique ids, https services, every citation field filled, `complete` citations | auto: arcgis::d1_the_catalog… |
| D1-3 | Layers of a service | group layers left out, geometry types shown | auto: arcgis::d1_layers… |
| D1-4 | Paging and conversion | 25 features over 3 pages + count; Esri dates → ISO; attributes kept; layer named and cited as a Web service (stub to complete) | auto: arcgis::d1_a_custom_layer… |
| D1-5 | Layer by number or name | "1" and "#0" work; unknown name says so; missing layer asks to choose | auto: arcgis::d1_a_layer_can_be_named… |
| D1-6 | Several layers | merged with CATEGORY | auto: arcgis::d1_several_layers… |
| D1-7 | Empty area, JSON error, HTTP 500, unreachable, not ArcGIS | nothing added / readable sentence naming the host | auto: arcgis::d1_an_empty_area… |
| D1-8 | Real services | every catalog entry answers a small query | ignored network: arcgis::d1_real_catalog_entries_answer |
| D1-9 | Dialog | catalog, source and licence shown before download; bad address explained; Download disabled until a layer is chosen | auto: e2e/arcgis.spec.ts |

## D2. Census data join and keychain (phase D2)

| Case | What | Expected | Status |
|---|---|---|---|
| D2-1 | Variable codes | upper-cased, de-duplicated; spaces, `;`, > 40 refused before any request | auto: census::d2_variable_codes…; ui census/presets.test.ts |
| D2-2 | Tract join across two states | one request per state; values by GEOID; no feature lost; unmatched and suppressed counted; Bureau markers (−666666666) stored as no value; source layer untouched; cited with the Census disclaimer; inherits the source's citations | auto: census::d2_tracts… |
| D2-3 | Joining twice | refused: never overwrites a field | auto: census::d2_tracts… |
| D2-4 | GEOIDs stored as numbers | leading zero restored (1 → 01); level guessed from length (2/5/11/12); odd length asks for the level | auto: census::d2_numbers… |
| D2-5 | Failures | unknown variable, wrong key, missing year, unreachable, missing field: sentences; the key never appears in a message | auto: census::d2_failures… |
| D2-6 | Real API | Oklahoma County population > 600 000 | ignored network: census::d2_real_api… |
| D2-7 | Dialog | GEOID field guessed, presets per dataset, bad codes block Join, years per dataset, the browser build says it has no keychain | auto: e2e/census.spec.ts |
| D2-8 | Keychain (manual, per OS) | Save a key; restart the app; the dialog says a key is saved and a join uses it (no key in the request log, project file or error text); Remove deletes it | manual M-31 (macOS, Windows, Linux); not verified on any OS yet |

## D3. STAC search and COG clip (phase D3)

| Case | What | Expected | Status |
|---|---|---|---|
| D3-1 | Addresses, dates, catalog | `s3://` public buckets → https; `gs://` refused; date forms checked; catalog citations complete | auto: stac::d3_hrefs_dates… |
| D3-2 | Search request and result | collection, dates, cloud limit, newest first sent; only GeoTIFF assets with a readable address listed | auto: stac::d3_search_sends… |
| D3-3 | Search errors | HTTP 400 with the service's message, not a STAC answer, unreachable, bad area/date/collection | auto: stac::d3_search_errors… |
| D3-4 | Clip | reads only the area through HTTP Range, in the image's CRS, cell values and bands intact, cited with the scene id | auto: stac::d3_a_clip_reads… |
| D3-5 | Clip refusals | box outside the image, too many cells (size named), missing file, unsupported address | auto: stac::d3_clip_refusals… |
| D3-6 | Real services | Copernicus DEM near Norman, OK is 250–450 m | ignored network: stac::d3_real… |
| D3-7 | Dialog | catalog listed, Download disabled until a scene is chosen, bad date explained | auto: e2e/stac.spec.ts |

## D4. Overture Maps GeoParquet (phase D4)

| Case | What | Expected | Status |
|---|---|---|---|
| D4-1 | Themes and files | six themes, S3 glob per release/theme/type, citation with licence naming ODbL/CDLA | auto: overture::d4_themes… |
| D4-2 | GeoParquet over HTTP | only the rows in the box; flat fields (id, name, class, height…); fields the file lacks left out; geometry as GEOMETRY or as WKB; cited | auto: overture::d4_a_geoparquet… |
| D4-3 | Errors | empty area, empty box, unreadable file in words | auto: overture::d4_empty_areas… |
| D4-4 | Real release | buildings in a few blocks of Norman, OK (257 s on the maintainer's machine) | ignored network: overture::d4_real… |
| D4-5 | Dialog | themes, area, time and licence notice | auto: e2e/overture.spec.ts |
| D4-6 | Local GeoParquet files | import by file (existing `parquet_source`) | covered in formats tests |

## D5. OGC API – Features (phase D5)

| Case | What | Expected | Status |
|---|---|---|---|
| D5-1 | Addresses and catalog | trailing `/`, `/collections…` tails and queries removed; non-https refused; catalog citations complete | auto: ogcapi::d5_addresses… |
| D5-2 | Collections | only collections with an items link are listed | auto: ogcapi::d5_collections… |
| D5-3 | Paging | `next` links (relative or absolute) followed to the end; features without geometry skipped; layer cited as a Web service (stub to complete) | auto: ogcapi::d5_pages… |
| D5-4 | Errors | empty area → no layer; HTTP 400 with the service's message; 503 retried then "isn't responding"; 404 collection; odd collection name; missing choice | auto: ogcapi::d5_empty_areas… |
| D5-5 | Real service | USGS Water Data monitoring locations near Norman, OK | ignored network: ogcapi::d5_real… (passes) |
| D5-6 | Dialog | catalog, source and licence before download; bad address explained; Download disabled until a collection is chosen | auto: e2e/ogc.spec.ts |

## D6. Earthquakes and soils (phase D6)

| Case | What | Expected | Status |
|---|---|---|---|
| D6-1 | Earthquakes | box, dates and smallest magnitude sent; times as dates; depth kept; cited | auto: jsonapis::d6_earthquakes… |
| D6-2 | Earthquake inputs and failures | no events → no layer; bad dates/magnitude/area refused before any request; HTTP 400 text shown; 20 000+ events asks for a smaller query; not GeoJSON; unreachable | auto: jsonapis::d6_earthquake_inputs… |
| D6-3 | Soil map units | polygon query to Soil Data Access; mukey, symbol, name; cited | auto: jsonapis::d6_soil_map_units… |
| D6-4 | Soil refusals | area over 0.25° refused before any request; empty body = nothing there; bad table; HTTP 500 | auto: jsonapis::d6_soil_refusals… |
| D6-5 | Real services | Oklahoma magnitude 3+ since 2024; soil units in Norman | ignored network: jsonapis::d6_real_services (passes) |
| D6-6 | Dialog | soils blocked for a large area, quake date errors explained | auto: e2e/hazards.spec.ts |
| D6-7 | WMS / WMTS display, NASA POWER | not built (see DECISIONS D6) | deferred |
