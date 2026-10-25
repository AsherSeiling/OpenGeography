# Test plan

One row per case for every feature in `docs/ROADMAP.md`. Last updated 2026-10-25 (macOS arm64).

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
| 1.21 | Export never overwrites the layer's own source | export to the imported path | refused | auto (ignored, BUG B-T1): formats::exporting_never_overwrites_the_layers_own_source_file |
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
| 2.6 | Raster tools refuse a CRS-less raster | Warp | Error | auto (ignored, BUG B-T7): rasters::warping_a_raster_of_unknown_crs_is_refused_not_guessed |
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
| Warp error paths | invalid CRS; raster without CRS | auto (ignored, BUG B-T5 crash, B-T7): rasters::warp_to_an_invalid_crs_is_an_error_not_a_crash, warping_a_raster_of_unknown_crs_is_refused_not_guessed |
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
| 8.13 | Undoing a node edit can drop `lastOutput` (known issue 2) | gap — documented in PROJECT_STATUS |

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
| 9.17 | Map series, templates (C3), topo map builder (C4), GeoPDF | gap (not built) |

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
| 13.9 | Cancel for geodesic Buffer and Dissolve (~12 s) | auto (ignored, BUG B-T4): cancel::cancel_geodesic_buffer, cancel_dissolve |
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
