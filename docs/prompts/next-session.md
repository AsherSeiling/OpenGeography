# Prompts for the next Sonnet sessions

Open a new Claude Code session in this folder, pick **Sonnet**, and paste one block. Run them **one at a time**. The general-purpose prompt is `docs/prompts/sonnet-runner.md`.

> Status 2026-10-23: M1 (engine) is merged; M2 (OpenLayers view in any CRS) is done on branch `claude/m2-openlayers-view-cc786c`, awaiting Opus review. Next is M3 (`docs/MAP_PROJECTION_PLAN.md`). A3 (your installer check) and the `v0.1.0` tag are independent.

---

## 1. Bug-fix session (from `docs/BUGS.md`, plus B5)

Follow docs/prompts/sonnet-runner.md. Fix these bugs. Each has an ignored test in `docs/BUGS.md`: unignore it and make it pass, and add a test where none exists.
- **B-T5 (High):** `raster.rs::warp` / `translate` / `dem` call GDAL option setters on a NULL options pointer. Check every `*AppOptionsNew` result. A bad CRS is a readable error, never a crash.
- **B-T7 (High):** Warp (and any raster tool that reprojects: check Clip, Contours, Polygonize) refuses a raster with no CRS, the same way the vector tools do (`need_crs`).
- **B-T1:** Export refuses a path equal to any layer's source file (canonicalized), with a message.
- **B-T2:** rename refuses a blank name and keeps the old one.
- **B-T4:** cancel within 2 s for geodesic Buffer and Dissolve, per `docs/DECISIONS.md` ("Slow cancel"): batches the interrupt can stop between. Results must equal today's output.
- **B-T6:** raster identify outside the CRS's domain returns "no value".
- **B-T8:** `Task::part` clamps to 100 %.
- **B5:** keep each node's `lastOutput` outside undo history (a `lastOutputs` map beside the trees, saved in project.json). Old projects still load.

Risky (Opus review): the raster CRS checks (B-T5, B-T7) and the Buffer/Dissolve batching (B-T4: results must not change). Mark each bug fixed in `docs/BUGS.md` and add `docs/TEST_PLAN.md` rows. Commit on your branch, no push.

---

## 2. M4 — rasters in any CRS, performance, polar check (after brief 1)

Brief: `docs/MAP_PROJECTION_PLAN.md` §3 M4 (P14–P16). M3 is done (identify and rectangle select take `crs` and map-CRS units;, browser-checked in UTM 15N and Web Mercator). Needs C2.4's `render_extent`. To be written in full by the orchestrator.
