# 4. Geoprocessing tools (`crates/engine/src/nodes/*.rs`)

Existing known-result cases per tool: TP §7. This file adds (a) generic cases that must hold for **every** tool, to automate as one parametrised test over `tool_specs` and (b) tool-specific edge cases.

Tool ids (from `ToolSpec`): vector – `input_layer`, `layer_output`, `add_to_map`, `filter`, `buffer`, `clip`, `erase`, `intersect`, `union`, `symmetrical_difference`, `dissolve`, `collect`, `merge`, `explode`, `repair`, `simplify`, `project`, `spatial_join`, `attribute_join` (Join Field), `join_nearest`, `calculate_field`, `calculate_geometry`, `select_by_location`, `summarize_within`, `summary_statistics`, `near`, `fishnet`, `feature_to_point`, `bounding_geometry`, `concave_hull`, `delaunay`, `voronoi`, `polygons_to_lines`, `lines_to_polygons`, `extract_vertices`, `points_to_path`, `points_along_lines`, `line_intersections`, `random_points`, `delete_duplicates`, `remove_null`, `export`; raster – `contours`, `hillshade`, `slope`, `aspect`, `ruggedness`, `clip_raster`, `warp`, `polygonize`, `sample_raster`, `zonal_statistics`. (51 specs; pending: split by attribute B6, rasterize, reclassify, mosaic, raster calculator, hydrology, interpolation, spatial stats.)

## 4.1 Generic cases (every tool)

| ID | Pre | Steps | Expected (user-facing) | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| TOOL-G01 | Each spec | Tool spec well-formed: unique ids, defaults valid, docs cite a QGIS id | pass | E | auto: TP 6.18, 7 (QGIS comment manual M-17) | P1 |
| TOOL-G02 | Each vector tool | Input has no CRS | Error contains "coordinate system"; tells to Define Projection | E | partial: TP 2.5 (Buffer, Project, Near, Spatial Join only); to automate for all | P0 |
| TOOL-G03 | Each vector tool | Input has 0 features | Output 0 features with the correct field list and CRS; no error | E | to automate | P0 |
| TOOL-G04 | Each vector tool | Input of the wrong geometry type (points to Dissolve-needs-polygons, lines to Fishnet) | Sentence "X needs polygon input, got points" or sensible output | E | partial; to automate | P0 |
| TOOL-G05 | Each | Input is a `Table` (no geometry) | Refused for geometry tools; allowed for field tools | E | to automate | P0 |
| TOOL-G06 | Each | Input with invalid geometry (bow-tie, 2-vertex ring, empty polygon, NaN coordinate, Inf) | Result computed or invalid count reported; never `GEOSException`/`TopologyException` text | E | to automate (fixture of 8 bad geometries) | P0 |
| TOOL-G07 | Each | Input with null geometries mixed in | Null rows kept or dropped per documented rule, count in the node message | E | to automate | P0 |
| TOOL-G08 | Each | Input crossing ±180°; on the pole; on the equator/prime meridian | No wrap artefacts; areas finite | E | to automate (geometry sets) | P0 |
| TOOL-G09 | Each | Tiny input (1 feature; 1 vertex; coordinates 1e-9 apart) | Runs | E | to automate | P1 |
| TOOL-G10 | Each | Huge input (1 M features) | Completes within the performance budget, memory bounded, cancel works | E (perf) | partial: TP 13.6–13.9 (several tools) | P1 |
| TOOL-G11 | Each | Output field names: duplicates between two inputs (join, merge, spatial join), reserved names, > 10 chars | Unique suffixes (`name_1`); documented | E | partial: TP 7 | P1 |
| TOOL-G12 | Each | Parameter garbage: missing, NaN, negative where positive needed, bad enum, 1e300, text with quotes/SQL | Error names the setting ("Distance must be a positive number") | E | partial: TP 6.17 (one case); to automate across all specs | P0 |
| TOOL-G13 | Each | Mixed CRS inputs on two-input tools | Second reprojected to first's CRS, `reprojected` noted | E | partial: TP 2.14 | P1 |
| TOOL-G14 | Each | Result CRS equals first input CRS (or documented target) | pass | E | partial: TP 7 header | P0 |
| TOOL-G15 | Each | Result source/citation inherits all inputs | pass | E | partial: TP 11.3 | P1 |
| TOOL-G16 | Each | Run twice → second `Cached`; param change → recomputed | pass | E | partial: TP 6.15 | P1 |
| TOOL-G17 | Each | Cancel mid-run | Output untouched, node status "Cancelled" | E | partial: TP 13.6–13.9 (7 of ~50 tools) | P1 |
| TOOL-G18 | Each | Output Layer name & UI labels: unicode layer names, 200 char names | Handled; no SQL breakage | E | to automate | P1 |
| TOOL-G19 | Each | Node error text never contains `__node_`, `duckdb`, `Binder Error`, `Catalog Error`, `ST_` function dumps, Rust panics, `Caused by:` | Assert on all error strings produced by G02–G12 | E | partial: TP 7 Filter; to automate as a global assertion | P0 |
| TOOL-G20 | Each | Two-input tools with same layer on both ports | Defined result (e.g. Erase self = empty) | E | to automate | P1 |
| TOOL-G21 | Each | Run on a layer with a definition query / selected-only flag | Honours the query | E | partial: TP 1.20, 4.4 | P1 |
| TOOL-G22 | Each | Fields with unicode, spaces, quotes in names pass through | Preserved | E | partial: TP 1.11 | P1 |
| TOOL-G23 | Windows/Linux | All of G01–G14 on each OS in CI | same results | E | partial: CI runs `cargo test -p engine --release` on macOS, Linux and Windows (`.github/workflows/build.yml`); the parametrised G-tests would run there too | P1 |

## 4.2 Tool-specific edge cases

| ID | Tool | Case | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| TOOL-BUF-01 | Buffer | Distance 0; negative (erode polygons); 1e7 m geodesic; points on the pole/antimeridian; segments param 1 and 1000; dissolve with 100 k overlaps | 0 → copy; negative on points → empty with note; geodesic around ±180 closes; no self-intersection | E | partial: TP 7 Buffer; to automate | P0 |
| TOOL-BUF-02 | Buffer | Geographic CRS planar vs geodesic default: 1 km at 60°N | Geodesic default; planar warns "degrees" | E | partial: TP 7 | P0 |
| TOOL-CLP-01 | Clip/Erase/Intersect | Mask empty, mask with holes, mask = input, mask disjoint, mask a line | Empty result with 0 features and note; line mask clipped to lines | E | partial; to automate | P0 |
| TOOL-CLP-02 | Clip | Result has touching pieces / slivers < 1e-9 | Dropped with count | E | to automate | P1 |
| TOOL-UNI-01 | Union / SymDiff | Overlay of 100 k polygons (perf), inputs with multi-parts | Finish; area conserved (A+B−overlap) | E (perf) | partial: TP 7 | P1 |
| TOOL-DIS-01 | Dissolve / Collect | Group field with NULL, empty string, 100 k groups, text/number mix | NULL group is its own group; counts right | E | partial: TP 7; to automate NULL | P0 |
| TOOL-MRG-01 | Merge | Different CRS, different geometry types, different field types for same name | Reprojected; Mixed; type widened to text with note | E | partial; to automate | P1 |
| TOOL-EXP-01 | Explode | GeometryCollection, empty parts | Parts exploded; empty dropped | E | to automate | P1 |
| TOOL-REP-01 | Repair | Already valid; unrepairable (NaN); collapse to line | Same / dropped with count / reported | E | partial: TP 7 | P0 |
| TOOL-SIM-01 | Simplify | Tolerance 0, huge (collapses polygon), topology (shared borders gap) | 0 copy; collapsed polygons dropped with count; documented that shared borders may open | E | partial: TP 7 | P1 |
| TOOL-PRJ-01 | Project | Target = source; to Mercator with lat 89.9/90/−90; to ortho with far hemisphere; to a CRS with a grid unavailable | Identity no-op; unprojectable features dropped with count; no NaN | E | partial: TP 7, 3.4 | P0 |
| TOOL-PRJ-02 | Project | Round-trip 4326→UTM→4326 for 1 M points < 1 mm | pass | E (perf) | partial: TP 7 | P1 |
| TOOL-SJN-01 | Spatial Join | Predicates: intersects, contains, within, touches, nearest; one-to-many (first/count/sum) | Counts by predicate right; many-to-one rule documented | E | partial: TP 7 | P1 |
| TOOL-AJN-01 | Join Field | Key type mismatch (int vs text), NULL keys, duplicate keys in the join table, case differences, whitespace | Documented matching; duplicates message; unmatched counted | E | partial: TP 7; to automate | P0 |
| TOOL-CFD-01 | Calculate Field | See TBL-09; target field exists with another type | Type conflict message | E | partial | P1 |
| TOOL-CGM-01 | Calculate Geometry | Area of polygon crossing antimeridian; length of line with Z; unit list incl. `usft` | Geodesic; units converted | E | partial: TP 7 | P1 |
| TOOL-SBL-01 | Select by Location | Distance negative; mask with no CRS | Message | E | partial | P1 |
| TOOL-SUM-01 | Summarize Within / Summary Statistics | Stat on text field, all NULL, huge, field absent; case field NULL | Min/max on text ok; sum on text refused with sentence | E | partial: TP 7 | P0 |
| TOOL-NEA-01 | Near / Join Nearest | Radius 0, unlimited, units; near set empty; near across antimeridian | Empty near set → NULL distances; antimeridian geodesic right | E | partial: TP 7 | P0 |
| TOOL-FSH-01 | Fishnet | Cell size 0, negative, > extent, 1e-9 (billions of cells), hexagons at poles | "Would create N cells; limit is M" refusal; no OOM | E | partial: TP 7 (small cases); to automate guard | P0 |
| TOOL-F2P-01 | Feature to Point | Inside vs centroid for concave/multi-part; empty geometry | Inside point lies inside | E | partial | P1 |
| TOOL-BND-01 | Bounding Geometry / Concave Hull | 1 point, 2 points, collinear, ratio 0, ratio 1; group field | Envelope degenerates handled (line/point) with note | E | partial: TP 7 | P1 |
| TOOL-DEL-01 | Delaunay / Voronoi | < 3 points, duplicates, collinear, 1 M points, points on antimeridian | "Needs at least 3 distinct points"; cancel < 3 s | E | partial: TP 7, 13.8 | P0 |
| TOOL-LIN-01 | Polygons↔Lines, Vertices, Path, Along, Line Intersections | Self-intersecting line, 0-length line, polygons with holes, points to path ordering field missing/NULL, spacing 0 / > length | Documented; spacing 0 refused | E | partial: TP 7 | P1 |
| TOOL-RND-01 | Random Points | Count 0, negative, 1e9; seed repeatability; polygon with hole; tiny polygon | Seeded repeatable; points inside; huge refused | E | partial: TP 7 | P1 |
| TOOL-DUP-01 | Delete Duplicates / Remove Null | Duplicates by geometry only vs attributes; 1 M rows | Count of removed in message | E | partial: TP 7 | P1 |
| TOOL-EXPT-01 | Export node | See EXP-*; relative path, `~`, env vars | Absolute required or resolved against project; message | E | to automate | P1 |
| TOOL-CTR-01 | Contours | Raster with NoData holes, flat, constant, interval tiny (millions of lines), base outside range, antimeridian raster | Interval too small → refusal with estimate; flat → 0 lines with note | E | partial: TP 7 | P0 |
| TOOL-HLS-01 | Hillshade/Slope/Aspect/Ruggedness | Geographic CRS (degrees → metres scale), z-factor 0/negative, NoData edges, 3×3 raster, 1×1 | Scale handled (or documented); tiny raster "too small" | E | partial: TP 7 | P0 |
| TOOL-CLR-01 | Clip Raster by Extent | Extent outside raster, partially outside, inverted, in another CRS, NaN | Outside: "extent does not overlap the raster"; other CRS reprojected | E | partial: TP 7 | P0 |
| TOOL-WRP-01 | Warp | Target resolution 0, giant output (1 M × 1 M), pole/antimeridian, resampling modes, source with NoData | Output size cap; no crash (B-T5 class) | E | partial: TP 7 (B-T5/B-T7) | P0 |
| TOOL-PLG-01 | Polygonize | 1 M distinct values (noise), diagonal connectivity, NoData | Progress/cancel; limit message | E (perf) | partial: TP 7 | P1 |
| TOOL-SMP-01 | Sample Raster | Points outside, on NoData, in another CRS, lines/polygons input | NULL for outside; documented geometry handling | E | partial: TP 7 | P1 |
| TOOL-ZON-01 | Zonal Statistics | Zones smaller than a pixel, overlapping zones, zone entirely outside, categorical raster | Tiny zones get centre-pixel or NULL (documented) | E | partial: TP 7 | P1 |
| TOOL-RSV-01 | Pending tools (B6 split by attribute, rasterize, reclassify, mosaic, calculator, hydrology, interpolation, spatial statistics, remote sensing) | Add the G01–G22 rows when each lands | — | — | pending feature | P2 |
