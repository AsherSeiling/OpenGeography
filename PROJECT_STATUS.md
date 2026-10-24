# Project status

_Updated 2026-10-23. Read this first. Order of work: `docs/MASTER_PLAN.md`. Why things are the way they are: `docs/DECISIONS.md` (read only for the area you touch). Session prompts: `docs/prompts/`._

## Exact next action
- **Orchestrator (Opus, `docs/prompts/orchestrator.md`):** C2.2 (layout view, `__layouts` storage, export commands) is merged and the Mac app is rebuilt with it. Next: **C2.3 + C2.4** in one Sonnet session (surround elements, then graticule/grid, rasters and basemaps; brief in `docs/prompts/next-session.md`), then the projected map view M1–M4 (`docs/MAP_PROJECTION_PLAN.md`).
- **You:** A3 (check the Windows and Linux installers on real machines), then tag `v0.1.0` (`git tag v0.1.0 && git push origin v0.1.0`), then decide on `v0.2` (Phase B is done).

## State
- `main` is pushed to https://github.com/AsherSeiling/OpenGeography (public). Never commit `.claude/` (local launch config and worktrees).
- Phases A (except A3 and the tag) and B are done. Phase C: C1 designed, C2.1 merged, C2.2 (Layout view, commands, `__layouts` storage) merged.
- CI is green on macOS, Linux and Windows through Phase B (`docs/CI_LOG.md`). CI logs need a GitHub login: read failures from the run annotations (`scripts/ci-run.sh` publishes key lines).
- The packaged Mac app and DMG are copied into the project folder (`GIS Studio.app`, `GIS-Studio-<version>-macos-arm64.dmg`) and rebuilt once per phase.

## Build and test status (macOS arm64, 2026-10-23)
| Check | Result |
|---|---|
| `npx tsc` | pass |
| `npx vitest run` | 78/78 |
| `cargo test -p engine` | 14 unit + 35 + 9 (history) + 12 (layout) integration; 4 network tests, H14 and P6 ignored |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass |

## Known gaps (layouts, C2.2)
- The Layout view has no rulers yet and no Arrange group (bring forward/back, align). Legend, scale bar, north arrow and sources are C2.3; graticule, rasters and basemaps are C2.4.
- Export dialog has no dpi box yet: PNG is 300 dpi. In the browser build the file path comes from a prompt.

## Known issues
1. Cancel lands between DuckDB work units (~100k rows/thread). GDAL translate/warp/contours/polygonize/rasterize stop mid-step; gdaldem (hillshade/slope/aspect/ruggedness) ignores GDAL's progress callback and finishes first.
2. Undoing a node-tree edit can drop a node's `lastOutput`. The table pane then shows "not run" until the next run, which is a cache hit.
3. Fixed 2026-10-23: the one `package-macos.sh` failure (dylibbundler, libabsl_time) came from two sessions packaging at once in the same folder; each run's `rm -rf Frameworks` deleted the other's libraries. The script now takes a lock (`target/.package-macos.lock`), so a second run exits at once.

## Next tasks (in order)
1. Layouts: C2.3 → C2.4 → C3.1–C3.2 (`docs/LAYOUTS_PLAN.md`).
2. Public data toolkit: generic ArcGIS REST catalog first (`docs/PUBLIC_DATA_CATALOG.md`).
3. Census data join (user enters the API key; OS keychain).
