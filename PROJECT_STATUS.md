# Project status

_Updated 2026-10-25. Read this first. Order of work: `docs/MASTER_PLAN.md`. Why things are the way they are: `docs/DECISIONS.md` (read only for the area you touch). Bugs: `docs/BUGS.md`. Test cases: `docs/TEST_PLAN.md`. Session prompts: `docs/prompts/`. The user follows the "GIS Studio — Project Status" doc (https://claude.ai/artifact/5Pvx6sQMhUVcsJeijMbrAj), which the orchestrator updates at every change._

## Exact next action
- **Orchestrator (Opus):** delegate the **bug-fix session** (brief 1 in `docs/prompts/next-session.md`: B-T1, B-T2, B-T4–B-T8 and B5), then **M4** (brief 2).
- **You:** A3 (check the Windows and Linux installers on real machines), then tag `v0.1.0` (`git tag v0.1.0 && git push origin v0.1.0`). Next tag: `v0.2.0` at the end of Phase C (decided 2026-10-25).

## State
- `main` is on GitHub at https://github.com/AsherSeiling/OpenGeography (public). Never commit `.claude/` (local launch config and worktrees).
- Phases A (except A3 and the tag) and B are done.
- Phase C:
  - C1 and C2.1–C2.4 are done: layouts print a full map.
  - The projected map view M1–M3 is done: OpenLayers draws in the map's CRS, and the tools are exact in any CRS.
  - Error pop-up and per-pane error boxes (Q1), and the test plan with its tests (Q2), merged 2026-10-25.
  - Left: M4 (rasters in any CRS, pole-safe tools, GDAL thread-safety gate), C3.0 layout polish, then C3.1–C3.2.
- CI history: `docs/CI_LOG.md`. CI logs need a GitHub login: read failures from the run annotations.
- The packaged Mac app and DMG are copied into the project folder (`GIS Studio.app`, `GIS-Studio-<version>-macos-arm64.dmg`).

## Build and test status (macOS arm64, 2026-10-25)
| Check | Result |
|---|---|
| `npx tsc` | pass |
| `npx vitest run` | 310/310 (30 files) |
| `cargo test -p engine` | 176 pass, 23 ignored (network, perf, and the open bugs in `docs/BUGS.md`) |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass |

## Known gaps (layouts) — C3.0 fixes these
- **Layout view:** no rulers, no Arrange group, no "1:N" scale-bar label.
- **Export:** no dpi box (PNG is 300 dpi).
- **Preview:** with a basemap it re-fetches tiles on every edit (4–5 s).
- **Later (C4):** feature labels and GeoPDF.

## Known issues
- **Open bugs:** see `docs/BUGS.md`. Each has an ignored test. The two High ones are B-T5 (raster warp to an invalid CRS crashes) and B-T7 (warping a raster with no CRS invents one).
- **Cancel granularity:** cancel lands between DuckDB work units (~100k rows/thread). gdaldem steps ignore GDAL's progress callback.
- **Undo:** undoing a node-tree edit can drop a node's `lastOutput`. B5 fixes it.
- **macOS CI crash:** one intermittent SIGSEGV in layout tests (run 40055424934, conda GDAL, parallel tests). It hasn't happened again. M4's P17 stress test is the gate.

## Next tasks (in order)
1. Bug-fix session (B-T1, B-T2, B-T4–B-T8, B5).
2. M4 (`docs/MAP_PROJECTION_PLAN.md`, P14–P18).
3. C3.0 layout polish → C3.1 templates → C3.2 map series (end of Phase C, `v0.2.0`).
4. Phase D: generic ArcGIS REST catalog (`docs/PUBLIC_DATA_CATALOG.md`), then the Census data join.
