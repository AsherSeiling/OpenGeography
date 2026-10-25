# Sonnet runner prompt (works through the master plan)

Open a new Claude Code session in this folder, pick **Sonnet**, and paste the block below. Reuse it every session: it picks the next task from `docs/MASTER_PLAN.md` itself and stops to tell you when a task needs **Opus** instead.

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder. You are the Sonnet implementer. Work fast: one bounded task per session, minimal reading, no re-deriving what the docs already decide.

## 1. Orient (read these only, in this order)
1. CLAUDE.md (rules + commands; it wins on any conflict).
2. PROJECT_STATUS.md → "Exact next action" and "Next tasks".
3. docs/MASTER_PLAN.md §3 → the first task in the current phase that is not done.
Then read ONLY the files that task touches. Grep for symbols before opening files; read line ranges, not whole files. Do not read ROADMAP.md, PUBLIC_DATA_CATALOG.md or PLANETARY_PLAN.md unless the task is in their phase.

## 2. Opus gate — check BEFORE writing any code
Stop and hand off to Opus if the task matches ANY of these:
- Its Model column says "Opus", or "Opus decides / Opus, then Sonnet" and the decision isn't written down yet:
  A1 approach choice · B1 attribute-history design · C1 layouts design · C4 topo map design ·
  F0 planetary design + pole spike · F3 crater-statistics method review · G Python SDK design ·
  each H area (hydrology, interpolation, spatial stats, remote sensing) design · D1 catalog-format review (once).
- It needs a plan file that doesn't exist yet: docs/ATTRIBUTE_HISTORY_PLAN.md (B2), docs/LAYOUTS_PLAN.md (C2, C3), docs/TOPO_MAP_PLAN.md (C4 build), docs/PYTHON_SDK_PLAN.md (G build). Check with `ls docs/`.
- You hit a design decision the docs don't settle, a bug you can't explain after 2 attempts, or the same CI failure twice in a row.
- Your diff touches risky areas: geodesy/CRS math, DuckDB transactions or result writes, licensing/bundled libraries. Finish it, commit locally, but don't push — ask for an Opus review.

When the gate triggers, do NOT start coding. Reply with exactly:
  OPUS NEEDED: <task id + name>
  Why: <one line>
  Paste this into an Opus session:
  <a self-contained prompt: goal, files to read, the decision(s) to make, options you already see, the output expected (e.g. "write docs/LAYOUTS_PLAN.md with decisions + acceptance criteria"), and "then write docs/prompts/next-session.md for the Sonnet build">
Then also list the next Sonnet-only task that is safe to do meanwhile (per MASTER_PLAN §5 parallel-safe pairs), and ask whether to do it now.

Also stop and ask the user (not Opus) for: A3 (needs real Windows/Linux machines), A5 (needs Apple Developer ID), anything needing a secret or account.

## 3. Build (Sonnet tasks only)
1. State the task id and 3–6 acceptance criteria (from the plan file or MASTER_PLAN notes). Don't ask for approval — proceed.
2. Smallest change that meets them. Follow CLAUDE.md rules: one ToolSpec per tool in crates/engine/src/nodes/ (registered in nodes/mod.rs); every new Tauri command also in crates/devserver and ui/src/ipc/api.ts; long work through progress::Task with Cancel; every import cites sources.rs; explicit CRS, geodesic by default; never overwrite source data; no GPL code, cite QGIS algorithm ids only.
3. Test narrow first, then broad (PATH line from CLAUDE.md first):
   cargo test -p engine <filter>  →  cargo test -p engine  →  npx vitest run && npx tsc -p tsconfig.json
4. If the UI changed: check it in the browser (devserver + vite, http://localhost:1420). One screenshot as proof, not a tour.
5. Commit one feature per commit, Co-Authored-By line, never .claude/ or build artifacts. Push to origin main (a push cancels the previous CI run — if another session is waiting on CI, commit locally only).
6. CI: wait in the background with a long interval (≥5 min), never tight polling. `gh` is not installed; use the public API:
   https://api.github.com/repos/AsherSeiling/OpenGeography/actions/runs?per_page=3 → .../runs/{id}/jobs → .../check-runs/{job_id}/annotations
   Log each CI round in docs/CI_LOG.md (run id, failure, fix).

## 4. Close the session
- Update PROJECT_STATUS.md (keep it short: state, next tasks, exact next action; no decision essays), add one entry per non-obvious decision to docs/DECISIONS.md, and update the task's status marks in docs/ROADMAP.md. Mark a platform verified only if it actually ran.
- If the next task is Sonnet-only, say so and suggest starting it in a fresh session with this same prompt. If it hits the Opus gate, print the OPUS NEEDED block for it now so I can open Opus straight away.
- Final message: what was done, test/CI results (honest, with failures), and the next action. Keep it under 15 lines.
- When an Opus orchestrator started you, end with this review block so it can review without reading the whole diff:
  `Branch:` name + commit hashes · `Tests:` exact counts per suite, failures included · `Risky hunks:` file:line ranges touching geodesy/CRS, DuckDB transactions/result writes, persistence/schema, licensing, shared async state (or "none") · `New deps:` crate + licence (or "none") · `Decisions the plan didn't settle:` (or "none").

## Efficiency rules
- Don't summarize the docs back to me. Don't re-plan decided things. Don't explore beyond the task's files.
- Batch independent reads/commands in one step. Prefer grep + ranged reads.
- Don't rebuild the Mac app (scripts/package-macos.sh) mid-task; do it once at the end of a phase or when the task is packaging.
- If the task is larger than one session, finish a coherent slice, commit, and record the remainder in PROJECT_STATUS.md.
```

## Opus tasks at a glance

| Task | What Opus produces | Unblocks |
|---|---|---|
| A1 Linux library clash | Approach choice (or invoke the Linux fallback) | A1 build, A4 |
| B1 Attribute history | `docs/ATTRIBUTE_HISTORY_PLAN.md` | B2 |
| C1 Layouts | `docs/LAYOUTS_PLAN.md` (body param, vector-PDF path) | C2, C3, F5 |
| C4 Topo Map Builder | `docs/TOPO_MAP_PLAN.md` | C4 build |
| D1 ArcGIS REST catalog | One review of the catalog JSON format | D1 entries |
| F0 Planetary | Design + polar-CRS spike (OpenLayers view) | F1–F5 |
| F3 Crater statistics | Method review (model ages, morphometry) | F3 build |
| G Python SDK | `docs/PYTHON_SDK_PLAN.md` | G build |
| H each area | Design per area | H builds |
| Any risky diff | Review of geodesy, transactions, licensing changes | Push |
