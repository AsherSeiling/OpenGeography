# Project status

_Updated 2026-10-28. Read this first. Order of work: `docs/MASTER_PLAN.md`. Why things are the way they are: `docs/DECISIONS.md` (read only for the area you touch). Bugs: `docs/BUGS.md`. Test cases: `docs/TEST_PLAN.md`. Session prompts: `docs/prompts/`. The user follows the "GIS Studio — Project Status" doc (https://claude.ai/artifact/5Pvx6sQMhUVcsJeijMbrAj), which the orchestrator updates at every change._

## Exact next action
- **Lead developer (Claude Code, standing order of work, MASTER_PLAN §4):** B7 (category drawing order); B6 built (local, pushed with B7) → C3.0 layout polish → C3.1 templates → C3.2 map series (ends Phase C) → Phase R (`docs/CRS_PLAN.md`).
- **Done:** the bug-fix session is merged. Q3 (2026-10-28): U-5 fixed, D-1 guard built. L1 (2026-10-28): labels on the map (pushed). L2 and L3 (2026-10-28): labels in layouts, and adjusting them in the layout (drag, edit text, hide, per-layout style) built. L2+L3 pushed together; CI green (run 40734953739); CI runs once for L1–L3 (user's rule).
- **On hold:** M4 (rasters in any CRS). Do not start it.
- **You:** A3 (check the Windows and Linux installers on real machines), then tag `v0.1.0` (`git tag v0.1.0 && git push origin v0.1.0`). Next tag: `v0.2.0` at the end of Phase C.

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
| `npx vitest run` | 335/335 (34 files) |
| `cargo test -p engine` | 179 pass, 23 ignored (network, perf, and the open bugs in `docs/BUGS.md`) |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass |
| Browser e2e (`scripts/e2e.sh`, headless Chromium) | 26 pass, 0 fixme, ~1.5 min; automates 9 manual cases fully and 7 in part (`docs/TEST_PLAN.md` §14) |

## Known gaps (layouts) — C3.0 fixes these
- **Layout view:** no rulers, no Arrange group, no "1:N" scale-bar label.
- **Export:** no dpi box (PNG is 300 dpi).
- **Preview:** with a basemap it re-fetches tiles on every edit (4–5 s).
- **Later (C4):** feature labels and GeoPDF.

## Known issues
- **e2e suite:** `e2e/*.spec.ts` (Playwright, devDependency). `scripts/run-test-plan.sh --e2e` runs it; run it alone with `scripts/e2e.sh <out-dir>`. Needs Google Chrome, `/Applications/Chromium.app`, `E2E_BROWSER_PATH`, or `npx playwright install chromium`. The devserver address and vite port come from `GIS_DEVSERVER_ADDR`, `GIS_VITE_PORT`, `VITE_GIS_DEVSERVER` (defaults unchanged).
- **Bugs:** U-5 fixed 2026-10-28 (the dock closed the open layout when Properties was clicked; tracer `?trace=layout`). D-1: guard built (`tnm::plan_elevation`: ellipsoid sizes, refuses areas wider than 6° / beyond 84° / needing cells coarser than 90 m, says when coarsened); the tiled download and the service-limit tests are D0. See `docs/BUGS.md`.
- **Cancel granularity:** cancel lands between DuckDB work units. Big geodesic Buffer and Dissolve now stop in ~0.1 s (`nodes/batching.rs`); other single-call work may still lag. gdaldem steps ignore GDAL's progress callback.
- **Intermittent local failure:** on 2026-10-28 `cargo test -p engine` reported the `layout` test target failed once in a full parallel run (output not captured); it passed on the next two runs. Capture the output if it recurs (it may be the crash below).
- **Bugs found 2026-10-28:** U-6 (MapView's display-ready cache survives New Project, so a re-used layer id can stay blank in a projected CRS) and E2E-3 (M-25 flaked once). See `docs/BUGS.md`.
- **macOS CI crash:** one intermittent SIGSEGV in layout tests (run 40055424934, conda GDAL, parallel tests). It hasn't happened again. M4's P17 stress test is the gate.

## Next tasks (in order)
1. B7, then push B6+B7 (CI once). (L1–L3 done; CI green for `f685b41` on all platforms, run 40734953739.)
2. B6 split a layer by attribute, B7 category drawing order.
3. C3.0 layout polish → C3.1 templates → C3.2 map series (end of Phase C).
4. Phase R, coordinate-system hardening (`docs/CRS_PLAN.md`; 112 cases in `docs/TEST_PLAN.md` §2A). Recommended: tag `v0.2.0` after it.
5. Phase D: tiled 3DEP download (D0), generic ArcGIS REST catalog (`docs/PUBLIC_DATA_CATALOG.md`), then the Census data join.
M4 is on hold.
