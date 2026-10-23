# Project status

_Updated 2026-10-23. Read this first; the roadmap is `docs/ROADMAP.md`._

## State
- Branch `main`, pushed to https://github.com/AsherSeiling/OpenGeography (public). Untracked: `.claude/` (local launch config — don't commit), `PROJECT_SUMMARY.md` (superseded by this file and the roadmap).
- macOS arm64 built and tested locally. CI is green on macOS, Linux and Windows (build, tests, installers, clean self-test) — see `docs/CI_LOG.md`.
- CI logs need a GitHub login; without one, read failures from the run's annotations (`scripts/ci-run.sh` publishes key output lines) and job summaries.

## Build and test status (macOS arm64, 2026-10-23)
| Check | Result |
|---|---|
| `npx tsc` | pass |
| `npx vitest run` | 55/55 pass |
| `cargo test -p engine` | 27/27 integration + 7 unit pass (4 network tests ignored) |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass (rebuilt 2026-10-23 09:31 with node shortcuts) |
| Browser check (devserver + vite) | model Cancel from toolbar stops a 3M-feature run in ~0.1 s |

## Completed milestones
Office-style UI, map, layers, table, CRS · per-layer node trees · spreadsheet editing · definition queries · rasters + 3DEP · 48 native tools · self-contained macOS installer · geodesic measuring + resection/intersection · National Map downloads · progress bars + cancel for downloads · sources & citations · third-party notices.

## Known issues / technical debt
1. Cancel lands between DuckDB work units (~100k rows/thread). GDAL translate/warp/contours/polygonize/rasterize stop mid-step; gdaldem (hillshade/slope/aspect/ruggedness) ignores GDAL's progress callback, so those finish first.
2. Linux AppImage still bundles Ubuntu's GDAL with GPL-only libraries (license gate reports); local Homebrew macOS builds too (warns). CI macOS/Windows release builds are GPL-free.
3. Fixed 2026-10-23: deleted-layer references are flagged; parameter typing is one undo step; bundled GDAL/PROJ data always wins; rasters are copied into the project (portable); tool output fields come from the engine registry (`Tool::output_fields`, with a drift test).

## Recent decisions
- 2026-10-23 **Select By Location on the map** (Map → Selection): intersects / within distance (geodesic) / within / contains / touches / crosses, optionally only the comparison layer's selected features, invert, and new / add / remove / subset selection methods. Engine `select_by_location` shares `nodes::select_by_location::location_test` with the model node.
- 2026-10-23 **Model-run cancellation** (done): Cancel on the Model toolbar and status bar. A watcher thread calls DuckDB `interrupt()` repeatedly until the run returns (DuckDB clears the flag at each query start, so a single interrupt can miss); the executor checks before each step and marks remaining steps `cancelled`. Result writes (`set_result`, `upsert_layer`) run in one transaction, so a cancelled/failed write keeps the previous result (a test proves the layer corrupts without it). Run tasks are labelled "Running model — <layer>".

- 2026-10-23 **Node editor**: Blender keys only over the editor (`nodes/keymap.ts`, rebindable in the Shortcuts dialog, saved in browser storage); one-step, type-checked inserts; changed/stale markers; mute = bypass in `graphPayload`.
- 2026-10-23 **Provenance**: Output/Add to Map record the sources of every upstream Layer input (same transaction as the result).
- 2026-10-23 **Geodesic speed**: aeqd centers snap to 0.25° (~30× faster; < 0.1 m error on 50 km buffers in tests).
- 2026-10-23 **GDAL pinned to 3.12** on every platform (conda-forge `libgdal-core=3.12` in CI; Homebrew 3.12.3 locally — don't `brew upgrade gdal duckdb`): gdal crate 0.19 doesn't support GDAL 3.13. DuckDB pinned to 1.5.6. The spatial extension ships gzip-compressed (Linux packagers corrupted it).
- 2026-10-23 **Catalog pane** (file manager) with a "This project" view; TIGER downloads by year; bibliography auto-export (APA/MLA/Chicago, BibTeX, RIS).

## Next tasks (in order)
1. (In progress — `036e780`, CI pending) Linux: build with conda-forge GDAL 3.12 + LGPL SpatiaLite like macOS/Windows, then make the Linux license gate strict (watch for conda vs. system WebKitGTK library conflicts).
2. Layouts (M2: page, map frame, legend, scale bar, north arrow, title, export PDF/PNG) — design in Opus first, then implement.
3. Attribute history between nodes ("git for tables"; see roadmap).
4. Public data toolkit: generic ArcGIS REST catalog first (docs/PUBLIC_DATA_CATALOG.md).
5. Census data join (user enters the API key; OS keychain).

## Exact next action
Check the latest CI run (docs/CI_LOG.md has the history; failures show as annotations on the run page). If green, start task 1. A ready-to-paste prompt for a new Sonnet session is in `docs/NEXT_SESSION_PROMPT.md`.
