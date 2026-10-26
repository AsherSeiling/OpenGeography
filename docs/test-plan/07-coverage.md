# 7. Coverage summary and top gaps

Generated from the case tables in this folder by counting the **Auto** column (script: see the end of this file) and from the Coverage column of `../TEST_PLAN.md` §1–13 (the 26 manual cases M-1…M-26 in §14 are not counted as rows). Numbers are **cases**, not tests. "Auto" = an existing automated test is named; "Partial" = something covers part of the case (the named test or a neighbouring case) but not the full edge; "To automate" = nothing covers it. Snapshot: 2026-10-26, commit of this branch.

## 7.1 Feature × status

| Feature area | New cases | Existing TP cases | Auto (new) | Partial | To automate | Manual | Pending feature | Auto total (new+TP) | Open gaps (new gap + partial + TP gap) |
|---|---|---|---|---|---|---|---|---|---|
| Vector/table import | 20 | 24 | 0 | 6 | 12 | 2 | 0 | 18 | 21 |
| Raster import | 8 | 0 | 0 | 1 | 7 | 0 | 0 | 0 | 8 |
| Export | 12 | 0 | 0 | 3 | 9 | 0 | 0 | 0 | 12 |
| Catalog / file system | 8 | 0 | 0 | 1 | 5 | 2 | 0 | 0 | 6 |
| TIGER downloads | 15 | 11 | 1 | 4 | 9 | 0 | 1 | 10 | 13 |
| National Map downloads | 10 | 0 | 2 | 2 | 6 | 0 | 0 | 2 | 8 |
| 3DEP elevation | 6 | 0 | 0 | 2 | 4 | 0 | 0 | 0 | 6 |
| Basemaps | 7 | 0 | 1 | 3 | 2 | 1 | 0 | 1 | 5 |
| Test data / new layers | 2 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 2 |
| CRS | 13 | 18 | 0 | 9 | 4 | 0 | 0 | 17 | 13 |
| Map view | 14 | 18 | 2 | 2 | 8 | 2 | 0 | 16 | 10 |
| Symbology | 11 | 0 | 0 | 5 | 3 | 1 | 2 | 0 | 8 |
| Selection and queries | 9 | 19 | 1 | 4 | 4 | 0 | 0 | 19 | 8 |
| Identify | 3 | 0 | 0 | 1 | 2 | 0 | 0 | 0 | 3 |
| Measure | 5 | 0 | 0 | 2 | 3 | 0 | 0 | 0 | 5 |
| Land navigation | 2 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 2 |
| Attribute table | 17 | 21 | 0 | 9 | 6 | 2 | 0 | 13 | 20 |
| Node editor | 14 | 24 | 2 | 6 | 5 | 1 | 0 | 23 | 11 |
| Executor and cache | 13 | 0 | 4 | 3 | 6 | 0 | 0 | 4 | 9 |
| Attribute history | 4 | 13 | 2 | 1 | 1 | 0 | 0 | 14 | 2 |
| Progress and cancel | 7 | 13 | 0 | 5 | 1 | 1 | 0 | 12 | 6 |
| Concurrency | 9 | 0 | 0 | 1 | 7 | 0 | 1 | 0 | 8 |
| Tools (51) | 57 | 51 | 1 | 44 | 11 | 0 | 1 | 30 | 76 |
| Layouts | 22 | 17 | 1 | 14 | 4 | 1 | 2 | 15 | 20 |
| Projects | 18 | 11 | 3 | 6 | 5 | 2 | 2 | 10 | 13 |
| Sources and citations | 6 | 11 | 0 | 4 | 2 | 0 | 0 | 9 | 7 |
| Secrets (keychain) | 6 | 0 | 0 | 0 | 0 | 0 | 6 | 0 | 0 |
| IPC commands / devserver | 8 | 0 | 0 | 3 | 5 | 0 | 0 | 0 | 8 |
| Ribbon, panes, dialogs | 12 | 0 | 0 | 5 | 5 | 2 | 0 | 0 | 10 |
| Cross-platform | 6 | 0 | 0 | 2 | 3 | 1 | 0 | 0 | 5 |
| Packaging and self-test | 10 | 0 | 2 | 0 | 4 | 4 | 0 | 2 | 4 |
| Error reporting | 18 | 0 | 1 | 7 | 8 | 1 | 1 | 1 | 15 |
| **Total** | **372** | **251** | **23** | **159** | **151** | **23** | **16** | **216** | **344** |

## 7.2 New cases by test level and status

A case that names two levels (for example E+2E) is counted at both.

| Level | Cases | Auto | Partial | To automate | Manual | Pending feature |
|---|---|---|---|---|---|---|
| E (engine unit/integration, `cargo test -p engine`) | 254 | 13 | 123 | 114 | 0 | 4 |
| V (vitest) | 59 | 7 | 32 | 17 | 0 | 3 |
| 2E (Playwright e2e) | 62 | 1 | 26 | 31 | 3 | 1 |
| PK (packaged app / self-test) | 17 | 1 | 5 | 5 | 5 | 1 |
| M (manual only) | 24 | 0 | 2 | 3 | 15 | 4 |
| CI (workflow) | 2 | 1 | 0 | 0 | 1 | 0 |

## 7.3 New cases by priority

| Priority | Cases | Auto | Partial | To automate | Manual | Pending |
|---|---|---|---|---|---|---|
| P0 | 146 | 5 | 73 | 58 | 4 | 6 |
| P1 | 172 | 14 | 77 | 68 | 7 | 6 |
| P2 | 54 | 4 | 9 | 25 | 12 | 4 |

Reading the totals: **623 cases** (372 new + 251 existing TP rows), **216 fully automated (34.7 %)**; counting partial coverage of new cases, 375 (60.2 %) have at least some automation. Excluding the 16 cases whose feature does not exist yet, the fully automated share is 35.6 %. The existing TP rows are strong (193 of 251 automated, 77 %); the new rows are mostly *edge cases on top of them*, which is why most are "partial". (The TP count excludes the 26 manual step lists M-1…M-26 in TP §14.)

## 7.4 Top 10 gaps (ranked by risk), recommended next tests

Risk = chance a real user hits it × damage (data loss, wrong silent result, crash) × how many cases it unlocks.

| # | Gap | Cases | Why it ranks here | First test to write |
|---|---|---|---|---|
| 1 | **No mock HTTP server: every network-failure path is untested** (HTML pages, truncated JSON, timeouts, 429/5xx, partial success, cancel mid-download). Two of the four user-reported bugs (U-2, U-3) were of this class. | TIG-01…10, TNM-04…07, DEM-02…05, BMP-01/05, CON-02, ERR-08 | Public servers misbehave daily; one helper unlocks ~30 cases | `crates/engine/tests/support/mock_http.rs` + `tiger_html_page_then_json_retries`, `tnm_one_dataset_fails_others_import` |
| 2 | **Tools have happy-path tests but no generic matrix**: no-CRS is checked for 4 vector tools only, plus empty input, wrong geometry type, invalid geometry, antimeridian/pole, bad settings | TOOL-G02…G22 | One parametrised test over `tool_specs` covers 51 tools; the invalid-geometry class already caused U-3 | `tools.rs::every_tool_refuses_a_layer_without_crs`, `…_accepts_empty_input`, `…_survives_invalid_geometry` |
| 3 | **Raw-error leak guard, IPC parity, ribbon sweep**: nothing asserts that no user-visible error contains Rust/DuckDB/GEOS text; no command-parity check between `commands.rs` and the devserver; ribbon commands are never exercised together (the U-4 class) | ERR-01, CMD-01…05, UI-01, PRJ-02 | One regex over all error strings guards the "readable sentence" rule everywhere | `scripts/check_ipc_parity.py`; a vitest that calls every ribbon `run()` with no args and with a click event |
| 4 | **Project robustness**: newer-version project, truncated DB, interrupted save, missing source/raster files, save/open while tasks run, Save As into itself | PRJ-03…08, CON-05, CON-06 | Data loss in the artefact the user owns | `project.rs::interrupted_save_keeps_the_previous_project`, `opening_a_newer_project_is_refused` |
| 5 | **Antimeridian and poles outside the CRS maths**: import, tiles, rasters, tools, layouts, downloads (Aleutians, Fiji, Antarctica) | IMP-02, RAS-03, CRS-06/07, TOOL-G08, LAY-04, TIG-07, TNM-01, SEL-06, IDN-03 | Silent stretched geometries hit anyone working near 180°; M4 is on hold so pole-safe raster tools stay unverified | `display.rs::antimeridian_polygon_in_mercator_is_split_or_dropped_and_counted` with fixtures `fiji.geojson`, `aleutians.geojson` |
| 6 | **Concurrency is untested**: two imports, tree run while editing, save during a run, parallel tile + IPC load | CON-01, CON-03…07, TBL-05 | Shared DuckDB connection bugs appear as hangs or corrupt results, not errors | `engine.rs::two_imports_at_once_get_unique_ids`, `edit_during_model_run_is_safe` |
| 7 | **Export correctness**: no-CRS export, Shapefile name truncation and type loss, atomic replace, source overwrite through symlink or case-folding, mixed geometry | EXP-01…06, EXP-09 | Exported files are what leaves the program; B-T1 shows the class | `formats.rs::export_without_crs_warns`, `shapefile_long_names_are_mapped_and_reported` |
| 8 | **Layout failures and CRS interaction**: deleted layers, changed layer CRS, huge page/dpi memory guard, unicode fonts, atomic export on failure | LAY-01, 03, 05, 06, 08, 16, 22 | Layout is the main output of the product (priority 2) | `layout.rs::export_with_a_deleted_layer_says_which`, `png_over_the_pixel_limit_is_refused` |
| 9 | **Cancel coverage is thin**: only model steps, raster steps, diff and layout render are cancel-tested; import, export, Calculate Field, select by attributes and downloads are not | PRG-04, TBL-08, EXP-08, TIG-10 | A job you cannot stop on a 5 M-row layer means killing the app | `cancel.rs::cancel_calculate_field`, `cancel_export` |
| 10 | **Memory-bomb guards in tools**: Fishnet with a tiny cell, Contours with a tiny interval, Warp to a huge output, Polygonize of noise, Random Points 1e9 | TOOL-FSH-01, CTR-01, WRP-01, PLG-01, RND-01, RAS-01 | Out-of-memory kills the process without a message | `tools.rs::fishnet_over_the_cell_limit_is_refused_with_the_count` |

Also schedule right after these: the cross-platform path matrix on CI (XPL-01), negative `--self-test` cases (PKG-02), secrets tests when the keychain UI is built (SEC-*), and Playwright e2e in CI (it runs only through `run-test-plan.sh --e2e` today).

## 7.5 How the counts were produced

A throw-away script counted table rows whose ID matches `^[A-Z]+(-[A-Z]+)?-[A-Z]?\d+` in `docs/test-plan/0[1-6]*.md` and classified the Auto column by its first word (`auto`, `partial`, `manual`, `pending`, otherwise "to automate"); existing TP rows were classified by the first word of their Coverage cell. Committing it as `scripts/test_plan_coverage.py` is the first item of the automation batch; until then update the tables and these totals by hand.
