# User requests from the Mac app test, 2026-11-05

The user's list after testing the rebuilt app (`47fe444`). "Today" says what the code does now, from reading it on 2026-11-05. Nothing here is built yet.

## Decisions to confirm with the user before building

1. **Order.** Recommended: this list comes before server connections and planetary, so planetary stays the last feature before the first release. It replaces the "no other new features first" rule only if the user confirms.
2. **What runs Python.** Today's "Python" mode is only a translator to SQL (`ui/src/query/pyToSql.ts`); it can't run a function, and Calculate Field takes SQL. A function whose return value fills a field needs a real interpreter. Recommended: ship CPython with the app (python-build-standalone, PSF licence), running in its own process, exchanging rows through Arrow. It adds about 30–60 MB, and it pulls part of Phase G (Python SDK, `docs/MASTER_PLAN.md`) ahead of the release. Rejected: RustPython (MIT, but no NumPy, and Python users would expect NumPy); keeping the translator (can't do what was asked).
3. **Scripts inside projects run code.** A shared project holding Python could do anything on the machine that opens it. Recommended: ask before running scripts from a project this computer didn't create, and don't sandbox beyond that. Say so plainly in the prompt.

## Work, in the recommended order

### Batch A: bug and small fixes

| ID | Request | Today | Work |
|---|---|---|---|
| UR-1 **done** (commit: see `git log --grep UR-1`) | Export Table fails with `ST_X only supports POINT geometries` (see `docs/BUGS.md`) | `detect_geom_type` (`crates/engine/src/layers.rs:423`) calls MultiPoint a "Point" layer, because it goes by dimension; `export_table.rs:139` then calls `ST_X`, which refuses MultiPoint | Write one row per point, and report the count. Check the other `ST_X` calls that assume a Point layer (`tiles.rs:104`, `usng.rs:331`). |
| UR-2 | Rename the node editor to "Processing nodes" | "Node Editor" in `ui/src/App.tsx:50`, `panes.ts`, `ShortcutsDialog.tsx` | Rename user-facing text. Note: the user's spelling was "Proccesing"; this assumes a typo. Check e2e selectors. |
| UR-3 | Sources pane closed by default | Default layout adds it, `ui/src/panes/panes.ts:129` | Leave it out of the default layout, keep it in the view menu. |
| UR-4 | Maps in a layout don't start with an outline frame | Frame is a per-element setting (`frameWidth`, 0 = none) | Find what a new map frame gets, and default it to none. Old layouts keep their frames. |
| UR-5 | Layout of any size | Allowed 20–5000 mm, `ui/src/dialogs/PageSetupDialog.tsx:12` | Widen the limits as far as export really works, and say what limits remain. Test a very large page's export. |
| UR-6 | Guides in layouts | **Already exists**: drag from a ruler, or Page Setup. The user didn't find it | Add a visible ribbon control, so it can be found. |
| UR-7 | Zoom toward the cursor for everything in a layout | Ctrl/⌘+wheel zooms the page, plain wheel scrolls (`LayoutView.tsx:113`) | Anchor zoom at the cursor. Decide with the user whether plain wheel should zoom. |

### Batch B: map and data working

| ID | Request | Today | Work |
|---|---|---|---|
| UR-8 | Clip raster by a layer's shape | Only "Clip Raster by Extent" (`nodes/raster_tools.rs:133`, `raster::clip_to_extent`) | New tool or mode: GDAL warp cutline. Handle CRS mismatch explicitly, keep no-data outside. |
| UR-9 | Keep working during public data downloads | Needs checking: `busy()` in `actions.ts` isn't itself modal; find what blocks | Downloads run in the background with progress and cancel. Guard against saving, or closing the project, mid-download. |
| UR-10 | Layers from the data importer in the project catalog | Catalog lists files, maps and layouts (`ui/src/panes/CatalogPane.tsx`) | Add a section listing project layers, including imported ones. |
| UR-11 | CRS list sorted: projected or geographic, then location, then for example state, then datum | Flat search list (`searchCatalog`, `ui/src/crs/crs.ts:36`); `crs-list.json` has projected flag and area text only | Tree view. Location and state come from EPSG area-of-use text, so needs parsing with a test on all entries, and an "Other" group for what doesn't parse. Datum is in the name, or from PROJ. Search stays. Don't change any CRS definition. |
| UR-12 | Custom unique-value symbology, rather than auto-generated | Unique values generate every class (`PropertiesPane.tsx`) | Start empty if the user wants; add a class by hand or picked from values; group several values in one class; a default "all other values" class. Saved with the project. |
| UR-13 | Track layers with no citation; let the user build or paste one | Citations are recorded automatically (`crates/engine/src/sources.rs`), with `update` and `remove`, but no list of missing ones | Flag layers with none in the Sources pane and Map Layers. Add a form to build one and a box to paste one. A derived layer still inherits its inputs' sources. |
| UR-14 | Tables with no spatial file: generate, import, export (Excel, CSV and other major formats) | "Table" layers exist for imports with no geometry; CSV import and CSV/Parquet export exist; Excel doesn't | New empty table with typed fields, edit rows, import and export XLSX, CSV, TSV, ODS, Parquet, JSON, DBF. Check whether the shipped GDAL has XLSX and ODS; if not, use the `rust_xlsxwriter` and `calamine` crates (both permissive). Update `docs/OPEN_SOURCE.md`. |
| UR-15 | More Export Features options | Formats GeoPackage, Shapefile, GeoJSON (plus GeoTIFF for rasters); all or selected; USNG/MGRS column | Add KML, FlatGeobuf, GeoParquet, CSV with WKT, GPX, DXF if the shipped GDAL has them; choose the CRS; choose fields; clip to view or a layer. Shapefile's field-name and size limits get reported. Confirm the list with the user. |

### Batch C: larger

| ID | Request | Today | Work |
|---|---|---|---|
| UR-16 | Program settings menu, easy to reach | None | Settings dialog on the ribbon, stored per user outside any project (never secrets in it). Holds UR-17 and default scripting language, default CRS, units. |
| UR-17 | Customizable startup panes | Fixed default layout, `panes.ts` | Save the current arrangement as the startup layout; reset to default. Needs UR-16. |
| UR-18 | Activate a map in the layout editor and move it | No activation in `LayoutView.tsx` | Double-click a map frame to activate it: pan and zoom change that frame's extent and scale. Esc or click outside ends it. One undo step. Needs care: the preview is engine-rendered, so pan must stay responsive. |
| UR-19 | All basemaps work in all CRSs | Refused on purpose (`basemapRefusal`, `ui/src/map/basemaps.ts:123`): vector basemaps in any non-Mercator CRS; raster ones outside the CRS's area of use (bug MM-3) | Raster: reproject over the whole valid range, leaving blank only what the CRS or the tiles can't cover. Vector: draw through an off-screen renderer into raster, then reproject. Must stay correct: nothing misplaced, and a clear notice where blank. **Print limits stay**: a provider's terms (`printRefusal`) aren't a technical limit. Reads `docs/MAP_PROJECTION_PLAN.md` M4 first. Read closely. |

### Batch D: Python scripting (after decision 2)

| ID | Request | Work |
|---|---|---|
| UR-20 | Python is the default language everywhere | Field calculator, definition queries, select by attributes, label expressions, raster calculator. SQL and today's forms stay as options. Saved old queries keep working as they are. List each place before starting. |
| UR-21 | The query form is a Python function; what it returns goes into the field or result | `def calculate(row): return row["POP"] / row["AREA"]`. For filters, a True or False return picks rows. Report errors with row number and message, and count rows that failed; never silently fill null. Must be cancellable, with progress. Raster calculator's form works on NumPy arrays. |
| UR-22 | Graphical scripting option | Blockly (Apache-2.0) building the same function, showing the Python it makes. A block script opens as text; text only converts back if it came from blocks. Update `docs/OPEN_SOURCE.md`. |

Batch D doesn't cover the Phase G SDK as a whole (custom Python nodes, headless CLI), which stays after the release unless the user says otherwise.

## Rules for all of this

New features add cases to `docs/TEST_PLAN.md`. Any new Tauri command also goes in `crates/devserver`. Project format changes keep old projects opening. Mark a request done in this file with its commit.
