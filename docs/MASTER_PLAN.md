# GIS Studio — master plan

_Written 2026-10-23, updated 2026-10-27 (Phase R, coordinate-system hardening, added after Phase C at the user's request; earlier: Opus plan decisions in `docs/DECISIONS.md`; user requests: labels L1–L3, B6, B7; M4 on hold). This file sets the **order** of work. What each feature is and its status live in the documents listed in §1; this file links to them rather than repeating them._

**Product priorities** (the user's ranking, which drives the order below):
1. Easy data manipulation.
2. Easy map production.
3. Fast, correct geoprocessing.

---

## 1. Plan documents and what each one owns

| Document | Owns | Read when |
|---|---|---|
| [`CLAUDE.md`](../CLAUDE.md) | Permanent rules, architecture in one line each, build/test commands | Every session (loaded automatically) |
| [`PROJECT_STATUS.md`](../PROJECT_STATUS.md) | Current state, known issues, **the exact next action** | Start of every session |
| **`docs/MASTER_PLAN.md`** (this file) | Phases, their order, dependencies, exit criteria, effort | Choosing what to do next; end of each phase |
| [`docs/ROADMAP.md`](ROADMAP.md) | Feature inventory with status marks, platform matrix, licensing, priority list P0–P9 | Checking whether something exists or how well it is tested |
| [`docs/PUBLIC_DATA_CATALOG.md`](PUBLIC_DATA_CATALOG.md) | Earth data: ~70 datasets by theme, connector strategy, connector order | Phase D |
| [`docs/PLANETARY_PLAN.md`](PLANETARY_PLAN.md) | Moon/Mars: body-aware CRS, USGS importers, crater tools, ISIS/ASP bridges | Phase F |
| [`docs/ATTRIBUTE_HISTORY_PLAN.md`](ATTRIBUTE_HISTORY_PLAN.md) | B1 design: snapshot persistence, diff rules, table pane UX, sessions B2.1–B2.3 | Phase B |
| [`docs/LAYOUTS_PLAN.md`](LAYOUTS_PLAN.md) | C1 design: engine SVG→PDF/PNG renderer, document model, print scale/CRS rules, sessions C2.1–C3.2 | Phase C |
| [`docs/MAP_PROJECTION_PLAN.md`](MAP_PROJECTION_PLAN.md) | Map view draws in the map's CRS: OpenLayers replaces MapLibre, engine tiles per CRS grid, sessions M1–M4 | Phase C (C5) |
| [`docs/CRS_PLAN.md`](CRS_PLAN.md) | Phase R: CRS invariants, audit of suspected gaps, decisions RD-1–RD-10, the test CRS "zoo", sessions R1–R7. Its 112 test cases are in `TEST_PLAN.md` §2A | Phase R |
| [`docs/TEST_PLAN_FULL.md`](TEST_PLAN_FULL.md) | Full test plan: edge and failure cases for every feature, error-reporting procedure, coverage gaps (extends `TEST_PLAN.md`) | Writing or running tests; before a release |
| [`docs/CI_LOG.md`](CI_LOG.md) | Every CI round: run id, failure, fix | Any CI work |
| [`docs/DECISIONS.md`](DECISIONS.md) | Decision log: why things are the way they are, newest first | When a task touches that area |
| [`docs/prompts/orchestrator.md`](prompts/orchestrator.md) | Opus orchestrator: delegates to Sonnet sessions, reviews in tiers, merges | Running several tasks from one Opus session |
| [`docs/prompts/sonnet-runner.md`](prompts/sonnet-runner.md) | Reusable Sonnet prompt: picks the next task, flags Opus tasks, ends with a review block | Any Sonnet session |
| [`docs/prompts/next-session.md`](prompts/next-session.md) | Ready-to-paste brief for the current Sonnet task | Starting the next Sonnet task |

**Plans still to be written.** Each is the output of an Opus design session, made before its phase starts:
- `docs/TOPO_MAP_PLAN.md` — phase C2.
- `docs/PYTHON_SDK_PLAN.md` — phase G.

---

## 2. How work is done (applies to every phase)

- **One session = one bounded task.** Each task gets acceptance criteria, the smallest change, tests, then a commit and push. Each session ends by updating `PROJECT_STATUS.md` and the roadmap marks.
- **Opus vs. Sonnet.**
  - **Opus:** design sessions (a plan file with decisions and acceptance criteria), hard debugging, and reviewing risky diffs (geodesy, transactions, licensing).
  - **Sonnet:** implementation from a written plan.
  - Every Sonnet task starts from a brief in `docs/prompts/next-session.md`. One Opus orchestrator session can run the whole loop (`docs/prompts/orchestrator.md`).
- **Parallel sessions** only when they touch different files. Two sessions writing to `lib.rs`, `commands.rs` or `store.ts` at the same time will collide.
- **A push cancels the previous CI run** (concurrency setting). While another session is waiting on CI, commit locally and let that session push.
- **"Done" means** all of the following:
  - tests pass;
  - CI is green on all three platforms;
  - the Mac app is rebuilt and copied into the project folder (once per phase, not per task);
  - the feature is checked in the browser (devserver + vite);
  - it cites its data sources;
  - it has a progress bar and Cancel where the work is long;
  - its failures reach the user through the error pop-up (`ui/src/errors/`), never only the console or status bar;
  - its test cases are in `docs/TEST_PLAN.md` (automated where possible, exact manual steps otherwise).
- **Bugs** live in `docs/BUGS.md`. Each merge batch triages them; one Sonnet bug-fix session fixes the batch, and Opus reviews fixes in risky areas.
- **Status for the user:** the orchestrator updates the "GIS Studio — Project Status" doc (https://claude.ai/artifact/5Pvx6sQMhUVcsJeijMbrAj) at every change; `PROJECT_STATUS.md` stays the sessions' file.

---

## 3. Phases

**Effort** is counted in focused sessions, not days. At about one to two sessions per working day, each session takes 0.5–1 day. Ranges include the usual CI and fix rounds; the history so far shows each platform problem costs 1–3 extra rounds.

### Phase A — Stabilize and first release (waiting on A3, yours)

**Goal:** a downloadable, permissively licensed v0.1 for all three platforms.

| Task | Effort | Model | Notes |
|---|---|---|---|
| A1. Linux AppImage library clash (conda GDAL vs. system libraries) | 1–3 | Opus decides, Sonnet builds | ✅ Done 2026-10-23. See known issue 2 in `PROJECT_STATUS.md` and `CI_LOG.md`. |
| A2. Refresh stale roadmap rows (platform matrix, portability risks already fixed) | 0.5 | Sonnet | ✅ Done 2026-10-23. |
| A3. Hands-on check of the Windows and Linux installers on real machines (open, import, run a model, save/reopen) | 1 | You + Sonnet | CI's `--self-test` doesn't exercise the UI. Needs a Windows PC/VM and a Linux desktop. |
| A4. GitHub Release v0.1.0: tag, installers attached, release notes, licence notices | 0.5 | Sonnet | Workflow done 2026-10-23 (`release` job in `build.yml`, runs on `v*` tags, publishes a pre-release). **Tag not pushed yet** — wait for A3. |
| A5. (Optional) macOS notarization and Intel build | 1 | Sonnet | Needs your Apple Developer ID ($99/yr). Intel build needs a `macos-13` runner. |

**Linux fallback (decided 2026-10-23; not needed — A1 fixed the AppImage, and Linux passes the strict licence gate).** Kept for reference.
1. Revert the Linux CI job to Ubuntu's GDAL (green as of `a4d6b8f`). Keep the Linux license gate warn-only.
2. Label the Linux AppImage "preview, not license-clean" in the README and release notes. macOS and Windows stay GPL-free.
3. Prefer shipping Linux as a `.deb` that depends on the system's GDAL instead of bundling it, so no GPL libraries are redistributed.
4. Keep Linux build + tests in CI on every push, so Linux breakage is caught early rather than piling up.
5. Revisit a GPL-free AppImage or Flatpak before 1.0. It's tracked as a 1.0 release blocker, not a v0.1 one.

**Exit:** CI green on all three platforms, with strict license gates on macOS and Windows; v0.1.0 published. Linux also needs a strict gate, unless the fallback above was used.
**Total:** 3–6 sessions.

### Phase B — Data manipulation completeness (priority 1)

**Goal:** the node editor and attribute workflows feel finished.

| Task | Effort | Model | Notes |
|---|---|---|---|
| B1. Attribute history between nodes ("git for tables"): design | 1 | Opus | ✅ Done 2026-10-23. Writes `ATTRIBUTE_HISTORY_PLAN.md`. Covers snapshot storage (persisting `__node_<hash>` tables beyond the session), diff format, size limits, and the table pane picker. |
| B2. Attribute history: build | 2–3 | Sonnet | ✅ Done 2026-10-23 (B2.1 persisted cache, B2.2 diff + node-rows commands, B2.3 table pane UI). Engine snapshots + diff, table pane picker, tests. |
| B3. Missing vector tools: union, difference, symmetrical difference, singlepart→multipart, concave hull, Delaunay | 2 | Sonnet | **Union, Symmetrical Difference, Collect done 2026-10-23** (difference = Erase); concave hull and Delaunay done too — B3 complete. One file per tool in `nodes/`; cite the QGIS algorithm ids without copying their code. |
| B4. Parameter validation and UI graph tests (splice, undo, tree switching) | 1–2 | Sonnet | **Done 2026-10-23** (engine `validate_params` + layer-tree switching tests; splice/undo tests already existed). Roadmap P2/P3 leftovers. |

**Exit:** roadmap P2 and P3 all ✅. **✅ Phase B complete 2026-10-23:** CI green on all three platforms ([40031502418](https://github.com/AsherSeiling/OpenGeography/actions/runs/40031502418)), packaged Mac app `--self-test` passes. The v0.2 tag is the user's call.
**Total:** 6–8 sessions.

### Phase C — Map production (priority 2) ⟵ *now*

The biggest missing piece: the app currently can't make a printable map.

| Task | Effort | Model | Notes |
|---|---|---|---|
| C1. Layouts design | 1 | Opus | ✅ 2026-10-23 → `LAYOUTS_PLAN.md`. Engine renders SVG → PDF/PNG; `body` parameter from day one. |
| C2. Layouts: page, map frame in the true CRS, legend, scale bar, north arrow, text, sources, graticule/grid, rasters, basemaps; PNG + PDF + SVG export | 4–6 | Sonnet | ✅ 2026-10-23 (C2.1–C2.4). CI green on all three platforms. |
| C5. Projected map view: the map view draws in the map's CRS | 4 (M1–M4) | Opus designed, Sonnet builds | `MAP_PROJECTION_PLAN.md`. ✅ M1, M2 (OpenLayers replaces MapLibre), M3 (exact tools in any CRS; merged 2026-10-25). ⏸ M4 (rasters in any CRS, P17 GDAL thread-safety gate, P18 pole-safe tools) on hold at the user's request. |
| Q1. Error handling and crash fixes | 1 | Sonnet | Error pop-up, per-pane error boxes, 8 layout/CRS crashes fixed. ✅ Merged 2026-10-25. |
| Q2. Test cases for every feature | 1–2 | Sonnet | `docs/TEST_PLAN.md` + missing automated tests (engine 95 → 176, vitest 86 → 310); 8 bugs to `docs/BUGS.md`. ✅ Merged 2026-10-25. |
| Q3. Layout Properties pane + large DEM download (user reports U-5, D-1) | 1–1.5 | Sonnet | **Done 2026-10-28 (U-5 fixed; D-1 guard built).** Moved to D0 with the tiled download: DEM-0/DEM-1 (need the network), DEM-3, DEM-6, DEM-7, DEM-10 (the `plan_elevation` command and dialog hint, confirm above 500 MB), DEM-12. Original brief: (L3 relies on the Properties pane, so it must be dependable first). Step 1: add the tracer (LP-0) and run the diagnostic cases LP-1…LP-16 and DEM-0 (the reproduction ladder), write down the cause in `BUGS.md`. Step 2: fix U-5; for DEM add the guard: read the service limits, `plan_elevation` (size, requests, resolution used), honest message when coarsened, a confirm above 500 MB, Cancel during the wait, and a readable error for each service failure (DEM-1…DEM-3, DEM-6, DEM-10, DEM-12). Test cases: `TEST_PLAN.md` §9A and §10A. Opus reviews if the fix touches the store design. |
| B5. Keep `lastOutput` outside undo history | 0.5 | Sonnet | ✅ Merged 2026-10-25 with the bug-fix batch (B-T1–B-T8, E2E-1/2, U-1–U-4 also fixed). |
| B6. Split by attribute (export by category) | 1 | Sonnet | User request 2026-10-25; spec in `docs/DECISIONS.md`. One layer or one file per unique value of a field (QGIS `native:splitvectorlayer`, reimplemented). After L3. |
| B7. Category precedence in unique-value symbology | 0.5–1 | Sonnet | User request 2026-10-25; spec in `docs/DECISIONS.md`. Drag-to-reorder categories; the order sets draw order on the map and in layouts, and the legend order. Can run beside B6 (different files). |
| L1. Feature labels on the map | 1 | Sonnet | `LAYOUTS_PLAN.md` L1: field, size, colour, halo, scale range; OpenLayers declutter; polygon labels from engine anchor points. User request 2026-10-25. |
| L2. Labels in print layouts | 1 | Sonnet | `LAYOUTS_PLAN.md` L2: same settings, greedy collision on the page, skipped labels reported. After L1. |
| L3. Adjust labels in the layout | 1–2 | Sonnet | `LAYOUTS_PLAN.md` L3: per-layout label style overrides; per-label move, rotate, edit text, hide, reset; pinned labels placed first. User request 2026-10-25. After L2. |
| C3.0 Layout polish | 1 | Sonnet | Rulers, Arrange group, scale bar "1:N" label, dpi box in Export, basemap tile disk cache. Before C3.1. |
| C3. Layout templates (C3.1) and map series (C3.2) | 1–2 | Sonnet | Templates use C3.0's label and arrange. |
| C4. Topo Map Builder: design, then build | 1 + 3–4 | Opus, then Sonnet | Needs C2, the National Map downloads and E1. Builds on `nav/geodesy.ts` (UTM/MGRS grid, WMM declination). |

**Order now:** batch merged 2026-10-25 (Q1, M3, Q2) and the bug fixes merged. Q3 done 2026-10-28. Next: **L1** (labels on the map) → L2 → L3 (labels on the map, in layouts, and adjusting them in the layout; user request), then B6 ∥ B7 (split by attribute; category precedence; user request), then C3.0 → C3.1 → C3.2. **Then Phase R (coordinate-system hardening, user request 2026-10-27) before anything else.** M4 is on hold at the user's request. C4 waits for E1.
**Exit:** a user can produce a print-scale PDF map with legend and grid; Topo Map Builder makes a USGS-style quad.
**Total:** 11–15 sessions (with Q3).

### Phase R — Coordinate-system hardening (user request 2026-10-27) ⟵ *next after Phase C's C3.2*

**Goal:** coordinate systems demonstrably work everywhere: transforms, datum shifts, units, axis order, tools, rasters, map, layouts, import/export, and all three platforms. Plan, audit and decisions: [`CRS_PLAN.md`](CRS_PLAN.md). Test cases (112, written before any code): [`TEST_PLAN.md`](TEST_PLAN.md) §2A.

| Task | Effort | Model | Notes |
|---|---|---|---|
| R0. Opus review of the plan's decisions RD-1–RD-10 and the tolerance table | 0.5 | Opus | Draft written 2026-10-27. Do it before R1. |
| R1. Reference fixtures (`cs2cs` values with PROJ version), tolerance table, transform / axis / unit / datum / edge cases | 1–1.5 | Sonnet | Finds bugs, doesn't fix them (`BUGS.md` rows `C-n`). Test files only, so it may run beside a feature session. |
| R2. CRS identity (`crs::key` / `same`), importer flavours, bad-definition hardening | 1–1.5 | Sonnet, Opus reviews | Removes string comparison of CRSs. |
| R3. Transform quality: operation + accuracy record, ballpark warning, datum grids checked by `--self-test`, GeoJSON export policy, Z/M policy | 1–2 | Sonnet, Opus reviews | |
| R4. Tool matrix over the CRS zoo, with a guard so new tools can't skip it | 1.5–2 | Sonnet | |
| R5. Rasters, raster import flavours, pole-safe identify/select (P18), gdaldem scale in geographic CRSs | 1–2 | Sonnet, Opus reviews P18 | |
| R6. CRS picker with custom entry and preview, CRS shown on layers, area-of-use chip, distortion display (optional Tissot circles, k and convergence in the status bar), layout CRS statement; map and layout cases | 1.5–2 | Sonnet | Not beside another map-UI task. |
| R8. **World view** (user request 2026-10-27): any CRS over the whole Earth (e.g. conic projections), curved graticule, globe outline, standard parallels, bundled world outline, projection builder, optional distortion colouring | 2–3 | Sonnet, Opus designs the clipping method | A feature, not a fix; needs R5 and R6. Can move earlier on request. |
| R7. CI on all three platforms, thread stress, performance baselines, manual Windows/Linux run | 1 + yours | Sonnet + you | Shares M4's P17 stress harness (engine side). |

**Not parallel** with each other (R2–R5 all edit `crs.rs` and `nodes/mod.rs`).
**Exit:** every §2A case is automated (or has manual steps) and passes on macOS, Windows and Linux; the tool guard is in CI; `--self-test` proves the datum grids; roadmap CRS rows ✅. Recommended: tag `v0.2.0` after Phase R.
**Total:** 7–10 sessions.

### Phase D — Public data toolkit (Earth)

Follows the connector order in [`PUBLIC_DATA_CATALOG.md`](PUBLIC_DATA_CATALOG.md). Connectors D3 and D5 are **shared with the planetary phase**: USGS Astrogeology serves its Moon/Mars data through a STAC API and an OGC API – Features service.

| Task | Effort | Notes |
|---|---|---|
| D0. **Tiled large-area 3DEP download** (bug D-1): split the box into service-sized tiles, retry/resume, cancel, mosaic to one raster, one output CRS policy for wide areas, native-resolution choice; or use 3DEP's staged GeoTIFF/COG products through the National Map API for the largest areas (Opus chooses). Cases DEM-4…DEM-9, DEM-11, DEM-13 | 2 | Builds on Q3's `plan_elevation`; first task of Phase D because the Topo Map Builder (C4) and the raster tools (E1) need big DEMs; may run beside D3 if they don't both add Tauri commands |
| D1. Generic ArcGIS REST catalog (generalize `tnm_vector.rs`) + catalog entries | 2 | PAD-US, FEMA NFHL, NWI and others become JSON entries. |
| D2. Census join + OS keychain for the user's API key | 2 | A Join Census Data node; ACS and 2020 data. |
| D3. STAC search + COG clip (`/vsicurl/`) | 2 | NAIP, Landsat, Sentinel-2, Copernicus DEM; reused by F1. |
| D4. Cloud GeoParquet (Overture) | 1 | DuckDB reads it natively. |
| D5. OGC API – Features client | 1 | USGS Water Data; reused by F1. |
| D6. Simple JSON APIs (earthquakes, NASA POWER), SSURGO, WMS/WMTS display | 2–3 | |

**Model:** Sonnet throughout; Opus reviews the D1 catalog format once.
**Exit:** the catalog's "Recommended order" list is done.
**Total:** 12–13 sessions (with D0).

### Phase E — Raster foundation (roadmap P4)

Needed by the Topo Map Builder (C4) and by planetary terrain work (F2).

| Task | Effort | Notes |
|---|---|---|
| E1. Raster calculator, reclassify, NoData tools, band select | 2 | |
| E2. Resample/align, mosaic, rasterize | 2 | Same cancellation/progress rules as existing GDAL steps. |
| E3. Make gdaldem steps cancellable (known issue 1) | 1 | Run them in a child process or chunk by tiles. |

**Total:** 4–5 sessions.

### Phase F — Planetary GIS (roadmap P9)

Details, sources and decisions D1–D5 are in [`PLANETARY_PLAN.md`](PLANETARY_PLAN.md).

| Task | Effort | Depends on |
|---|---|---|
| F0. Design (the pole spike is no longer needed: polar CRSs through the C5 map view) | 1 (Opus) | — |
| F1. Body foundation: one body per project; body-aware geodesics, gdaldem scale and display | 2–3 | F0 |
| F2. Importers: planetary files, USGS STAC data, global mosaics, OGC vector, place names, geologic maps, PDS image search | 3–4 | F1, D3, D5 |
| F3. Crater tools, crater statistics with model ages, morphometry, height reference | 1 (Opus method review) + 2–3 | F1, E1 |
| F4. ISIS / Ames Stereo Pipeline bridges | 1–2 | F1 |
| F5. Planetary cartography | 1–2 | C2 |

**Total:** 11–16 sessions.

### Phase G — Python tool SDK + headless CLI (roadmap P7)

- Opus design, then 3–4 Sonnet sessions.
- User Python nodes exchange data through Arrow/GeoParquet; a CLI runs a saved model without the UI.
- Must keep the "standalone, permissive" rules: no bundled GPL Python packages.

**Total:** 4–5 sessions.

### Phase H — Advanced analysis (roadmap P8)

- Areas: hydrology (flow direction/accumulation, watersheds), interpolation (IDW, TIN, kriging), spatial statistics (hot spots, Moran's I), remote sensing (indices, classification).
- Each area gets an Opus design session, then Sonnet sessions.

**Total:** 8–12 sessions.

---

## 4. Recommended sequence

```
A  Stabilize + v0.1      ███▒                      (code done; A3 + tag are yours)
B  Data manipulation        ████                   ✅  (+ B6 split by attribute, B7 category order: user requests)
C  Map production              ███████▒▒▒          ← now: L1–L3 labels → B6 ∥ B7 → C3.0 → C3.1–C3.2; M4 on hold
R  CRS hardening                         ███████    ← next after C3.2 (user request); R1 can start earlier beside a feature session
D  Public data (D1–D3)                  █████       ← D3 can run beside C3 (different files)
E  Raster foundation                         ███
C4 Topo Map Builder                             ███  (contour labels build on L1–L3)
F  Planetary (F0–F4)                            ███████  ← F0/F1 after E; polar maps use the C5 view
D  Public data (D4–D6)                                 ███
G  Python SDK                                             ███
H  Advanced analysis                                         ██████
```

**Why this order:**
- **A first:** nothing else matters if users can't install it, and the Linux fix is already underway.
- **B before C:** data manipulation is priority 1, and attribute history is already promised.
- **C early:** map production is priority 2 and is the largest gap.
- **D3, D5 and E before F2/F3:** planetary imports and terrain tools reuse the STAC/COG and OGC connectors and the raster tools.
- **Planetary after the shared pieces:** this avoids building those connectors twice.
- **G and H last:** they widen the tool but are not core to the top two priorities.

**Releases** (decided 2026-10-25; the user pushes every tag):
- v0.1.0 once A3 passes.
- v0.2.0 at the end of Phase C (B + C together; no separate tag for B, which finished before v0.1). Recommended 2026-10-27: after Phase R, so the first release with printable maps has verified coordinates.
- v0.3.0 after D1–D3 + E.
- v0.4.0 after F.
- …

**Total effort:** about 56–77 focused sessions. At 5 sessions a week that's roughly 3–4 months, more if usage limits cap sessions per day.

---

## 5. Parallel-safe pairs

These can run in two sessions at once:

| Pair | Why it's safe |
|---|---|
| A1 Linux CI ∥ B1 design | Workflow/scripts vs. a design doc |
| C2 layouts UI ∥ D3 STAC connector | New layout files vs. a new engine module + dialog |
| E1 raster tools ∥ D4 GeoParquet | Separate `nodes/` files vs. an importer |
| F3 crater tools ∥ D6 JSON APIs | Separate modules |
| C3.0/C3.x layouts ∥ D3 STAC connector | Layout files vs. a new engine module + dialog (only one of them may add Tauri commands at a time) |
| Feature work ∥ test-only session | The test session adds new test files only and reports bugs instead of fixing them |

**Never parallel:** two tasks that both add Tauri commands, because both edit `commands.rs`, `main.rs`, the devserver and `api.ts`. Sequence them or merge carefully.

---

## 6. Risks that could change this plan

| Risk | Effect | Mitigation |
|---|---|---|
| Linux AppImage library clash takes many rounds | Delays v0.1 | Linux fallback in phase A (Ubuntu GDAL, `.deb` first, AppImage labelled preview); GPL-free AppImage/Flatpak before 1.0. |
| gdal crate stays on GDAL ≤ 3.12 | Stuck on an older GDAL | Watch gdal-rs releases; the pin is fine for at least a year. |
| ~~MapLibre can't show the poles~~ | Resolved 2026-10-23 | The C5 map view draws any CRS, including polar stereographic. |
| GDAL crashes when used from many threads (one macOS CI SIGSEGV) | Raster tiles in M4 could crash the app | P17 stress test before M4 merges; fallback: one process-wide warp mutex. |
| ~~Vector PDF export in layouts~~ | Resolved in C1/C2 | The engine renders vector SVG → PDF. |
| A CRS audit finds many bugs at once | Phase R stretches | R1 only finds and files them (`C-n`); fixes are batched by area in R2–R5; Opus triages which are release blockers. |
| Usage limits | Fewer sessions per week | Opus only for design and review; Sonnet for code; keep sessions short and the status file current. |
| Remote services change or retire (HIFLD, EJScreen already gone) | Broken importers | Network tests per endpoint; clear "service unavailable" messages; the catalog is data, so fixes are edits, not code. |
