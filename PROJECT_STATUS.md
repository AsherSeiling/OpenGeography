# Project status

_Updated 2026-10-23. Read this first. Order of work: `docs/MASTER_PLAN.md`. Why things are the way they are: `docs/DECISIONS.md` (read only for the area you touch). Session prompts: `docs/prompts/`._

## Exact next action
- **Orchestrator (Opus):** review and merge branch `claude/m2-openlayers-view-cc786c` (M2, OpenLayers replaces MapLibre; licences listed in the runner's review block, coordinate handling in `map/mapController.ts`, `map/readout.ts`, `panes/MapView.tsx`, `display.rs::basemap_offset`). Next Sonnet task: **M3** (brief in `docs/prompts/next-session.md`).
- **Orchestrator (Opus, `docs/prompts/orchestrator.md`):** review and merge branch `claude/c2-3-4-layout-elements-0bebf9` (C2.3 + C2.4, two commits; geodesy hunks: `layout/surround.rs` scale bar and north arrow, `layout/grid.rs` graticule, `raster.rs::ground_pixel_m`), then rebuild the Mac app and run `--self-test` (P19). Next Sonnet task: **M1** of the projected map view (brief in `docs/prompts/next-session.md`), then M2–M4.
- **You:** A3 (check the Windows and Linux installers on real machines), then tag `v0.1.0` (`git tag v0.1.0 && git push origin v0.1.0`), then decide on `v0.2` (Phase B is done).

## State
- `main` is pushed to https://github.com/AsherSeiling/OpenGeography (public). Never commit `.claude/` (local launch config and worktrees).
- Phases A (except A3 and the tag) and B are done. Phase C: M1 (CRS grids, display tables, tiles in any CRS) merged; M2 (OpenLayers map view in any CRS) done on its branch awaiting review; C1 designed, C2.1 and C2.2 merged, C2.3 + C2.4 (surround elements, graticule/grid, rasters, USGS basemaps) done on their branch awaiting review.
- CI is green on macOS, Linux and Windows through Phase B (`docs/CI_LOG.md`). CI logs need a GitHub login: read failures from the run annotations (`scripts/ci-run.sh` publishes key lines).
- The packaged Mac app and DMG are copied into the project folder (`GIS Studio.app`, `GIS-Studio-<version>-macos-arm64.dmg`) and rebuilt once per phase.

## Build and test status (macOS arm64, 2026-10-23)
| Check | Result |
|---|---|
| `npx tsc` | pass |
| `npx vitest run` | 80/80 |
| `cargo test -p engine` | 19 unit + 35 + 9 (history) + 25 (layout) integration; 4 network tests, H14, P6 and the USGS basemap network test ignored |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass |

## Known gaps (layouts)
- The Layout view has no rulers yet and no Arrange group (bring forward/back, align). Scale bar has no "1:N" label option. Preview with a basemap re-fetches tiles on each edit (4–5 s). Feature labels and GeoPDF are later (C4).
- Export dialog has no dpi box yet: PNG is 300 dpi. In the browser build the file path comes from a prompt.

## Known issues
1. Cancel lands between DuckDB work units (~100k rows/thread). GDAL translate/warp/contours/polygonize/rasterize stop mid-step; gdaldem (hillshade/slope/aspect/ruggedness) ignores GDAL's progress callback and finishes first.
2. Undoing a node-tree edit can drop a node's `lastOutput`. The table pane then shows "not run" until the next run, which is a cache hit.
3. Fixed 2026-10-23: the one `package-macos.sh` failure (dylibbundler, libabsl_time) came from two sessions packaging at once in the same folder; each run's `rm -rf Frameworks` deleted the other's libraries. The script now takes a lock (`target/.package-macos.lock`), so a second run exits at once.
4. Intermittent: the layout test binary crashed once with SIGSEGV on macOS CI (run 40055424934, conda-forge GDAL). It crashed after the C2.4 raster/basemap tests passed, while the other layout tests ran in parallel. The same code then passed on all three platforms (run 40056362301), and 15 local runs (Homebrew GDAL) passed. No GDAL or PROJ object is shared between threads in the layout code. Suspect: conda GDAL/PROJ under parallel tests. If it recurs, rerun on macOS CI with `RUST_BACKTRACE=1 cargo test -p engine --release --test layout -- --test-threads=1` to name the test. Ship nothing that renders rasters from several threads until it's understood.

## Next tasks (in order)
1. Projected map view M3–M4 (`docs/MAP_PROJECTION_PLAN.md`; M1–M2 done: the map draws in the map's CRS, rasters still in Web Mercator only) → layouts C3.1–C3.2.
2. Public data toolkit: generic ArcGIS REST catalog first (`docs/PUBLIC_DATA_CATALOG.md`).
3. Census data join (user enters the API key; OS keychain).
