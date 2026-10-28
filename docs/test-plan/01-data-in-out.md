# 1. Data in and out: imports, exports, catalog, downloads

Conventions (levels, status words, priorities) are in [`../TEST_PLAN_FULL.md`](../TEST_PLAN_FULL.md#conventions). "TP n.m" = a case in [`../TEST_PLAN.md`](../TEST_PLAN.md), kept as is and not repeated here. Fixtures named `fx:` live in `crates/engine/tests/fixtures/` or are generated in the test.

## 1.1 Vector and table import (`import_files`, `Engine::import`)

Existing coverage: TP 1.1–1.15, 1.22–1.24.

| ID | Pre | Steps | Expected (user-facing) | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| IMP-01 | Empty project | Import a GeoJSON with 1 feature, 1 vertex per line (a 1-point layer) | Layer added; extent is a degenerate box; zoom-to-layer does not divide by zero | E+2E | to automate: formats.rs `single_point_layer_has_a_usable_extent` | P1 |
| IMP-02 | — | Import a GeoJSON with coordinates exactly at ±180 / ±90 (antimeridian and pole) | Imported unchanged; extent −180…180; no "out of range" error | E | to automate: formats.rs | P0 |
| IMP-03 | — | Import GeoJSON whose coordinates are in a projected CRS but with no `crs` member (metres, x≈500000) | Treated as WGS 84 per spec **with a warning** that values look projected (suggestions list UTM); never silently drawn | E | to automate: crs_edge.rs | P0 |
| IMP-04 | — | Import a polygon file with an invalid geometry (bow-tie, self-touching ring, duplicate vertices) | Layer imports; `Repair` fixes it; tile build and area do not throw; invalid count is not hidden | E | to automate: formats.rs `invalid_polygons_import_and_render` | P0 |
| IMP-05 | — | Import a file with 3D / measured (Z, M) coordinates | Imports; Z kept or dropped consistently; tools run; message says if Z is dropped | E | to automate | P1 |
| IMP-06 | — | Import a file with 200 fields, one 10 000-char text value, a field named `geom`, `fid`, `__fid`, `select`, `a.b`, `"quoted"` | Imports; reserved names renamed with a notice; table pane renders; edits round-trip | E | partial: TP 1.19, 1.11; to automate reserved-name import | P1 |
| IMP-07 | — | Import 5 M points (GeoPackage, generated) | Progress bar moves, Cancel works (see PRG-04), memory stays bounded, UI responsive; first tile < 5 s afterwards | E (perf) | to automate (ignored, perf): formats.rs `five_million_points` | P1 |
| IMP-08 | — | Import a 1-byte file named `.shp`, `.geojson`, `.gpkg`, `.csv`, `.tif` | Readable error per case ("Could not read … as …"); no layer; no citation; no lingering progress task | E | partial: TP 1.14, 1.15 (shp/gpkg/json); to automate the rest | P0 |
| IMP-09 | — | Import a path that is a folder, a symlink to a missing file, a path with a trailing space | Error says "not a file" / "File not found"; engine still usable | E | to automate: formats.rs | P1 |
| IMP-10 | — | Import the same file twice, then a copy under another name | Two layers; one source for the first two, distinct citation for the copy; names de-duplicated ("name (2)") | E | partial: TP 11.1; to automate naming | P1 |
| IMP-11 | — | Import a file while another import runs | Both finish, layer ids unique, progress shows two tasks (see CON-01) | E | to automate | P1 |
| IMP-12 | Windows/macOS/Linux | Import from paths with spaces, `é`, CJK, emoji, a 240-char path, a UNC/`\\?\` path (Windows) | Imports; layer name and citation title show the file name unchanged | E+PK | partial: TP 1.11 (values only); to automate path matrix (XPL-01) | P0 |
| IMP-13 | — | Import a CSV with 1 M rows, with BOM, CRLF, `;` delimiter, quoted newlines, a ragged row, duplicate header names, empty header | Rows counted exactly; BOM stripped; delimiter detected or a readable message; duplicate headers made unique | E | partial: TP 1.6–1.8; to automate | P1 |
| IMP-14 | — | CSV with lat/lon swapped (lat 200), lat/lon out of range, text in a coordinate column, empty coordinates | Row-level problems reported as a count ("37 rows have no valid coordinates"); valid rows kept; layer not silently emptied | E | to automate: formats.rs | P0 |
| IMP-15 | — | Shapefile with `.prj` present but unreadable/empty, with a `.cpg` of an unknown code page, with `.dbf` missing | CRS "undefined" with warning (never guessed); odd code page falls back with a notice; missing `.dbf` gives a readable error | E | partial: TP 1.3; to automate | P1 |
| IMP-16 | — | Import a KML/KMZ, GPX, GML, DXF, MapInfo, ESRI File GDB folder (formats GDAL reads but the dialog may not list) | Either imports or "format not supported" naming the extension; never a raw GDAL error | E+M | to automate (list the supported set in one test) | P2 |
| IMP-17 | — | Import a GPKG with a table that has no `gpkg_contents` geometry columns and a raster tile table | Table becomes `Table`; unsupported items are listed ("skipped 1 raster table") | E | to automate | P2 |
| IMP-18 | — | Import when the project folder (DuckDB file) is read-only or the disk is full | Readable "could not write the project database" error, no half layer | E | to automate (read-only dir) / manual (disk full) | P2 |
| IMP-19 | — | Add Data dialog: choose a file the OS lets you pick but whose permissions deny reading | Error pop-up shows "Permission denied: <file name>", not an errno | M | manual M-1 extension | P2 |
| IMP-20 | — | Dropping a `.gproj` folder or `.zip` on the app | Folder: opens as project or says what it is; zip: "unzip first" (or supported); never a blank no-op | M | manual (extend M-3) | P2 |

## 1.2 Raster import

Existing: TP 1.10, 1.15, 2.6, 4.9, 9.12.

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| RAS-01 | — | Import a 1×1 pixel GeoTIFF; a 1×N strip; a 20 000×20 000 float32 (1.6 GB) | 1×1 and strips draw (no divide by zero); huge raster imports lazily or errors with "too large for memory", never an OOM crash | E (perf) | to automate (ignored, perf) | P1 |
| RAS-02 | — | Raster whose pixel size is negative-Y, rotated (non-zero rotation terms), or with a non-square pixel | Drawn in the right place or "rotated rasters are not supported" | E | to automate | P1 |
| RAS-03 | — | Raster that crosses the antimeridian (extent 170…190° in a lon/lat CRS) and one at the pole | Both halves draw; Identify at 179.9° and −179.9° returns values; no wrapping seam | E | to automate | P0 |
| RAS-04 | — | Raster all NoData; NoData = NaN; no NoData tag; integer raster with a NoData of −9999 | Stats say "no valid cells" (not min=max=NaN printed); tiles transparent; Identify returns none | E | partial: TP 7 Zonal/Polygonize; to automate stats | P1 |
| RAS-05 | — | Multi-band (RGB, 4-band, 224-band) and paletted rasters | RGB draws as colour; others use band 1 with a message naming the band used | E | to automate | P1 |
| RAS-06 | — | Raster with a CRS that has an axis order of northing/easting (EPSG:3035, EPSG:2193) | Extent correct, tile aligns with a vector in the same area | E | to automate | P1 |
| RAS-07 | — | Import the same raster twice, delete one layer | Project copy of the raster is not deleted while another layer uses it | E | to automate | P1 |
| RAS-08 | — | Compressed (LZW, deflate, JPEG, COG), BigTIFF, `.vrt`, `.img`, `.asc`, `.nc` | Imports or "unsupported" with extension named | E | to automate | P2 |

## 1.3 Export (`Export` tool, Export Features dialog)

Existing: TP 1.16–1.21, 1.23.

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| EXP-01 | — | Export a layer with no CRS to GPKG/Shapefile/GeoJSON | Written without CRS and **warns** "no coordinate system"; reopens as undefined, never as 4326 | E | to automate | P0 |
| EXP-02 | — | Export field names > 10 chars, non-ASCII, duplicates after truncation to Shapefile | Names truncated uniquely (`name_1`) with a notice listing the mapping; DBF encoding UTF-8 `.cpg` written | E | to automate | P0 |
| EXP-03 | — | Export a field of type date/boolean/int64 > 2³¹ to Shapefile, GeoJSON, CSV | Round-trips or a notice of the type change; no overflow | E | to automate | P1 |
| EXP-04 | — | Export Mixed geometry types to Shapefile and GPKG | Shapefile: refused or split with a message; GPKG: written as generic geometry | E | to automate | P1 |
| EXP-05 | — | Export a layer with null geometries and a layer with 0 features | Written; reopens with the same count; 0-feature export gives an explicit notice, not an error | E | to automate | P1 |
| EXP-06 | — | Export to a path with spaces / unicode / read-only folder / existing file in use / a path ending in `/` | Success, or "could not write <folder>: <reason>"; existing file replaced atomically (no half file on failure) | E+M | partial: TP 1.18; to automate atomic replace | P0 |
| EXP-07 | — | Export during a definition query + selection ("selected only") + a Layer Input with `fids` | Rows exported equal rows shown; message states how many were exported | E | partial: TP 1.20 | P1 |
| EXP-08 | — | Export 1 M polygons to GeoPackage; cancel at 50 % | Progress shown; cancel leaves no partial file | E (perf) | to automate | P1 |
| EXP-09 | — | Export to GeoPackage with a layer name containing quotes or `/`; to a file with the same name as the layer's source via a symlink / different case on macOS | Safe name; self-overwrite refused also through symlinks and case-insensitive paths | E | partial: TP 1.21 (plain path only); to automate symlink/case | P0 |
| EXP-10 | — | Export CRS-edge: layer in EPSG:2277 (ftUS), a bare PROJ string CRS, a compound CRS | CRS written in a form QGIS reads; numbers unmoved | E | to automate | P1 |
| EXP-11 | — | Export from the node tree with a failed upstream step | Export node shows the upstream error; no file | E | to automate | P1 |
| EXP-12 | — | `save_text` (bibliography, layout side files) to a missing folder / read-only / unicode name | Readable error; no exception text | E | to automate | P2 |

## 1.4 Catalog pane and file system (`fs_roots`, `fs_list`, `fs_stat`, `fs_inspect`)

No automated tests exist for these commands today beyond 3 unit tests in `catalog.rs`.

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| CAT-01 | — | `fs_roots` on each OS | Home, Desktop/Documents, drives (Windows) or `/Volumes` (macOS) listed; none missing | E+PK | to automate (shape) / manual per OS | P1 |
| CAT-02 | Folder of 20 000 files | `fs_list` it | Returns within 2 s; sorted folders first; hidden files flagged; datasets recognised by extension | E (perf) | to automate | P1 |
| CAT-03 | — | `fs_list` on a missing path, a file, a permission-denied folder, a broken symlink, a path with unicode/spaces | Each gives a readable message ("You don't have permission to open …"); pane shows it inline, not the pop-up | E+2E | partial: unit catalog.rs; to automate | P0 |
| CAT-04 | — | `fs_stat` on 1 000 paths including missing ones | One state per path in order; missing are `exists=false` | E | to automate | P1 |
| CAT-05 | — | `fs_inspect` on a shapefile without `.prj`, a multi-layer GPKG, a raster, a corrupt file, a 5 GB file | Geometry type, count, CRS (or "undefined"), fields shown; corrupt file → "could not read" inline; large file does not read everything | E | to automate | P1 |
| CAT-06 | — | Catalog: double-click, drag to Contents / Model canvas / Map, "add all" in a folder | Layers added in listed order; dedupe warning on adding twice | M+2E | manual M-3; to automate drag via dev hooks | P2 |
| CAT-07 | — | Catalog while the folder changes underneath (file deleted after listing) | Add shows "File not found"; refresh updates the list | E+2E | to automate | P2 |
| CAT-08 | Windows | Drive letters, `C:` relative paths, network share offline | Listed; offline share errors within a timeout, UI not frozen | M | manual | P2 |

## 1.5 TIGER/Line and Census downloads (`tiger_*`)

Existing: TP 10.1, 10.2, 10.4, 10.6, 10.10, 10.11. Fixed bug **U-2** has the unit test `tiger::tests::bad_responses_are_described_not_just_not_json` and two network tests.

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| TIG-01 | Offline | Open the dialog, list states/vintages/counties | Dialog shows "Could not reach <host>. Check your internet connection" in the dialog; no pop-up crash; Retry works | E+2E | to automate (mock server, see HTTP-MOCK below) | P0 |
| TIG-02 | Mock server | Server returns HTML ("503 Service busy" page, 200 status) for a page | Retries up to 3 times; error names "a web page titled …"; no "isn't JSON" alone | E | partial: tiger tests (describe fn); to automate end-to-end with mock | P0 |
| TIG-03 | Mock | Response truncated mid-JSON (cut at 50 %) | Retry, then halves page size (1000→500→250); finally "response ended early" naming dataset and page | E | partial: U-2 unit test; to automate halving | P0 |
| TIG-04 | Mock | Server returns `{"error":{"code":400,...}}` JSON, 429 with Retry-After, 500, 404 for a vintage | Message includes HTTP status in plain words ("The Census server is overloaded; try again in a minute"); no raw JSON | E | to automate | P0 |
| TIG-05 | Mock | Server accepts the connection and never answers; slow-drip body | Times out (≤ 60 s per request, configurable) with "timed out waiting for"; task disappears; Cancel works within 2 s | E | to automate | P0 |
| TIG-06 | Mock | Features arrive with geometry null, rings that do not close, 0 features for a rural county | Null-geometry rows counted ("12 without geometry"); empty result: "no features in <county>" notice, no empty layer silently added | E | to automate | P1 |
| TIG-07 | — | State at antimeridian (Alaska: Aleutians cross 180°); Hawaii; territories (Guam, AS) | Layer imports; Aleutians West county keeps both sides; map CRS 3338/Albers draws; geometry valid | E (network) | to automate (ignored, network) | P0 |
| TIG-08 | — | Same dataset downloaded twice | Second layer gets "(2)" or a "replace?" prompt; citations show 2 uses of 1 source; no duplicated source | E | to automate | P1 |
| TIG-09 | — | Download 4 datasets, kill the network after the 2nd | First two layers kept and listed as "imported", others in a "failed" list with reasons; progress cleared | E | to automate (mock) | P0 |
| TIG-10 | — | Cancel during the 1st page and during the last page | Nothing imported for the cancelled dataset; completed datasets stay; "Cancelled" not shown as an error | E | partial: TP 10.9 (TNM only); to automate | P0 |
| TIG-11 | — | Two TIGER downloads at once; a TIGER and a TNM at once | Two progress bars; each cancel independent; layer ids unique (CON-02) | E+2E | to automate | P1 |
| TIG-12 | — | Choose `Blocks` for a state (needs county), counties list for a state with 254 counties (TX) | Block requires county (disabled until chosen); big lists scroll; > 100 000 blocks pages without timing out | E+M | partial: TP 10.1; manual | P1 |
| TIG-13 | — | Census API key (when the Census join exists) missing, wrong, or revoked | "The Census API rejected the key" with a link to Settings; key never in logs | E | pending feature (Census join) | P2 |
| TIG-14 | Dialog open | Close the dialog or switch project during a download | Download continues or is cancelled explicitly; layers added to the project that started it | 2E | to automate | P2 |
| TIG-15 | — | Vintage list: newest first; vintage for which dataset is missing (ACS has no blocks) | Unavailable datasets disabled with a reason | E | auto: TP 10.6 (network) | P2 |

**HTTP-MOCK (shared infrastructure to build once).** A tiny local HTTP server in `crates/engine/tests/support/mock_http.rs` that scripts responses (status, body, delay, truncate, drop connection), plus an env override of the TIGERweb/TNM/3DEP base URLs. It turns every "network failure" case in this plan (TIG-01…10, TNM-01…10, DEM-01…06, BMP-*) into a deterministic offline test. **No network case below is realistically automatable without it.**

## 1.6 The National Map and 3DEP (`tnm_*`, `download_elevation`)

Existing: TP 10.3, 10.5, 10.7–10.9. Fixed bug **U-3** has `clip_tests::clipping_repairs_invalid_polygons_instead_of_failing`.

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| TNM-01 | — | Area given as map view / layer extent / "clip to layer"; area is 1 m wide; area whole world; area crossing the antimeridian | Tiny: warns "no data expected"; world: refused ("area too large, limit N km²"); antimeridian: split into two requests or a clear message | E | partial: TP 10.3 (empty/NaN); to automate rest | P0 |
| TNM-02 | — | Clip layer is MultiPolygon with holes, a point layer, a layer without CRS, a CRS ≠ 4326 | Holes respected; point/no-CRS refused with a sentence; reprojected clip proven correct | E | to automate | P0 |
| TNM-03 | Mock | Esri JSON with invalid polygons (self-intersecting, the U-3 coordinates), duplicate rings, zero-area rings | Repaired (`ST_MakeValid`, same dimension); count of repaired features reported; import never aborts on one dataset | E | auto: `clip_tests::clipping_repairs_invalid_polygons…` + TP 10.5; to automate a mock end-to-end | P0 |
| TNM-04 | Mock | One of 5 datasets fails (500) | Others imported; dialog's "failed" list names the dataset and reason | E+2E | to automate | P0 |
| TNM-05 | Mock | `exceededTransferLimit: true` with offset paging; paging that never ends; same page repeated | All pages fetched; loop guarded by a max; message if the limit stops it | E | to automate | P0 |
| TNM-06 | Mock | HTML error page, truncated JSON, empty body, wrong content type (as TIG-02/03) | Same readable messages as TIGER; the failing dataset named | E | to automate | P0 |
| TNM-07 | Mock | Timeout / connection reset mid-download; Cancel mid-page | Readable timeout; cancel imports nothing from that dataset | E | partial: TP 10.9; to automate | P0 |
| TNM-08 | — | Dataset layers' geometry types (lines for streams, polygons for lakes, points) vs the schema declared; Z/M values | Declared type matches; Z dropped with a note | E | auto: TP 10.7 (network) | P1 |
| TNM-09 | — | Area with 0 features | "No <dataset> features in the area", no empty layer | E | to automate | P1 |
| TNM-10 | — | Re-download over existing layers | Duplicate names handled; citations not duplicated | E | to automate | P1 |
| DEM-01 | — | 3DEP area small (0.01°), default resolution, resolution finer than available, coarser than 1 arc-second | Smallest honest resolution reported; "resampled" notice when requested finer than source | E (network) | partial: TP 10.8 | P1 |
| DEM-02 | — | Area outside US coverage (Europe, ocean) | "3DEP covers the United States only" / no-data message, no empty raster layer | E | to automate (mock) | P0 |
| DEM-03 | Mock | Server returns an HTML/XML service exception with HTTP 200 instead of a TIFF | "The elevation service returned an error: <text>"; the XML is parsed, not shown raw | E | to automate | P0 |
| DEM-04 | Mock | Truncated TIFF, zero-byte body, huge (500 MB) response | Truncated: "download ended early"; size cap enforced with message | E | to automate | P1 |
| DEM-05 | — | Cancel mid-download; offline | As TIG-05/TIG-01; temp file deleted | E | to automate | P0 |
| DEM-06 | — | Result CRS is UTM of the area's centre; area straddling two UTM zones | Chooses one zone, says which; area at 84°N/80°S (outside UTM) uses polar stereographic or says it cannot | E | partial: TP 10.8; to automate | P1 |

## 1.7 Basemaps (map view and layouts)

Existing: TP 3.15, 9.12 (`p18_*`), bug U-1 (`layout.test.ts` "map frame basemap follows the map view").

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| BMP-01 | Offline | Switch to USGS Topo | Tiles fail quietly: one notice "Basemap tiles could not be loaded", no per-tile pop-ups; vectors still draw | 2E | to automate (block host in Playwright) | P0 |
| BMP-02 | — | Switch among 5 basemaps 50× quickly | No leaked layers (one active); citation row appears once (`record_basemap`) | 2E | partial: TP 11.5; to automate rapid switch | P1 |
| BMP-03 | — | Basemap in a map CRS other than Web Mercator (UTM, polar, geographic) | Reprojected (or disabled with a reason "basemap is Web Mercator only"); never drawn misaligned | 2E+M | to automate / manual | P0 |
| BMP-04 | — | NAD27 map CRS | Datum-offset note shown | M | manual M-6 | P2 |
| BMP-05 | — | Layout basemap: map frame with Topo, scale 1:24 000 and 1:10 000 000, polar CRS, antimeridian frame, offline, tile server 500 | Warped to the frame and cited; offline: export succeeds with a warning "basemap omitted (no network)" or fails clearly; never a blank silently | E | partial: TP 9.12; to automate failure paths with mock | P0 |
| BMP-06 | — | OpenStreetMap basemap then new layout / "Set from Map View" | Prints as none with the message (tile policy); no hotlinking | V | auto: layout.test.ts "map frame basemap follows the map view" | P1 |
| BMP-07 | — | Basemap attribution visible and included in the layout Sources element | Present exactly once | E+V | partial: TP 11.6 | P1 |

## 1.8 Imports of non-file sources

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| GEN-01 | — | `generate_test_layer` kinds (points, polygons), counts 0, 1, 10 M, negative, NaN, a bounding box inverted / zero-size / crossing ±180 | 0/negative/NaN: readable error; inverted bbox rejected; huge shows progress; layer CRS 4326 and cited as "generated test data" | E | partial: self-test uses it; to automate edge values | P1 |
| GEN-02 | — | `create_features` (layer from selection / copy) with an empty selection, all, 1 M | Empty: message "Select features first"; result inherits sources and CRS | E | partial: TP 5.16; to automate empty | P1 |
