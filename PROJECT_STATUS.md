# Project status

_Updated 2026-10-23. Read this first; the roadmap is `docs/ROADMAP.md`._

## State
- Branch `main`. Working tree clean apart from untracked `.claude/` (local launch config — don't commit) and `PROJECT_SUMMARY.md` (long-form summary written for a prompt generator; superseded by this file and the roadmap).
- macOS arm64 is the only platform built and tested. Windows/Linux CI jobs exist but have never run (no git remote).

## Build and test status (macOS arm64, 2026-10-23)
| Check | Result |
|---|---|
| `npx tsc` | pass |
| `npx vitest run` | 34/34 pass |
| `cargo test -p engine` | 20/20 pass (4 network tests ignored) |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass (2026-10-22 build; not rebuilt since — no native changes) |
| Browser check (devserver + vite) | model Cancel from toolbar stops a 3M-feature run in ~0.1 s |

## Completed milestones
Office-style UI, map, layers, table, CRS · per-layer node trees · spreadsheet editing · definition queries · rasters + 3DEP · 48 native tools · self-contained macOS installer · geodesic measuring + resection/intersection · National Map downloads · progress bars + cancel for downloads · sources & citations · third-party notices.

## Known issues / technical debt
1. Geodesic per-feature tools (buffer, near, join by nearest) build a PROJ transform per row (~50 µs–1.5 ms/feature): slow on big layers, and Cancel only lands between DuckDB work units (~100k rows/thread), so it can take seconds there. GDAL raster steps can't be interrupted mid-step.
2. Splice (drop onto connection) is two undo steps and doesn't check the new node's output against the downstream port.
3. No "changed since last run" marker; a failed run leaves the previous result shown without saying it's stale.
4. Trees referencing a deleted layer aren't flagged until run.
5. Model results inherit citations only from the owning layer.
6. GPL-only libraries (poppler, x265, librttopo) in the macOS package.
7. Projects with rasters aren't portable (rasters in a per-user store).
8. `store/model.ts::outputFields` duplicates per-tool knowledge (can drift from the registry).

## Recent decisions
- 2026-10-23 **Model-run cancellation** (done): Cancel on the Model toolbar and status bar. A watcher thread calls DuckDB `interrupt()` repeatedly until the run returns (DuckDB clears the flag at each query start, so a single interrupt can miss); the executor checks before each step and marks remaining steps `cancelled`. Result writes (`set_result`, `upsert_layer`) run in one transaction, so a cancelled/failed write keeps the previous result (a test proves the layer corrupts without it). Run tasks are labelled "Running model — <layer>".

## Next tasks (in order)
1. Push to a GitHub remote and get the Windows/Linux CI jobs green (needs the user's repository).
2. GPL-free GDAL for the installer.
3. Splice as one undo step + downstream type check; stale-result marker.
4. Provenance from all model inputs.
5. Faster geodesic per-feature tools (group features by rounded projection center), which also makes Cancel prompt for them.

## Exact next action
Ask the user for a GitHub repository (or permission to create one) to push `main` to, so `.github/workflows/build.yml` runs on Windows and Linux; then fix whatever those jobs report.
