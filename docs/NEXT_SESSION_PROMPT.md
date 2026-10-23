# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**. The general-purpose prompt is `docs/SONNET_RUNNER_PROMPT.md`.

> Status 2026-10-23: Phase A is down to A3 (your hands-on installer check) and tagging v0.1.0, neither of which conflicts with these. B2 (attribute history) and B3/B4 are done, so C2.1 is the next Sonnet task.

---

## 1. C2.1 — Layout renderer core (layouts, part 1)

Phase B is finished, so this is next in MASTER_PLAN order.

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
