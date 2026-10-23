# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**: both edit `crates/engine/src/lib.rs`. The general-purpose prompt is `docs/SONNET_RUNNER_PROMPT.md`.

> Status 2026-10-23: Phase A is down to A3 (your hands-on installer check) and tagging v0.1.0, neither of which conflicts with these. B3 and B4 are done, so B2.1 is the next Sonnet task.

---

## 1. B2.1 — Persist the node cache (attribute history, part 1)

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/ATTRIBUTE_HISTORY_PLAN.md (§1, §2 D1–D4, §3 B2.1). Read only crates/engine/src/graph.rs, the init_schema / save_project / clear_model_cache / new_project parts of crates/engine/src/lib.rs, and the existing engine tests that run models (grep "run_graph" in crates/engine/tests). Grep before opening other files.

TASK B2.1: persist the model's node cache (__node_<hash> tables) in the project, with an index table, version check, prune and size budget, exactly as the plan's D1–D4 say. Engine only; no UI, no new Tauri commands except that prune_node_cache is an Engine method (its command comes in B2.2).

Acceptance: H1–H7 in the plan, plus all existing engine tests green (especially the cancel and result-transaction tests).

Notes:
- The index row and the CREATE of a cache table go in one transaction (H6).
- engine_version = env!("CARGO_PKG_VERSION") + a CACHE_FORMAT const in graph.rs; bump CACHE_FORMAT whenever a tool's SQL changes meaning.
- Keep serde_json without preserve_order; H3 guards it.
- Raster cache entries stay session-only.

Test: cargo test -p engine <filter>, then cargo test -p engine (PATH line from CLAUDE.md). Commit (Co-Authored-By line, never .claude/), push, wait for CI in the background with a long interval (public API, see docs/SONNET_RUNNER_PROMPT.md), and log CI rounds in docs/CI_LOG.md.
Finish by updating PROJECT_STATUS.md (next action: B2.2) and replacing this block in docs/NEXT_SESSION_PROMPT.md with a B2.2 prompt built the same way from the plan.
Stop and ask (OPUS NEEDED) if the plan doesn't settle something, e.g. DuckDB can't COPY views between databases on Save As.
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
