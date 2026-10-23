# Opus design prompts (paste one into an Opus session)

## C1 — Layouts design — ✅ done 2026-10-23 → `docs/LAYOUTS_PLAN.md`

### (original prompt) (priority 2: map production; the biggest missing piece)

You are the Opus designer for GIS Studio (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI). Read CLAUDE.md, PROJECT_STATUS.md, docs/MASTER_PLAN.md (Phase C, F4 notes), then the roadmap rows for layouts and `ui/src/map/`, `ui/src/panes/`, `src-tauri/src/commands.rs`.

Goal: write `docs/LAYOUTS_PLAN.md` with decisions and acceptance criteria for tasks C2 (page, map frame in the true CRS, legend, scale bar, north arrow, title, text, graticule, PNG + PDF export) and C3 (templates, map series).
Decide: (1) rendering path for vector PDF — MapLibre canvas capture vs. engine-rendered (Rust/Skia/cairo-style) — with license, cross-platform and bundle-size consequences; (2) layout document model and how it is saved in the project; (3) scale/CRS correctness for print (true scale at page size, geodesic scale bar); (4) a `body` parameter from day one so planetary maps (F4) are styling, not a rewrite; (5) the Layout ribbon tab and pane structure in the existing Office/ArcGIS-Pro style; (6) split of C2 into Sonnet-sized sessions with acceptance criteria each.
Options I already see: A) capture MapLibre canvases at print resolution + SVG overlay → PDF via a JS lib; B) engine renders vectors to SVG/PDF itself from the same style spec. Choose and justify.
Then write `docs/NEXT_SESSION_PROMPT.md` for the first Sonnet build session. Do not write feature code.

## B1 — Attribute history ("git for tables") design — ✅ done 2026-10-23 → `docs/ATTRIBUTE_HISTORY_PLAN.md`

### (original prompt)

Same orientation reading, plus `crates/engine/src/graph.rs` (node cache, `__node_<hash>` tables) and `ui/src/panes/` table pane.
Goal: write `docs/ATTRIBUTE_HISTORY_PLAN.md`: how a user sees how a table changed between any two nodes of a layer's tree.
Decide: snapshot storage (persisting `__node_<hash>` tables beyond the session, inside the project file), diff format (added/removed/changed rows and fields, keyed by `__fid` or a stable key), size limits and eviction, the table pane node-picker UX, and how it interacts with undo, muting and stale nodes. Split into Sonnet sessions (B2) with acceptance criteria and tests.
Then write `docs/NEXT_SESSION_PROMPT.md` for the Sonnet build. Do not write feature code.
