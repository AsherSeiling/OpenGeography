# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**: both edit `crates/engine/src/lib.rs`. The general-purpose prompt is `docs/SONNET_RUNNER_PROMPT.md`.

> Status 2026-10-23: Phase A is down to A3 (your hands-on installer check) and tagging v0.1.0, neither of which conflicts with these. B3 and B4 are done, B2.1 is done, so B2.2 is the next Sonnet task.

---

## 1. B2.2 — Diff and node-rows commands (attribute history, part 2)

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/ATTRIBUTE_HISTORY_PLAN.md (§2 D5–D7, §3 B2.2). Read only crates/engine/src/table.rs (RowsQuery / RowsPage), the graph.rs `Cache` and `run` parts (B2.1 persisted the node cache: `__node_cache` index, `Cache::prune`), the existing Tauri command + devserver route + ui/src/ipc/api.ts pattern for `rows`, and the progress/cancel pattern in `Engine::run_graph`. Grep before opening other files.

TASK B2.2: new crates/engine/src/history.rs with node_table_info, node_rows, node_diff, node_diff_rows (D5–D7), Engine methods in lib.rs, plus `prune_node_cache` and a budget setter as Tauri commands (src-tauri/src/commands.rs + main.rs registration), crates/devserver routes and ui/src/ipc/api.ts functions. Engine + IPC only; no UI components.

Acceptance: H8–H15 in the plan; every command reachable through the devserver (curl check); all existing engine tests green.

Notes:
- B2.1 is done: `Engine::prune_node_cache(live, pinned)` and `Engine::set_node_cache_budget(bytes)` exist; wire them as commands (`prune_node_cache`, `set_node_cache_budget`). Layer Input snapshots are VIEWs; node tables are plain `__node_<hash>` tables (stable `rowid`).
- Diff tables `__diff_*` are temporary: never indexed in `__node_cache`; drop them on new_project/open_db. Note the `__node_%` orphan sweep in `Cache::load` must not touch them (different prefix).
- Run the diff in a progress::Task "Comparing tables" and cancel via the connection interrupt, as run_graph does (H15).
- The ignored 1M-row performance test (H14): record the number in PROJECT_STATUS.md.
- Bump graph::CACHE_FORMAT only if a tool's SQL changes meaning (not needed here).

Test: cargo test -p engine <filter>, then cargo test -p engine, then cargo build -p gis-studio -p devserver (PATH line from CLAUDE.md). Commit (Co-Authored-By line, never .claude/), push, wait for CI in the background with a long interval (public API, see docs/SONNET_RUNNER_PROMPT.md), and log CI rounds in docs/CI_LOG.md.
Finish by updating PROJECT_STATUS.md (next action: B2.3) and replacing this block in docs/NEXT_SESSION_PROMPT.md with a B2.3 prompt built the same way from the plan.
Stop and ask (OPUS NEEDED) if the plan doesn't settle something, e.g. paging a Layer Input VIEW stably.
```

---

## 2. C2.1 — Layout renderer core (layouts, part 1)

Run after B2 (MASTER_PLAN order), or earlier if you decide map production comes first. Not at the same time as B2.1/B2.2.

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/LAYOUTS_PLAN.md (all of §1–§3, and §4 C2.1). Read only crates/engine/src/layers.rs (LayerInfo, __g/R-tree), crates/engine/src/crs.rs, crates/engine/src/sources.rs (citation lookup), ui/src/store/store.ts (LayerStyle, defaultStyle) and scripts/third_party_notices.py. Grep before opening other files.

TASK C2.1: the engine-side layout renderer: layout model (L2), Body + scale factor + convergence (L3, L4), map frame → SVG for vector layers with the existing single/categorized/graduated styles, SVG → PDF (svg2pdf) and → PNG (resvg), bundled Noto Sans fonts. Engine only: no UI and no Tauri commands yet (C2.2).

Step 0, before any code: the licence gate in L1. Add the crates, run python3 scripts/third_party_notices.py and the licence gate, and confirm every new crate and transitive dependency is permissive (MIT/Apache/BSD/ISC/Zlib/OFL for fonts). If anything isn't, stop: OPUS NEEDED.

Acceptance: P1–P6 in the plan. P3 (golden SHA-256 of the SVG) must pass on all three CI platforms. P4 uses PROJ's proj_factors as the reference.

Rules: geographiclib-rs Geodesic::new(a, f) from Body for all layout geodesics (no WGS 84 literals outside layout/geo.rs); explicit CRS, never guessed; skipped layers are reported as warnings; no system fonts.

Test narrow, then cargo test -p engine. Commit, push, wait for CI in the background, log rounds in docs/CI_LOG.md. Update PROJECT_STATUS.md and write the C2.2 prompt into docs/NEXT_SESSION_PROMPT.md the same way.
This touches licensing (new bundled crates and fonts): commit, but before pushing, print an OPUS NEEDED review request listing the new dependencies and their licences.
```
