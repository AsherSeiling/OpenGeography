# Project status

_Updated 2026-10-23. Read this first. Order of work: `docs/MASTER_PLAN.md`. Why things are the way they are: `docs/DECISIONS.md` (read only for the area you touch). Session prompts: `docs/prompts/`._

## Exact next action
- **Orchestrator (Opus, `docs/prompts/orchestrator.md`):**
  1. ✅ C2.1 CI green on all three platforms (run 40047865204): the P3 golden SVG hash matches on Linux and Windows too.
  2. Delegate **C2.2** (layout view UI + commands; brief in `docs/prompts/next-session.md`).
- **You:** A3 (check the Windows and Linux installers on real machines), then tag `v0.1.0` (`git tag v0.1.0 && git push origin v0.1.0`), then decide on `v0.2` (Phase B is done).

## State
- `main` is pushed to https://github.com/AsherSeiling/OpenGeography (public). Never commit `.claude/` (local launch config and worktrees).
- Phases A (except A3 and the tag) and B are done. Phase C: C1 designed, C2.1 merged.
- CI is green on macOS, Linux and Windows through Phase B (`docs/CI_LOG.md`). CI logs need a GitHub login: read failures from the run annotations (`scripts/ci-run.sh` publishes key lines).
- The packaged Mac app and DMG are copied into the project folder (`GIS Studio.app`, `GIS-Studio-<version>-macos-arm64.dmg`) and rebuilt once per phase.

## Build and test status (macOS arm64, 2026-10-23)
| Check | Result |
|---|---|
| `npx tsc` | pass |
| `npx vitest run` | 68/68 |
| `cargo test -p engine` | 14 unit + 35 + 9 (history) + 8 (layout) integration; 4 network tests, H14 and P6 ignored |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass |

## Known issues
1. Cancel lands between DuckDB work units (~100k rows/thread). GDAL translate/warp/contours/polygonize/rasterize stop mid-step; gdaldem (hillshade/slope/aspect/ruggedness) ignores GDAL's progress callback and finishes first.
2. Undoing a node-tree edit can drop a node's `lastOutput`. The table pane then shows "not run" until the next run, which is a cache hit.
3. Fixed 2026-10-23: the one `package-macos.sh` failure (dylibbundler, libabsl_time) came from two sessions packaging at once in the same folder; each run's `rm -rf Frameworks` deleted the other's libraries. The script now takes a lock (`target/.package-macos.lock`), so a second run exits at once.

## Next tasks (in order)
1. Layouts: C2.2 → C2.3 → C2.4 → C3.1–C3.2 (`docs/LAYOUTS_PLAN.md`).
2. Public data toolkit: generic ArcGIS REST catalog first (`docs/PUBLIC_DATA_CATALOG.md`).
3. Census data join (user enters the API key; OS keychain).
