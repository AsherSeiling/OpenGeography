# GIS Studio — instructions for Claude Code

Free, open-source, node-based desktop GIS for geoscientists and government staff.
Priorities: easy data manipulation → easy map production → fast, correct geoprocessing.
Start each session with `PROJECT_STATUS.md` (state + exact next action). Order of work: `docs/MASTER_PLAN.md` (links every other plan). Feature inventory: `docs/ROADMAP.md`.

## Architecture (one line each)
- `src-tauri/` Tauri 2 shell: IPC commands (`commands.rs`), `gis://` tile protocol, `--self-test`.
- `crates/engine/` all GIS logic, UI-independent: DuckDB Spatial storage (`layers.rs`), CRS (`crs.rs`), tiles, rasters via GDAL (`raster.rs`), model executor (`graph.rs`), tools (`nodes/*.rs`, one file per tool, registered in `nodes/mod.rs`), downloads (`tiger.rs`, `tnm*.rs`), progress/cancel (`progress.rs`), citations (`sources.rs`).
- `crates/devserver/` same commands over HTTP (127.0.0.1:7878) so the UI runs in a browser. **Every new Tauri command must also be added here.**
- `ui/src/` React 19 + Fluent UI v9. Model state `store/model.ts`, editor `panes/ModelPane.tsx` + `nodes/`, IPC `ipc/api.ts`.

## Commands
```bash
export PATH=/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:$PATH   # cargo is not on the default PATH here
cargo test -p engine                     # engine (add -- --ignored <name> for network tests)
npx vitest run && npx tsc -p tsconfig.json
cargo build --release -p devserver && ./target/release/devserver   # + `npx vite` → http://localhost:1420 (window.__gis dev hooks)
scripts/package-macos.sh                 # macOS app + DMG; then run the app with --self-test
python3 scripts/third_party_notices.py   # after dependency changes
```

## Rules
- Preserve the UI style: Office/ArcGIS Pro ribbon + dockable panes; Blender-style node cards with settings inside; Shift+A add menu; one node tree per layer with fixed Layer Input/Output; Output never modifies source data.
- One authoritative tool definition (`ToolSpec` in `nodes/`). The UI reads it via `tool_specs`; don't add parallel catalogs.
- GIS correctness first: explicit CRS (never guess), geodesic by default for distance/area, report dropped features, never overwrite source data.
- Cross-platform is a requirement: no hard-coded paths or OS checks outside `resources.rs`/`raster.rs::store_dir`; mark a platform verified only after it actually ran.
- Standalone: never require QGIS; reimplement QGIS algorithms and cite their ids — never copy GPL code. Project license MIT OR Apache-2.0.
- Long work reports through `progress::Task` (progress bar) and should be cancellable.
- Every import records a citation in `sources.rs`; derived layers keep their inputs' sources.
- Secrets (Census API key, Earthdata token) are entered by the user and stored in the OS keychain — never in code, logs or projects.

## Working efficiently
- Read `PROJECT_STATUS.md`, then only the files for the task (grep for symbols first).
- Small bounded tasks: acceptance criteria → smallest change → focused tests → broader tests → commit (one per feature, never commit `.claude/` or build artifacts).
- Update `PROJECT_STATUS.md` (and the roadmap's status marks) before ending a session.
