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
| `npx vitest run` | 44/44 pass |
| `cargo test -p engine` | 22/22 pass (4 network tests ignored) |
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
- 2026-10-23 **Model-run cancellation** (done): Cancel on the Model toolbar and status bar. A watcher thread calls DuckDB `interrupt()` repeatedly until the run returns (DuckDB clears the flag at each query start, so a single interrupt can miss); the executor checks before each step and marks remaining steps `cancelled`. Result writes (`set_result`, `upsert_layer`) run in one transaction, so a cancelled/failed write keeps the previous result (a test proves the layer corrupts without it). Run tasks are labelled "Running model — <layer>".

- 2026-10-23 **Node editor**: Blender keys only over the editor (`nodes/keymap.ts`, rebindable in the Shortcuts dialog, saved in browser storage); one-step, type-checked inserts; changed/stale markers; mute = bypass in `graphPayload`.
- 2026-10-23 **Provenance**: Output/Add to Map record the sources of every upstream Layer input (same transaction as the result).
- 2026-10-23 **Geodesic speed**: aeqd centers snap to 0.25° (~30× faster; < 0.1 m error on 50 km buffers in tests).
- 2026-10-23 **GDAL pinned to 3.12** on every platform (conda-forge `libgdal-core=3.12` in CI; Homebrew 3.12.3 locally — don't `brew upgrade gdal duckdb`): gdal crate 0.19 doesn't support GDAL 3.13. DuckDB pinned to 1.5.6. The spatial extension ships gzip-compressed (Linux packagers corrupted it).
- 2026-10-23 **Catalog pane** (file manager) with a "This project" view; TIGER downloads by year; bibliography auto-export (APA/MLA/Chicago, BibTeX, RIS).

## Next tasks (in order)
1. Attribute history between nodes ("git for tables"; see roadmap).
2. Public data toolkit: generic ArcGIS REST catalog first (docs/PUBLIC_DATA_CATALOG.md).
3. GPL-free GDAL for the installer.
4. Census data join.

## Exact next action
Attribute history between nodes: persist each node's output table as a versioned snapshot and let the Attribute Table switch between the Output (default) and earlier nodes, with a diff. Check the latest CI run first and log it in docs/CI_LOG.md.
