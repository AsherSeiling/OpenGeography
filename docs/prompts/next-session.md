# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**. The general-purpose prompt is `docs/prompts/sonnet-runner.md`.

> Status 2026-10-23: Phase A is down to A3 (your hands-on installer check) and tagging v0.1.0, neither of which conflicts with these. B2 (attribute history) and B3/B4 are done, C2.1 (layout renderer core) is done, so C2.2 is the next Sonnet task.

---

## 1. C2.2 — Layout view UI + commands (layouts, part 2)

C2.1 is finished: the engine renders a layout to SVG, PDF and PNG (`crates/engine/src/layout/`: `render_svg`, `Engine::render_layout_svg`, `export::svg_to_pdf`, `export::svg_to_png`).

```text
You are continuing GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

Read CLAUDE.md, PROJECT_STATUS.md, then docs/LAYOUTS_PLAN.md (§1–§3, L5 and §4 C2.2). Read only crates/engine/src/layout/mod.rs and model.rs (the model and RenderRequest you will send), src-tauri/src/commands.rs (command pattern, progress::Task use), crates/devserver/src/main.rs, ui/src/ipc/api.ts, ui/src/store/model.ts (zundo pattern), ui/src/ribbon/Ribbon.tsx (the existing newLayout command), ui/src/panes/ (dockview document tabs, Contents, Properties). Grep before opening other files.

TASK C2.2: storage, commands and the Layout view.
- Engine: `__layouts (id VARCHAR PRIMARY KEY, json JSON, updated TIMESTAMP)` in project.duckdb (create in init_schema; list/save/delete; validate with `Layout::from_json` on save). Add `Engine::export_layout(req, format, dpi, path, task)` (SVG / PDF / PNG; write to a temp file then rename; `progress::Task` "Exporting layout — <name>" with Cancel, checked between layers and before encoding).
- Commands, each in src-tauri, crates/devserver and ui/src/ipc/api.ts: list_layouts, save_layout, delete_layout, render_layout_preview (PNG bytes + warnings; measure IPC bytes vs a gis:// URL and choose, record the numbers), export_layout.
- UI: ui/src/layout/types.ts (mirror of the model), ui/src/store/layout.ts (zundo undo per layout, debounced 500 ms save, marks project dirty), Layout document tab with the page on a grey canvas + preview image + HTML selection overlay (select/move/resize, Delete, arrow-key nudge), Properties target { kind: "layoutElement" }, contextual ribbon tab "Layout" (Page size/orientation; Insert Map Frame, Text; Map Frame: Set from Map View, scale box, CRS picker; Export PDF/PNG/SVG), Contents "Layouts" section. Send styles/order/visible from the app store in each render request; warnings from the render are shown (CRS missing, geographic frame, scale variation).
- Legend, scale bar, north arrow and sources elements are C2.3: the ribbon does not offer them yet.

Acceptance: P7–P10 in the plan. Browser check (devserver + vite, http://localhost:1420): one screenshot of the Layout view, saved under docs/screenshots/.

Rules: explicit CRS (a frame takes the map CRS; if none, refuse with "Set a coordinate system for the map frame"); never guess; long work through progress::Task with Cancel; Fluent UI v9 + the existing ribbon/dockview style; no secrets; keep layout files separate from other tasks' files (this task adds Tauri commands, so it is never parallel with another command-adding task).

Test narrow, then cargo test -p engine, npx vitest run && npx tsc -p tsconfig.json, cargo build -p gis-studio -p devserver (needs `npm run build` and `mkdir -p src-tauri/resources` in a fresh worktree). Commit. Update PROJECT_STATUS.md and write the C2.3 prompt into docs/prompts/next-session.md the same way.
```
