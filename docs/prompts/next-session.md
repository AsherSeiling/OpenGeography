# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**. The general-purpose prompt is `docs/prompts/sonnet-runner.md`.

> Status 2026-10-23: M1 (engine) is merged; M2 (OpenLayers view in any CRS) is done on branch `claude/m2-openlayers-view-cc786c`, awaiting Opus review. Next is M3 (`docs/MAP_PROJECTION_PLAN.md`). A3 (your installer check) and the `v0.1.0` tag are independent.

---

## 1. L1 — Feature labels on the map (user request)

Follow docs/prompts/sonnet-runner.md. The spec is docs/LAYOUTS_PLAN.md §4 "L1".
- **Read:** `ui/src/store/store.ts` (LayerStyle, defaultStyle), `ui/src/map/style.ts` and `olStyle.ts`, `ui/src/panes/MapView.tsx` (layers), the layer Properties pane (symbology section), `crates/engine/src/tiles.rs` and `display.rs` (tile attributes, display tables), the `gis://` route and the devserver route.
- **Build:**
  - `LayerStyle.label`, with old projects defaulting to off.
  - The Labels section in Properties.
  - OpenLayers text styles with declutter.
  - A polygon label-anchor tile route (`ST_PointOnSurface`, in any CRS via the display table), added to the devserver and `api.ts`.
  - The label field included in tiles only when labels are on.
- **Acceptance:** L1-P1 to L1-P4. Add vitest for the style mapping, an engine test for the anchor tiles, and an e2e case. Add `docs/TEST_PLAN.md` rows.
- **Finish:** commit on your branch, no push, no Mac rebuild. In a fresh worktree run `npm ci && npm run build && scripts/stage-resources.sh` first. End with the runner's review block, plus one screenshot of labels in UTM.

---

## 2. M4 — rasters in any CRS, performance, polar check (ON HOLD at the user's request, 2026-10-25)

Brief: `docs/MAP_PROJECTION_PLAN.md` §3 M4 (P14–P16). M3 is done (identify and rectangle select take `crs` and map-CRS units;, browser-checked in UTM 15N and Web Mercator). Needs C2.4's `render_extent`. To be written in full by the orchestrator.
