# Full test plan (every feature, edge cases, error reporting)

_Written 2026-10-26 from the code (`src-tauri/src/commands.rs`, `crates/devserver`, `crates/engine/src/nodes/`, `ui/src/`), not only from the docs. Plan only: no test code, scripts or app code were written or changed._

This plan **extends** [`TEST_PLAN.md`](TEST_PLAN.md). The 251 existing rows (IDs `1.1`…`13.13`, manual cases `M-1`…`M-26`) keep their IDs and are referenced here as "TP n.m", never copied. This plan adds the cases that go beyond the happy path: boundary values, bad and empty input, missing CRS, CRS changes mid-workflow, antimeridian and poles, huge and tiny data, invalid geometry, network failure, cancel, undo/redo, save/reopen, layouts, concurrency, and path cases for every OS.

| File | Contents |
|---|---|
| [`test-plan/01-data-in-out.md`](test-plan/01-data-in-out.md) | Imports (vector, table, raster), export, Catalog/file system, TIGER, National Map, 3DEP, basemaps, test data |
| [`test-plan/02-crs-map-selection.md`](test-plan/02-crs-map-selection.md) | CRS, map view and tiles, symbology, selection, definition query, identify, measure, land navigation |
| [`test-plan/03-tables-model-history.md`](test-plan/03-tables-model-history.md) | Attribute table and editing, node editor, executor and cache, attribute history, progress and cancel, concurrency |
| [`test-plan/04-tools.md`](test-plan/04-tools.md) | Generic matrix for all 51 tools + tool-specific edge cases |
| [`test-plan/05-layouts-projects-sources-app.md`](test-plan/05-layouts-projects-sources-app.md) | Layouts, projects, sources, secrets, IPC parity, ribbon/panes/dialogs, cross-platform, packaging and `--self-test` |
| [`test-plan/06-error-reporting.md`](test-plan/06-error-reporting.md) | **Error-reporting procedure**: bug template, where it goes, mapping automated failures to case IDs, triage rules, readable-error checklist (ERR-*) |
| [`test-plan/07-coverage.md`](test-plan/07-coverage.md) | Coverage table (feature × status and level), top 10 gaps with the first test to write |

**Headline numbers** (details in file 07): 33 feature areas in the coverage table (the 51 tools are one area; 83 features if each tool is counted) · 623 cases (251 existing + 372 new) · 216 fully automated (34.7 %); 375 (60.2 %) have at least partial automation · 16 cases are for features not built yet.

## Conventions

**Case ID**: `<AREA>-<nn>` (`IMP-02`, `TIG-05`, `TOOL-G08`, `TOOL-BUF-01`); the area prefix is the file's section. Existing IDs: `TP 7` = TEST_PLAN §7 and so on. Name the ID in the test (`fn imp_02_…`, `it("CRS-06 …")`) so failures map back mechanically (see 06 §6.3).

**Columns** — *Pre* (preconditions), *Steps*, *Expected (user-facing)* (what the user sees, including the kind of error: inline message in a dialog, pane error box, error pop-up, status-bar notice), *Lvl*, *Auto*, *Pri*.

**Levels**: `E` engine unit/integration (`cargo test -p engine`, also `(perf)` ignored tests) · `V` vitest (`ui/src/**/*.test.ts`) · `2E` Playwright e2e against the devserver (`e2e/*.spec.ts`, `scripts/e2e.sh`) · `M` manual · `PK` packaged app (installer, `--self-test`) · `CI` workflow. `A+B` = both levels needed.

**Auto column** (first word is what the coverage script counts):
- `auto: <ref>` — an automated test covers the whole case (file::test or TP id).
- `partial: <ref>; to automate <what is missing>` — part of the case is covered.
- `to automate: <proposed test>` — nothing covers it yet.
- `manual …` — needs a person (steps in TP §14 or noted).
- `pending feature (…)` — the feature is not built (M4 on hold, labels L1–L3, B6, B7, C3/C4, keychain UI, recent projects, Census join). Cases are written now so they exist when the feature lands.

**Priority**: **P0** = data loss, crash, silent wrong result, or a main-path failure; must be automated or run manually before every release. **P1** = important edge or common failure. **P2** = rough edge, rare path.

**Network cases** run against the shared HTTP mock described in 01 §1.5 (to build once); real-network tests stay `#[ignore = "network: …"]`.

**Pending features, future cases only** (not counted above; add the generic matrix when each lands): *M4* — rasters in any CRS (tile pixel accuracy, antimeridian split rasters, pole-safe Slope/Aspect/Hillshade, warp at ±90°) and the GDAL thread-safety stress gate (8 threads × raster tools, no SIGSEGV; CON-08) · *L1–L3* labels (SYM-08, LAY-20) · *B6* split by attribute (one layer or file per value; NULL and 1 000-value fields; names that clash or contain `/`) · *B7* category order (SYM-09) · *C3/C4* map series, templates, Topo Map, GeoPDF (LAY-21) · keychain (SEC-*) · recent projects (PRJ-09) · Census join (TIG-13).

## Feature inventory (from the code)

### Tauri commands (`src-tauri/src/commands.rs`: 73; devserver: 71 routes — parity is case CMD-01)

| Area | Commands | Cases |
|---|---|---|
| Layers | `import_files`, `list_layers`, `remove_layer`, `copy_layer`, `rename_layer`, `layer_extent`, `create_features`, `generate_test_layer` | IMP, RAS, GEN, UI-04, MAP |
| CRS | `define_projection`, `suggest_crs`, `validate_crs`, `transform_point`, `map_scale_factor`, `crs_display_info` | CRS |
| Map | `build_display`, `display_status` (and the `gis://` tile protocol, devserver `/tiles`) | MAP, SYM |
| Table and selection | `get_rows`, `select_where`, `select_rect`, `identify`, `get_feature`, `unique_values`, `quantile_breaks`, `set_definition_query`, `select_by_location`, `measure_feature`, `raster_value` | TBL, SEL, IDN, MSR |
| Editing | `add_field`, `delete_field`, `rename_field`, `update_cell`, `calculate_field`, `calculate_geometry`, `delete_rows` | TBL, TOOL-CFD, TOOL-CGM |
| Graph | `tool_specs`, `run_graph`, `preview_output`, `clear_model_cache`, `prune_node_cache`, `set_node_cache_budget` | MDL, EXE, TOOL |
| History | `node_table_info`, `node_rows`, `node_diff`, `node_diff_rows`, `drop_node_diff` | HIS |
| Projects | `save_project`, `open_project`, `new_project` | PRJ |
| Downloads | `tiger_datasets`, `tiger_vintages`, `tiger_availability`, `tiger_counties`, `tiger_import`, `tnm_datasets`, `tnm_import`, `download_elevation`, `record_basemap` | TIG, TNM, DEMF, BMP |
| Catalog | `fs_roots`, `fs_list`, `fs_stat`, `fs_inspect` | CAT |
| Sources | `sources`, `update_source`, `remove_source`, `save_text` | SRC, EXP-12 |
| Layouts | `list_layouts`, `save_layout`, `delete_layout`, `render_layout_preview`, `export_layout` | LAY |
| Progress | `progress`, `cancel_task` | PRG |
| Diagnostics | `frontend_log`, `--self-test` (`src-tauri/src/selftest.rs`) | ERR, PKG |

### Tools (51 `ToolSpec`s in `crates/engine/src/nodes/`)

Listed in [`test-plan/04-tools.md`](test-plan/04-tools.md); known-result cases in TP §7; generic matrix `TOOL-G01…G23`; 33 tool-specific rows.

### UI (`ui/src/`)

- **Ribbon** (`ribbon/commands.tsx`, 65 commands; tabs include Map, Analysis, Data, View and the contextual Layout and Feature Layer tabs): cases UI-01, PRJ-02, and the per-feature cases.
- **Panes** (`panes/`): Contents, Catalog, Map, Attribute Table, Model, Properties, Geoprocessing (Tools), Sources — UI-03…UI-06.
- **Dialogs** (`dialogs/`): Add/Calculate Field, Calculate Geometry, Select by Attributes, Define Projection, Rename, Export, Layer Properties/Symbology, Definition Query, TIGER, National Map, Elevation, Benchmark — UI-07.
- **Map** (`map/`: OpenLayers controller, styles, CRS), **Navigation** (`nav/`: measure, land nav, geodesy), **Layout** (`layout/`), **Query** (`query/`), **Node editor** (`nodes/`), **Progress** (`progress/`), **Errors** (`errors/`: pop-up + pane boundaries), **Status bar**, **Sources** (`sources/`: citation formats, prefs), **Stores** (`store/`).

### Cross-cutting

Imports and downloads · CRS and the projected view · symbology · layouts (view, PDF/PNG/SVG export, basemaps) · projects (save, open; recent list not built) · model editor and executor · progress and cancel · citations · keychain (not built) · error pop-up · packaging and `--self-test` · cross-platform paths · concurrency.

## Running the plan

| What | Command | Notes |
|---|---|---|
| Static + UI + engine (CI) | `scripts/run-test-plan.sh --quick` | |
| + ignored engine tests (bug, network, perf) | `scripts/run-test-plan.sh` | `--no-network` offline |
| + browser e2e | `scripts/run-test-plan.sh --e2e` | not in CI yet |
| + packaged app and self-test | add `--package` | macOS only today; Windows/Linux by hand (M-26) |
| Manual cases | summary checklist and TP §14 | add the `manual` rows of files 01–05 to that checklist |

Output goes to `test-results/<time>/summary.md`. How a failing line becomes a case ID, a `docs/BUGS.md` entry and a triage decision: [`test-plan/06-error-reporting.md`](test-plan/06-error-reporting.md).

## Suggested order of automation

1. HTTP mock + network-failure tests (gap 1).
2. Parametrised tool matrix and error-text blacklist (gaps 2–3).
3. Project robustness and concurrency (gaps 4, 6).
4. Antimeridian/pole fixtures across import, tiles, layouts (gap 5).
5. Export, layout failure paths, cancel coverage, memory guards (gaps 7–10).
6. `scripts/map_failures.py` and `scripts/test_plan_coverage.py` so the summary reports failed case IDs and the coverage table updates itself.

Full ranking with the first test to write for each: [`test-plan/07-coverage.md`](test-plan/07-coverage.md#74-top-10-gaps-ranked-by-risk-recommended-next-tests).
