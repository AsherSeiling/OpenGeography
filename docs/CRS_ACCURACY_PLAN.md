# CRS accuracy and honesty plan (2026-10-29)

Why: a wrong coordinate-system result that looks right can send real decisions wrong (flood zones, parcels, hazard maps used by government staff). The software must either be accurate or **say how accurate it is**. Phase R (`docs/CRS_PLAN.md`) made the common US cases correct and tested; this plan closes what R left silent. Nothing here is built yet.

## 1. Gaps found (what can still go wrong without the user knowing)

| # | Gap | Size of the error | Evidence |
|---|---|---|---|
| G1 | Datum shifts outside the US use whatever PROJ has on the machine. The installer bundles only US NADCON and state HPGN grids (58 files). Canada (NTv2), Mexico, Europe, Australia, Japan and others fall back to a "ballpark" shift or a 3-parameter Helmert | usually 1–5 m; old datums (Tokyo, Indian 1975, ED50 without grids) tens of metres | CI run 40898504473 showed even the US needed 51 more grids than we shipped (fixed in c0dd0f5); the reference test at 60°N, -93° (Canada) differs from `cs2cs` by about 3 m |
| G2 | The result never says which operation was used or how accurate it is | user cannot tell a 0.01 m grid shift from a 5 m ballpark | `crs_info` reports CRS facts, not the transformation |
| G3 | Start-up/`--self-test` datum check covers only two US points | a missing grid elsewhere is not reported | `crs.rs` datum check |
| G4 | Vertical datums are not tracked: 3DEP is NAVD88, Copernicus DEM is EGM2008, GPS heights are ellipsoidal; Raster Calculator, Align, hydrology and flood-style comparisons mix them freely | 0.3–2 m in the US, up to 100 m between ellipsoidal and geoid heights | no vertical CRS on rasters |
| G5 | Planar tools in a distorting CRS: buffers, nearest-neighbour statistics, kriging and hot spots use map units; Web Mercator or a UTM zone far from its centre inflate lengths and areas | up to 1.0004 in a UTM zone edge, 2× area at 60° in Web Mercator | status bar shows k/γ but tools do not warn |
| G6 | US survey foot vs international foot (2 ppm). State-plane data in feet carries coordinates in the millions, so mixing them shifts points by metres | about 1.2 m at 600 000 m, up to 4 ft at 2 000 000 ft | units come from the CRS text; no warning on the legacy foot |
| G7 | Epoch/realisation differences (WGS 84 realisations, NAD83(2011), ITRF) are ignored | 1–2 m horizontal, cm/year drift | not modelled |
| G8 | Exports do not carry the transformation/accuracy; CSV coordinates have no stated CRS unless the user types one | silent reuse of a wrong CRS later | Export Table writes the CRS; GeoJSON policy is WGS 84 |
| G9 | The reference suite is US-centred, and until run 40898504473 it never ran in CI (the guard test stopped `cargo test` first) | regressions elsewhere would go unseen | `crs_reference.rs` |

## 2. Principles

1. **Never silently degrade.** If the best transformation needs data we do not have, the user is told before the result is used.
2. **Say what was done.** Every transformed layer/export records the operation, the grids used and the stated accuracy.
3. **Ship what matters, fetch the rest on request.** Small high-value grids ship; large ones are a user-triggered download with a checksum.
4. **Tools warn where the geometry lies.** Planar tools state the distortion they run with, and refuse the cases that are plainly wrong.
5. **Test against PROJ's own answer** wherever a grid exists, on every platform in CI.

## 3. Work plan

Each step: one branch `claude/<id>`, acceptance criteria, focused tests, merge `--no-ff` with `[skip ci]`, rows in `docs/TEST_PLAN.md`, `docs/OPEN_SOURCE.md` for every data source. Sizes in sessions of the usual length.

### Tier 0: be honest about what happens (1–2 sessions, highest value)

| ID | What | Done when |
|---|---|---|
| A1 | `transform_report(from, to, bbox)` in the engine: lists PROJ's candidate operations for the area, the one that will be used, its stated accuracy (m), the grids it needs, and which are missing. Uses GDAL's coordinate-transformation API / PROJ operation list; spike first to confirm the accuracy and grid-availability fields are reachable (otherwise parse `projinfo`-equivalent output) | unit tests over a table of regions (US, Canada, Mexico, Europe, Australia, Japan) give the expected operation and accuracy; missing-grid cases flagged |
| A2 | Show it: Coordinate System tab and the Project/Reproject dialogs print "Shifted with NAD83(HARN) → NAD83, grid us_noaa_okhpgn, accuracy 0.05 m" or "No grid available: ballpark shift, error up to about 3 m"; the layer's source panel records the same | e2e: reprojecting an Oklahoma layer shows the grid line; a Canadian layer with grids removed shows the ballpark warning |
| A3 | Missing-grid policy setting, **default Warn**: a clear dialog before the step runs (continue / get the grid / cancel), logged in Sources; **Strict** option refuses; applies to Project, Align, Warp, Export, display reprojection | unit + e2e for both settings; model runs headless use the setting without a dialog and record the warning in the node message |
| A4 | Exports carry the CRS, datum, units and transformation note: CSV header comment/sidecar JSON, GeoPackage metadata, `.prj` for shapefiles, PDF/PNG layout footers when a transformation was approximate | each exporter has a test reading the note back |
| A5 | Start-up and `--self-test` datum checks cover every bundled region and report the regions whose grids are missing, in plain words | self-test prints a coverage table |

### Tier 1: cover more of the world (2–3 sessions)

| ID | What | Done when |
|---|---|---|
| B1 | Ship small, high-value grids: Canada NTv2 (NRCan), Mexico, GDA94→GDA2020, ETRS89-related national grids where small (each licence checked: most are CC-BY or open-government licences needing attribution). Budget to agree with the user (suggest ≤ 50 MB) | reference points in each region match `cs2cs` with full proj-data to 5 cm |
| B2 | Datum Grids manager (Settings pane): lists regions, shows installed/missing grids and sizes, and downloads chosen grids from the PROJ-data CDN on the user's click, verified by SHA-256, into the per-user data folder; PROJ search path extended so DuckDB's PROJ and GDAL's PROJ both find them. This is an outward-facing download the user triggers | works offline after install; checksum failure is refused with a sentence |
| B3 | Honour the grids in headless/model runs and the Python CLI (G phase) the same way | node run reports the same operation text |

### Tier 2: vertical datums and tools (2–3 sessions)

| ID | What | Done when |
|---|---|---|
| C1 | Record a vertical datum on elevation rasters (NAVD88, EGM2008, ellipsoidal, unknown) from the download source or the user; show it in the layer info | 3DEP and Copernicus downloads fill it automatically |
| C2 | Warn (or convert with an installed geoid grid via GDAL/PROJ) when Raster Calculator, Align, Mosaic or hydrology mix vertical datums or an unknown one | e2e: subtracting NAVD88 from EGM2008 DEMs warns |
| C3 | Warnings for planar tools: when a tool measures in map units and the layer's extent has scale factor outside 0.9996–1.0004 or the CRS is Web Mercator, say so and suggest a CRS (UTM zone / state plane / equal-area for the extent, using `crs_info` area of use); refuse area-based statistics in non-equal-area CRSs unless the user confirms | unit tests with known factors; the picker offers "Suggested for this extent" |
| C4 | Foot units: show "US survey foot" vs "international foot" in the CRS info and warn when a layer in ftUS is combined with one in ft | test with NAD83 / Texas Central (ftUS) vs metre data |
| C5 | Epoch note: state the realisation/epoch assumption for WGS 84 ↔ NAD83(2011) in the report text (not modelled) | text appears in the transform report |

### Tier 3: keep it honest (1 session, then continuous)

| ID | What | Done when |
|---|---|---|
| D1 | Extend `crs_reference.rs` and `scripts/gen_crs_reference.py` to the new regions and to vertical shifts, regenerated from `cs2cs` with full proj-data; CI keeps `proj-data` installed (done 2026-10-29) | all regions pass on Linux, macOS and Windows CI |
| D2 | Matrix test (`crs_matrix.rs`) adds a non-US datum pair per tool recipe | guard still enumerates every tool |
| D3 | A "Limitations and responsible use" page in the app Help and in `docs/`: coverage table by region (accuracy, grids shipped, grids downloadable), what the tool is not suitable for (legal survey, emergency decisions without verification), how to read the transform report | page linked from the Coordinate System tab and About |
| D4 | Release checklist item: before each release run the datum coverage table and fix any "missing" | listed in `docs/MASTER_PLAN.md` release steps |

## 4. Decisions needed from the user

1. Default when a good grid is missing: **warn and continue** (recommended) or block until the user chooses?
2. Bundle size budget for grids in the installer (suggest 50 MB), and which regions come first (suggest Canada, Mexico, Australia, then Europe).
3. May the app download grids from the PROJ-data CDN when the user clicks (outward-facing, user-triggered, checksum-verified)?
4. Wording of the responsible-use statement (draft in D3, for your review).

## 5. Order

A1 → A2 → A3 → A4 → A5 (Tier 0, in that order); B1 and C3/C4 can run beside them (different files); B2 after A3; C1/C2 after A2; D1 continuously; D3 once A2 exists. Tier 0 alone removes the "silent" part of the ethical concern; Tiers 1–2 shrink the errors themselves.
