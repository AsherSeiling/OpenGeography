# GIS Studio — master plan

_Written 2026-10-23. This file sets the **order** of work. What each feature is and its status live in the documents listed in §1; this file links to them rather than repeating them._

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
| [`docs/CI_LOG.md`](CI_LOG.md) | Every CI round: run id, failure, fix | Any CI work |
| [`docs/NEXT_SESSION_PROMPT.md`](NEXT_SESSION_PROMPT.md) | Ready-to-paste prompt for the current Sonnet task | Opening a new Sonnet session |
| [`docs/SONNET_RUNNER_PROMPT.md`](SONNET_RUNNER_PROMPT.md) | Reusable Sonnet prompt that picks the next task from this plan and flags Opus tasks | Any Sonnet session without a task-specific prompt |

**Plans still to be written.** Each is the output of an Opus design session, made before its phase starts:
- `docs/LAYOUTS_PLAN.md` — phase C.
- `docs/ATTRIBUTE_HISTORY_PLAN.md` — phase B.
- `docs/TOPO_MAP_PLAN.md` — phase C2.
- `docs/PYTHON_SDK_PLAN.md` — phase G.

---

## 2. How work is done (applies to every phase)

- **One session = one bounded task.** Each task gets acceptance criteria, the smallest change, tests, then a commit and push. Each session ends by updating `PROJECT_STATUS.md` and the roadmap marks.
- **Opus vs. Sonnet.**
  - **Opus:** design sessions (a plan file with decisions and acceptance criteria), hard debugging, and reviewing risky diffs (geodesy, transactions, licensing).
  - **Sonnet:** implementation from a written plan.
  - Every Sonnet task starts from a pasteable prompt like `docs/NEXT_SESSION_PROMPT.md`.
- **Parallel sessions** only when they touch different files. Two sessions writing to `lib.rs`, `commands.rs` or `store.ts` at the same time will collide.
- **A push cancels the previous CI run** (concurrency setting). While another session is waiting on CI, commit locally and let that session push.
- **"Done" means** all of the following:
  - tests pass;
  - CI is green on all three platforms;
  - the Mac app is rebuilt and copied into the project folder;
  - the feature is checked in the browser (devserver + vite);
  - it cites its data sources;
  - it has a progress bar and Cancel where the work is long.

---

## 3. Phases

**Effort** is counted in focused sessions, not days. At about one to two sessions per working day, each session takes 0.5–1 day. Ranges include the usual CI and fix rounds; the history so far shows each platform problem costs 1–3 extra rounds.

### Phase A — Stabilize and first release ⟵ *now*

**Goal:** a downloadable, permissively licensed v0.1 for all three platforms.

| Task | Effort | Model | Notes |
|---|---|---|---|
| A1. Linux AppImage library clash (conda GDAL vs. system libraries) | 1–3 | Opus decides, Sonnet builds | ✅ Done 2026-10-23. See known issue 2 in `PROJECT_STATUS.md` and `CI_LOG.md`. |
| A2. Refresh stale roadmap rows (platform matrix, portability risks already fixed) | 0.5 | Sonnet | ✅ Done 2026-10-23. |
| A3. Hands-on check of the Windows and Linux installers on real machines (open, import, run a model, save/reopen) | 1 | You + Sonnet | CI's `--self-test` doesn't exercise the UI. Needs a Windows PC/VM and a Linux desktop. |
| A4. GitHub Release v0.1.0: tag, installers attached, release notes, licence notices | 0.5 | Sonnet | Workflow done 2026-10-23 (`release` job in `build.yml`, runs on `v*` tags, publishes a pre-release). **Tag not pushed yet** — wait for A3. |
| A5. (Optional) macOS notarization and Intel build | 1 | Sonnet | Needs your Apple Developer ID ($99/yr). Intel build needs a `macos-13` runner. |

**Linux fallback (decided 2026-10-23).** Keep Linux supported; defer only the GPL-free AppImage.
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
| B1. Attribute history between nodes ("git for tables"): design | 1 | Opus | Writes `ATTRIBUTE_HISTORY_PLAN.md`. Covers snapshot storage (persisting `__node_<hash>` tables beyond the session), diff format, size limits, and the table pane picker. |
| B2. Attribute history: build | 2–3 | Sonnet | Engine snapshots + diff, table pane picker, tests. |
| B3. Missing vector tools: union, difference, symmetrical difference, singlepart→multipart, concave hull, Delaunay | 2 | Sonnet | One file per tool in `nodes/`; cite the QGIS algorithm ids without copying their code. |
| B4. Parameter validation and UI graph tests (splice, undo, tree switching) | 1–2 | Sonnet | Roadmap P2/P3 leftovers. |

**Exit:** roadmap P2 and P3 all ✅.
**Total:** 6–8 sessions.

### Phase C — Map production (priority 2)

The biggest missing piece: the app currently can't make a printable map.

| Task | Effort | Model | Notes |
|---|---|---|---|
| C1. Layouts design | 1 | Opus | Writes `LAYOUTS_PLAN.md`. Must take a **`body`** parameter from day one so planetary maps (F4) are styling, not a rewrite. Decide the rendering path (MapLibre canvas vs. engine-rendered) for vector PDF. |
| C2. Layouts M2: page, map frame in the true CRS, legend, scale bar, north arrow, title, text, graticule; PNG + PDF export | 4–6 | Sonnet | Office-style Layout ribbon tab. |
| C3. Layout templates and map series | 1–2 | Sonnet | |
| C4. Topo Map Builder (M4): design, then build | 1 + 3–4 | Opus, then Sonnet | Needs C2, the National Map downloads and E1. Builds on `nav/geodesy.ts` (UTM/MGRS grid, WMM declination). |

**Exit:** a user can produce a print-scale PDF map with legend and grid; Topo Map Builder makes a USGS-style quad.
**Total:** 10–14 sessions.

### Phase D — Public data toolkit (Earth)

Follows the connector order in [`PUBLIC_DATA_CATALOG.md`](PUBLIC_DATA_CATALOG.md). Connectors D3 and D5 are **shared with the planetary phase**: USGS Astrogeology serves its Moon/Mars data through a STAC API and an OGC API – Features service.

| Task | Effort | Notes |
|---|---|---|
| D1. Generic ArcGIS REST catalog (generalize `tnm_vector.rs`) + catalog entries | 2 | PAD-US, FEMA NFHL, NWI and others become JSON entries. |
| D2. Census join + OS keychain for the user's API key | 2 | A Join Census Data node; ACS and 2020 data. |
| D3. STAC search + COG clip (`/vsicurl/`) | 2 | NAIP, Landsat, Sentinel-2, Copernicus DEM; reused by F1. |
| D4. Cloud GeoParquet (Overture) | 1 | DuckDB reads it natively. |
| D5. OGC API – Features client | 1 | USGS Water Data; reused by F1. |
| D6. Simple JSON APIs (earthquakes, NASA POWER), SSURGO, WMS/WMTS display | 2–3 | |

**Model:** Sonnet throughout; Opus reviews the D1 catalog format once.
**Exit:** the catalog's "Recommended order" list is done.
**Total:** 10–11 sessions.

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
| F0. Design + MapLibre pole spike | 1 (Opus) | — |
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
A  Stabilize + v0.1      ███                      (now)
B  Data manipulation        ████
C  Layouts (C1–C3)              ██████
D  Public data (D1–D3)              █████            ← D3 can run beside C (different files)
E  Raster foundation                     ███
C4 Topo Map Builder                         ███
F  Planetary (F0–F4)                        ███████  ← F0/F1 can start after E; F5 after C2
D  Public data (D4–D6)                             ███
G  Python SDK                                         ███
H  Advanced analysis                                     ██████
```

**Why this order:**
- **A first:** nothing else matters if users can't install it, and the Linux fix is already underway.
- **B before C:** data manipulation is priority 1, and attribute history is already promised.
- **C early:** map production is priority 2 and is the largest gap.
- **D3, D5 and E before F2/F3:** planetary imports and terrain tools reuse the STAC/COG and OGC connectors and the raster tools.
- **Planetary after the shared pieces:** this avoids building those connectors twice.
- **G and H last:** they widen the tool but are not core to the top two priorities.

**Releases:** tag a release at the end of each phase:
- v0.2 after B
- v0.3 after C1–C3
- v0.4 after D1–D3 + E
- v0.5 after F
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

**Never parallel:** two tasks that both add Tauri commands, because both edit `commands.rs`, `main.rs`, the devserver and `api.ts`. Sequence them or merge carefully.

---

## 6. Risks that could change this plan

| Risk | Effect | Mitigation |
|---|---|---|
| Linux AppImage library clash takes many rounds | Delays v0.1 | Linux fallback in phase A (Ubuntu GDAL, `.deb` first, AppImage labelled preview); GPL-free AppImage/Flatpak before 1.0. |
| gdal crate stays on GDAL ≤ 3.12 | Stuck on an older GDAL | Watch gdal-rs releases; the pin is fine for at least a year. |
| MapLibre can't show the poles | Lunar south pole maps | Polar layouts/exports (PLANETARY_PLAN D2). |
| Vector PDF export in layouts | Layout scope grows | Decide the rendering path in C1; PNG/raster PDF first if needed. |
| Usage limits | Fewer sessions per week | Opus only for design and review; Sonnet for code; keep sessions short and the status file current. |
| Remote services change or retire (HIFLD, EJScreen already gone) | Broken importers | Network tests per endpoint; clear "service unavailable" messages; the catalog is data, so fixes are edits, not code. |
