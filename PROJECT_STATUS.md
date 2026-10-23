# Project status

_Updated 2026-10-23. Read this first; the order of work is `docs/MASTER_PLAN.md`, the feature inventory `docs/ROADMAP.md`._

## State
- Branch `main`, pushed to https://github.com/AsherSeiling/OpenGeography (public). Untracked: `.claude/` (local launch config — don't commit).
- macOS arm64 built and tested locally. CI is green on macOS, Linux and Windows (build, tests, installers, clean self-test) — see `docs/CI_LOG.md`.
- CI logs need a GitHub login; without one, read failures from the run's annotations (`scripts/ci-run.sh` publishes key output lines) and job summaries.

## Build and test status (macOS arm64, 2026-10-23)
| Check | Result |
|---|---|
| `npx tsc` | pass |
| `npx vitest run` | 57/57 pass |
| `cargo test -p engine` | 35 + 8 (history) integration + 7 unit pass (4 network tests + H14 ignored) |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass (rebuilt 2026-10-23 09:31 with node shortcuts) |
| Browser check (devserver + vite) | model Cancel from toolbar stops a 3M-feature run in ~0.1 s |

## Completed milestones
Office-style UI, map, layers, table, CRS · per-layer node trees · spreadsheet editing · definition queries · rasters + 3DEP · 53 native tools · self-contained macOS installer · geodesic measuring + resection/intersection · National Map downloads · progress bars + cancel for downloads · sources & citations · third-party notices.

## Known issues / technical debt
1. Cancel lands between DuckDB work units (~100k rows/thread). GDAL translate/warp/contours/polygonize/rasterize stop mid-step; gdaldem (hillshade/slope/aspect/ruggedness) ignores GDAL's progress callback, so those finish first.
2. Fixed 2026-10-23: Linux AppImage clash (conda GDAL vs. system libs) — private libs with unique `gs_` sonames (`5fd5c23`, run 40014559269); Linux builds with conda-forge GDAL 3.12 and passes the strict license gate. Local Homebrew macOS builds still warn; CI release builds on all platforms are GPL-free.
3. Fixed 2026-10-23: deleted-layer references are flagged; parameter typing is one undo step; bundled GDAL/PROJ data always wins; rasters are copied into the project (portable); tool output fields come from the engine registry (`Tool::output_fields`, with a drift test).

## Recent decisions
- 2026-10-23 **Table pane UI (B2.3, done; Phase B exit reached: roadmap P2/P3 ✅, v0.2 tag is the user's call, MASTER_PLAN §4)**: `ui/src/panes/table/` (`history.ts` pure helpers + `useTableTarget` store, `useNodeTable.ts` node/diff lifecycle, `NodePicker.tsx`, `DiffBar.tsx`); `AttributeTable.tsx` has node mode (read-only; edit/selection buttons disabled with tooltips) and diff mode (D8 styling, chips filter the grid). Node card right-click menu: View table / Compare with input (`nodes/NodeMenu.tsx`). `lastOutput` per node is saved in project.json; `pruneNodeCache(live, pinned)` runs on save and after a layer (tree) is removed. The diff is dropped on close/node switch/compare change; a diff or snapshot lost to reopen/Save As re-runs once, then shows a prompt or "Run to this node" (never a blank grid). Undoing a tree edit can drop `lastOutput` from the restored node (it then shows "not run" until the next run, an instant cache hit). Also fixed B2.2: Layer Input view pages use `row_number()` positions. Checked in the browser (H19): `docs/screenshots/h19-attribute-history.jpg`. Mac app rebuild skipped (orchestrator).
- 2026-10-23 **Diff + node rows (B2.2, done)**: `history.rs` + Engine `node_table_info`, `node_rows`, `node_diff`, `node_diff_rows`, `drop_node_diff` (Tauri, devserver, `api.ts`; plus `prune_node_cache`, `set_node_cache_budget`). Diffs are `__diff_<a>_<b>_<key>` tables, never indexed, swept on open/new project (a first Save As reopens the DB, so an open diff must be re-run after it). Layer Input views page in insertion order (unsorted) or by `row_number()` (sorted); `fids` of node rows are row positions. **H14: 1M rows × 20 fields + point geometry key diff = 0.44 s (release, M-series).**
- 2026-10-23 **Persisted node cache (B2.1, done)**: `__node_<hash>` tables are indexed in `__node_cache` (written in the same transaction as the CREATE). On open: other `engine_version` (`CARGO_PKG_VERSION+CACHE_FORMAT`, bump `graph::CACHE_FORMAT` when a tool's SQL changes meaning), missing/broken (deleted layer) and orphan tables are dropped. Save As keeps snapshots (DuckDB copies the Layer Input views too). `Engine::prune_node_cache(live, pinned)` drops non-live then applies the budget (default 2 GB, `set_node_cache_budget`; views count 0 bytes; budget 0 = prune on save only). Raster entries stay session-only. No UI/commands yet (B2.2).
- 2026-10-23 **Layouts design (C1)**: the engine renders each page to SVG, then PDF (svg2pdf) and PNG (resvg). MapLibre can't draw a true CRS. Bundled fonts only, for identical output on every OS. Frames take an explicit CRS. Scale bar and north arrow are geodesic at the frame centre, through a `Body` (Earth only until F1). Layouts are stored in `__layouts`. See `docs/LAYOUTS_PLAN.md`.
- 2026-10-23 **Attribute history design (B1)**: snapshots are the existing `__node_<hash>` cache tables, persisted with an index (`__node_cache`), an engine-version check, and a 2 GB LRU budget pruned to the UI's live outputs. The diff is tool-agnostic: user key → auto-detected unique key → content-hash multiset diff (added/removed only). Geometry is not compared across CRSs. See `docs/ATTRIBUTE_HISTORY_PLAN.md`.
- 2026-10-23 **Setting validation**: before a tool runs, `nodes::validate_params` rejects non-numeric Number settings and Enum values outside the options (readable error on the node). Empty = unset.
- 2026-10-23 **Overlay tools**: Union, Symmetrical Difference (`union.rs` holds the shared `minus_sql`/`overlay_ctes`) and Singlepart To Multipart (`collect`). Concave hull is a shape in Minimum Bounding Geometry (`ST_ConcaveHull`); Delaunay (`delaunay.rs`) triangulates through GDAL/GEOS and inlines the result as hex WKB (cap 200k features). B3 complete.
- 2026-10-23 **Select By Location on the map** (Map → Selection): intersects / within distance (geodesic) / within / contains / touches / crosses, optionally only the comparison layer's selected features, invert, and new / add / remove / subset selection methods. Engine `select_by_location` shares `nodes::select_by_location::location_test` with the model node.
- 2026-10-23 **Model-run cancellation** (done): Cancel on the Model toolbar and status bar. A watcher thread calls DuckDB `interrupt()` repeatedly until the run returns (DuckDB clears the flag at each query start, so a single interrupt can miss); the executor checks before each step and marks remaining steps `cancelled`. Result writes (`set_result`, `upsert_layer`) run in one transaction, so a cancelled/failed write keeps the previous result (a test proves the layer corrupts without it). Run tasks are labelled "Running model — <layer>".

- 2026-10-23 **Node editor**: Blender keys only over the editor (`nodes/keymap.ts`, rebindable in the Shortcuts dialog, saved in browser storage); one-step, type-checked inserts; changed/stale markers; mute = bypass in `graphPayload`.
- 2026-10-23 **Provenance**: Output/Add to Map record the sources of every upstream Layer input (same transaction as the result).
- 2026-10-23 **Geodesic speed**: aeqd centers snap to 0.25° (~30× faster; < 0.1 m error on 50 km buffers in tests).
- 2026-10-23 **GDAL pinned to 3.12** on every platform (conda-forge `libgdal-core=3.12` in CI; Homebrew 3.12.3 locally — don't `brew upgrade gdal duckdb`): gdal crate 0.19 doesn't support GDAL 3.13. DuckDB pinned to 1.5.6. The spatial extension ships gzip-compressed (Linux packagers corrupted it).
- 2026-10-23 **Catalog pane** (file manager) with a "This project" view; TIGER downloads by year; bibliography auto-export (APA/MLA/Chicago, BibTeX, RIS).

## Next tasks (in order)
1. Layouts (C2–C3): designed in `docs/LAYOUTS_PLAN.md` (engine renders SVG → PDF/PNG; true CRS; body-aware); build C2.1 → C2.4, then C3.1–C3.2. Prompt for C2.1 in `docs/NEXT_SESSION_PROMPT.md`.
2. Public data toolkit: generic ArcGIS REST catalog first (docs/PUBLIC_DATA_CATALOG.md).
3. Census data join (user enters the API key; OS keychain).

## Exact next action
Build **C2.1** (layout renderer core; prompt 1 in `docs/NEXT_SESSION_PROMPT.md`; it adds bundled crates and fonts, so it ends with an OPUS NEEDED licence review). Before that, the orchestrator rebuilds the Mac app (`scripts/package-macos.sh`, `--self-test`) once to close Phase B. Meanwhile A3: you check the Windows and Linux installers on real machines, then tag `v0.1.0` (`git tag v0.1.0 && git push origin v0.1.0`).
