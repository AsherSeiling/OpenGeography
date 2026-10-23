# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**: both edit `crates/engine/src/lib.rs`. The general-purpose prompt is `docs/SONNET_RUNNER_PROMPT.md`.

> Status 2026-10-23: Phase A is down to A3 (your hands-on installer check) and tagging v0.1.0, neither of which conflicts with these. B3 and B4 are done, B2.1 and B2.2 are done, so B2.3 is the next Sonnet task.

---

## 1. B2.3 — Table pane UI (attribute history, part 3)

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/ATTRIBUTE_HISTORY_PLAN.md (§2 D3, D7, D8, §3 B2.3). Read only ui/src/panes/AttributeTable.tsx, ui/src/store/model.ts, ui/src/nodes/ (node card + context menu), and the history types and functions in ui/src/ipc/api.ts (nodeTableInfo, nodeRows, nodeDiff, nodeDiffRows, dropNodeDiff, pruneNodeCache, setNodeCacheBudget: all done in B2.2). Grep before opening other files.

FIX FIRST (Opus review of B2.2, engine, small): in crates/engine/src/history.rs `node_rows`, the `(true, None)` arm (a Layer Input VIEW with no sort) selects `0::BIGINT AS __pos`, so every row's `fids` entry is 0. The grid would get duplicate row keys. Give views a real, stable position: page with `row_number() OVER () - 1 AS __pos` and `ORDER BY __pos` as in the sorted arm, or re-select the base layer's `__fid` per plan D7. Extend test H13 to assert that a view page's `fids` are distinct and increasing, and stable across two calls. Commit it separately ("B2.2 fix: …") before the UI work.

TASK B2.3: UI only (after the fix above). Node picker ("Table of:") in the table toolbar; "Compare with" dropdown defaulting to the node's primary input; diff bar (+added / −removed / ~changed chips that filter the grid, field-change chip, "Match rows by" key picker, the summary `note` line); diff-mode grid styling (D8); node mode read-only with the edit/selection buttons disabled and tooltipped; node card context menu "View table" and "Compare with input"; `lastOutput` stored in each node's data (saved in project.json); call `pruneNodeCache(live, pinned)` on save and after a tree is deleted; "Run to this node" for not-stored nodes (run_graph with `target`). If the table pane exceeds ~700 lines, split into ui/src/panes/table/NodePicker.tsx + DiffBar.tsx.

Engine facts you need (B2.2):
- node_rows pages a snapshot; its `fids` are row positions, not layer fids. Sort/filter work as in get_rows. The geometry column is not returned.
- node_diff(a, b, key?) returns a DiffSummary (keySource user|auto|none, note, fields order, fieldsAdded/Removed/Retyped, counts, geometryCompared). node_diff_rows(diffId, kind added|removed|changed|all, offset, limit) returns {kind, key, values:[{field, a, b, changed}]}; when geometry is compared the last field is "geom" (WKT preview). Call dropNodeDiff when the pane closes the diff or switches nodes.
- A diff is lost when the project is reopened or first saved (Save As reopens the database): node_diff_rows then errors "no longer available" and the UI must re-run nodeDiff.
- The diff runs in a cancellable progress task "Comparing tables" (the existing task UI shows Cancel).
- A snapshot may be missing (evicted/never run): nodeTableInfo/nodeRows then error "is not stored"; show the "not stored" badge and Run-to-this-node.

Acceptance: H16–H20 in the plan. Vitest for pure functions (`pickerEntries(tree, results)`, default compare input, prune payload with a mocked api). Check H19 in the browser (devserver + vite, http://localhost:1420), one screenshot. Then rebuild the Mac app (scripts/package-macos.sh), run --self-test, set the ROADMAP row "Attribute history" to ✅, update PROJECT_STATUS.md.

Test: npx vitest run && npx tsc -p tsconfig.json; cargo build -p gis-studio -p devserver (PATH line from CLAUDE.md). Commit (Co-Authored-By line, never .claude/), push, wait for CI in the background with a long interval (see docs/SONNET_RUNNER_PROMPT.md), log CI rounds in docs/CI_LOG.md.
B2.3 ends Phase B. Finish by deleting this block from docs/NEXT_SESSION_PROMPT.md (the C2.1 block below becomes prompt 1). In PROJECT_STATUS.md, note that Phase B's exit (roadmap P2 and P3 all ✅) is reached and the v0.2 tag is the user's call (MASTER_PLAN §4). Do not tag.
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
