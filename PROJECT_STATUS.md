# Project status

_Updated 2026-10-29. Read this first. Order of work: `docs/MASTER_PLAN.md`. Why things are the way they are: `docs/DECISIONS.md` (read only for the area you touch). Bugs: `docs/BUGS.md`. Test cases: `docs/TEST_PLAN.md`. Session prompts: `docs/prompts/`. The user follows the "GIS Studio — Project Status" doc (https://claude.ai/artifact/5Pvx6sQMhUVcsJeijMbrAj), which the orchestrator updates at every change._

## Exact next action
- **Lead developer (Claude Code, standing order of work, MASTER_PLAN §4):** Phase C is built (pushed). **Phase R in progress** (`docs/CRS_PLAN.md`): R0, R1 (reference suite `crs_reference.rs`; bugs C-1…C-3 found, open) R2 (`crs::key`/`same`, importer fixes C-4, C-5, G1 closed) and R3 (C-1 fixed: PROJ data folder, datum check at start-up and in `--self-test`, GeoJSON export policy; C-1b accepted) and R4 (tool matrix over 8 CRSs with the guard; C-2 and C-6 fixed, C-3 closed) and R5 (G11 terrain on geographic DEMs, C-7 modified raster CRS kept, P18 pole-safe identify/select) and R6 core (engine `crs_info`/`map_factors`, picker preview of custom definitions, status-bar k/γ and area-of-use notice) done; R6b (Tissot layer, source panel details, layout {crs}/{datum}/{units} tokens, picker recents) done; R8 world view done (world: CRS prefix, cut/densify, graticule + reference lines, bundled Natural Earth 110m, projection builder); local Mac build made and copied to the project folder (self-test OK; found and fixed P-1, missing datum grids in the bundle); `cargo clean` deferred until the end of the session to keep rebuilds fast; Phase D: D0 tiled DEM, D1 ArcGIS catalog, D2 Census join + keychain, D3 STAC/COG, D4 Overture GeoParquet, D5 OGC API and D6 earthquakes + SSURGO soils done (WMS/WMTS and NASA POWER deliberately not built, DECISIONS D6) (keychain not yet tried on a real OS: M-31); Phase D is complete. Phase E done (calculator, reclassify, NoData, band select, resample, align, mosaic, rasterize as model nodes; every raster step cancellable). Full local run green (engine 40 test files, vitest 377, e2e 48); Mac app rebuilt from this state and copied to the project folder (self-test OK, httpfs bundled). Phase H started 2026-10-29 (`docs/ANALYSIS_PLAN.md`; user approved reusing permissive libraries and chose a bundled WhiteboxTools sidecar for hydrology): H1 interpolation (IDW, TIN, nearest, average via GDAL grid), H2 ordinary kriging and H3 Moran's I + Gi* hot spots and H4 hydrology (six nodes, WhiteboxTools v2.4.0 built by `scripts/build-whitebox.sh` and bundled as a sidecar) and H5 remote sensing (spectral indices, K-means) done: Phase H is complete. Mac app rebuilt 2026-10-29 with the H tools (self-test OK; `whitebox_tools` bundled and found through the app's resources; hydrology tests pass against them; not yet clicked through in the app UI). Next: F and G need your design decisions, then R7 (CI). Hydrology is untested in the packaged app (M-32) and on Windows/Linux (R7). F and G need design decisions from you, then R7 (CI, fix the Phase C Windows/macOS failure). THIRD_PARTY_NOTICES.md needs regenerating from a CI build (see DECISIONS D2). Session plan and feature report: `docs/SESSION_REPORT.md`. **No CI until phases R–H are all complete** (user, 2026-10-28): work locally, push with `[skip ci]`.
- **Done:** the bug-fix session is merged. Q3 (2026-10-28): U-5 fixed, D-1 guard built. L1 (2026-10-28): labels on the map (pushed). L2 and L3 (2026-10-28): labels in layouts, and adjusting them in the layout (drag, edit text, hide, per-layout style) built. L2+L3 pushed together; CI green (run 40734953739); CI runs once for L1–L3 (user's rule).
- **Side session 2026-10-29 (labels, tables, layers; all merged to local main with `[skip ci]`, not pushed by that session):** Export Table (point layer attributes + coordinates in a chosen CRS as CSV/Parquet, sidecar `.crs.txt`; XLSX not built), selecting several layers (Shift/Ctrl/Cmd-click; menu: zoom, show/hide, move/gather, apply symbology or labels, export layers, remove), geoprocessing tools in nested dropdown categories (`ToolSpec.category` = "Group › Sub-group"), the Symbology and Labels node (styles the layer from its node tree, index contours, labels), and the labeling suite per `docs/LABELS_PLAN.md`: fonts installed on the device (`list_fonts`; layouts, PDF/PNG, text elements), placement/rendering options (positions, priority, overlap, wrap, background, shadow, data-defined rotation/size/colour), contour labels (map curved + layouts straight where the line is straight). Cases: TEST_PLAN sections ET, MS, TC, SY, LB. Decisions: DECISIONS ET, MS, TC, SY, LB1–LB3. Open: named layer groups, raster colour-ramp symbology, curved text in layouts, Windows/Linux font lists (R7), packaged-app click-through. Full local run before the last merge: engine all pass, vitest 408, e2e 55.
- **Side session 2026-10-29 (raster symbology + cartography, `docs/CARTOGRAPHY_PLAN.md`; all merged to local main with `[skip ci]`, not pushed):** RS1–RS7 raster symbology (Properties › Symbology for rasters: gray, pseudocolour with 26 ramps incl. colour-blind-safe marks, stretches with a histogram and draggable handles, classified colours, unique values, multiband composites with band presets, relief shading, resampling, transparency, adjustments; raster legends in layouts; style files). CG1–CG10 cartography: shapes and pictures, background/frame/shadow on any element; 8 north-arrow styles + grid north + custom picture; 5 scale-bar styles + second unit + verbal scale; editable legends (rename, hide, extra entries, patch shapes); overview/inset maps with extent indicator; tables, charts (Okabe–Ito), rich text and {author}/{layer_count}/{layer_names}; Page Setup (size, margin, bleed, crop marks, author, world file), guides + snapping, distribute/same size/group, element list; frame shapes (ellipse, rounded, polygon outline) and layer drop shadows; symbol library in layouts (7 point shapes + SVG, 4 line styles, 7 fill patterns); themes (topographic, thematic, minimalist); world file + .prj for PNG exports. Cases: TEST_PLAN RS, CG; decisions DECISIONS RS1, CG8, CG10. **Deferred (reasons in the plan):** GeoPDF and CMYK, symbols/patterns on the map view (needs a sprite pipeline), blur effects, declination diagram (C4), spatially filtered tables and paging, text auto-wrap, user-made theme files, RAT labels for rasters. **Not checked in the packaged app:** raster pane look, file dialogs (picture, style files, SVG symbol), real-screen look of the new layout elements; raster tiles on a CRS-less test raster are not drawn in e2e.
- **On hold:** M4 (rasters in any CRS). Do not start it.
- **You:** A3 (check the Windows and Linux installers on real machines), then tag `v0.1.0` (`git tag v0.1.0 && git push origin v0.1.0`). Next tag: `v0.2.0` at the end of Phase C.

## State
- `main` is on GitHub at https://github.com/AsherSeiling/OpenGeography (public). Never commit `.claude/` (local launch config and worktrees).
- Phases A (except A3 and the tag) and B are done.
- Phase C:
  - C1 and C2.1–C2.4 are done: layouts print a full map.
  - The projected map view M1–M3 is done: OpenLayers draws in the map's CRS, and the tools are exact in any CRS.
  - Error pop-up and per-pane error boxes (Q1), and the test plan with its tests (Q2), merged 2026-10-25.
  - Left: M4 (rasters in any CRS, pole-safe tools, GDAL thread-safety gate), C3.0 layout polish, then C3.1–C3.2.
- CI history: `docs/CI_LOG.md`. CI logs need a GitHub login: read failures from the run annotations.
- The packaged Mac app and DMG are copied into the project folder (`GIS Studio.app`, `GIS-Studio-<version>-macos-arm64.dmg`).

## Build and test status (macOS arm64, 2026-10-25)
| Check | Result |
|---|---|
| `npx tsc` | pass |
| `npx vitest run` | 361/361 (38 files) |
| `cargo test -p engine` | 179 pass, 23 ignored (network, perf, and the open bugs in `docs/BUGS.md`) |
| `cargo build -p gis-studio` | pass, 0 warnings |
| Packaged app `--self-test` (clean env) | pass |
| Browser e2e (`scripts/e2e.sh`, headless Chromium) | 36 pass, 0 fixme, ~1.5 min; automates 9 manual cases fully and 7 in part (`docs/TEST_PLAN.md` §14) |

## Known gaps (layouts) — C3.0 fixes these
- **Layout view:** no rulers, no Arrange group, no "1:N" scale-bar label.
- **Export:** no dpi box (PNG is 300 dpi).
- **Preview:** with a basemap it re-fetches tiles on every edit (4–5 s).
- **Later (C4):** feature labels and GeoPDF.

## Known issues
- **e2e suite:** `e2e/*.spec.ts` (Playwright, devDependency). `scripts/run-test-plan.sh --e2e` runs it; run it alone with `scripts/e2e.sh <out-dir>`. Needs Google Chrome, `/Applications/Chromium.app`, `E2E_BROWSER_PATH`, or `npx playwright install chromium`. The devserver address and vite port come from `GIS_DEVSERVER_ADDR`, `GIS_VITE_PORT`, `VITE_GIS_DEVSERVER` (defaults unchanged).
- **Bugs:** U-5 fixed 2026-10-28 (the dock closed the open layout when Properties was clicked; tracer `?trace=layout`). D-1: guard built (`tnm::plan_elevation`: ellipsoid sizes, refuses areas wider than 6° / beyond 84° / needing cells coarser than 90 m, says when coarsened); the tiled download and the service-limit tests are D0. See `docs/BUGS.md`.
- **Cancel granularity:** cancel lands between DuckDB work units. Big geodesic Buffer and Dissolve now stop in ~0.1 s (`nodes/batching.rs`); other single-call work may still lag. gdaldem steps ignore GDAL's progress callback.
- **Intermittent local failure:** on 2026-10-28 `cargo test -p engine` reported the `layout` test target failed once in a full parallel run (output not captured); it passed on the next two runs. Capture the output if it recurs (it may be the crash below).
- **Bugs found 2026-10-28:** U-6 (MapView's display-ready cache survives New Project, so a re-used layer id can stay blank in a projected CRS) and E2E-3 (M-25 flaked once). See `docs/BUGS.md`.
- **macOS CI crash:** one intermittent SIGSEGV in layout tests (run 40055424934, conda GDAL, parallel tests). It hasn't happened again. M4's P17 stress test is the gate.

## Next tasks (in order)
1. (L1–L3, B6, B7 done; B6+B7 pushed together; L1–L3 CI green for `f685b41` on all platforms, run 40734953739.)
2. B6 split a layer by attribute, B7 category drawing order.
3. C3.0 layout polish → C3.1 templates → C3.2 map series (end of Phase C).
4. Phase R, coordinate-system hardening (`docs/CRS_PLAN.md`; 112 cases in `docs/TEST_PLAN.md` §2A). Recommended: tag `v0.2.0` after it.
5. Phase D: tiled 3DEP download (D0), generic ArcGIS REST catalog (`docs/PUBLIC_DATA_CATALOG.md`), then the Census data join.
M4 is on hold.
