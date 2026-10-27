# Coordinate systems (CRS) hardening — plan

_Written 2026-10-27 (draft by Sonnet from reading the code and docs; **Opus review of §3 and §4 before R1 starts**). User request: a robust plan so coordinate systems work, with full test cases written down, run right after the current phase (Phase C) in `docs/MASTER_PLAN.md` as **Phase R**. The 100 test cases are in `docs/TEST_PLAN.md` §2A (IDs `R-A1`…`R-L4`); this file owns the design, the audit and the sessions._

**Why now.** Every other priority rests on CRS being right: measurements, buffers, joins, map drawing, layouts, downloads and (later) planetary bodies. CRS bugs are silent: a wrong number looks like a right one. Today CRS handling is spread over ~49 `ST_Transform` call sites, string comparisons and a few helpers (`crs.rs`, `nodes/mod.rs::geom_in`, `display.rs`, `raster.rs`, `layout/geo.rs`). 18 CRS cases are covered (`TEST_PLAN.md` §2), mostly one CRS pair per case. Nothing yet proves the whole app is consistent across many CRSs.

## 1. What "CRS works" means (the invariants)

| # | Invariant | Where it is tested |
|---|---|---|
| I1 | **A CRS is never guessed.** No CRS → refuse or warn, never assume. | R-B6, R-E2, R-E4, R-G10 |
| I2 | **Transforms are correct** to 1 mm projected / 1e-8° geographic against an independent reference (PROJ's `cs2cs` output, committed with its PROJ version) and analytic anchors. | R-A |
| I3 | **Identity is by meaning, not spelling.** `EPSG:4326`, WKT, PROJJSON, a URN and an equivalent PROJ string are one CRS; two CRSs that differ in datum or unit never are. | R-B |
| I4 | **x is east, y is north, everywhere** (engine, GDAL, UI), whatever the authority's axis order (EPSG:4326, 3035, 2193 are lat/lon or north/east on paper). | R-C |
| I5 | **Units are honoured**: metre, US survey foot, international foot, degree, non-Greenwich prime meridians; distance parameters say what unit they use. | R-C4–C7, R-F6 |
| I6 | **Datum changes use the best operation PROJ has, and the user is told its accuracy.** A missing grid (a "ballpark" shift, up to ~100 m off for NAD27) is reported, never silent. | R-D |
| I7 | **Measurements are CRS-independent.** Geodesic length, area, buffer and distance on one shape give the same answer from any CRS it is stored in. | R-F2–F4 |
| I8 | **Results keep a stated CRS** (first input's, unless the tool says otherwise) and say when they reprojected. | R-F5, R-F10 |
| I9 | **Anything that can't be projected is counted and reported**, not dropped quietly (poles, antimeridian, outside a projection's domain). | R-F11, R-J |
| I10 | **Save, export and reopen preserve the CRS**, including on another OS. | R-E, R-K |
| I11 | **The screen, the engine and the print agree**: map readout, scale, layout scale bar and graticule and the distortion display (Tissot circles, k, convergence) match the engine's numbers. | R-H, R-I |
| I12 | **A new tool can't skip CRS testing.** A guard test fails when a registered tool is missing from the CRS matrix. | R-F1 |

## 2. Audit — where CRS touches the code, and suspected gaps

Found by reading the code on 2026-10-27; **each is a hypothesis until its test runs** (R1 and R2 confirm or clear them and file real ones in `docs/BUGS.md` as `C-n`). References are to file and symbol, not line.

| Gap | Where | What may go wrong | Test |
|---|---|---|---|
| G1 | `nodes/mod.rs::geom_in` compares CRS strings with `eq_ignore_ascii_case` | An `EPSG:26915` layer and the same CRS as ESRI WKT (shapefile `.prj`) differ as strings, so tools reproject needlessly, flag `reprojected`, and add rounding noise. Other sites may compare the same way. | R-B1–B3, R-E2 |
| G2 | `ST_Transform` under DuckDB Spatial (49 call sites, `always_xy = true`) | PROJ may pick a *ballpark* shift when a datum grid is missing: `projinfo` shows NAD27→NAD83 has a "ballpark, unknown accuracy" fallback beside the 0.15 m grid operation. A packaged app with the wrong `PROJ_DATA` would be off by tens of metres, silently. | R-D1–D6, R-K2 |
| G3 | `nodes/mod.rs::aeqd_for` | Hard-codes `+datum=WGS84` and an `EPSG:4326` pivot. Fine on Earth (≤ ~2 m for NAD83/NAD27 data); wrong off-Earth (planetary phase F1 replaces it). Must be documented and bounded, not left implicit. | R-F3 (error bound by datum) |
| G4 | `lib.rs::identify` / `select_rect` (P18 in `MAP_PROJECTION_PLAN.md`) | Build the query in the map CRS, then transform to Web Mercator `__g`. Web Mercator is undefined beyond ±85.05°, so identify/select at the South Pole in EPSG:3031 is wrong. | R-H4, R-J1 |
| G5 | Z / M values through `ST_Transform` | Behaviour unknown (kept, dropped, or error). Vertical datums are not handled at all. Needs a stated policy. | R-D8, R-J5 |
| G6 | `raster.rs::crs_of` returns `AUTH:CODE` whenever one exists | A GeoTIFF with an EPSG code *plus* a modified definition loses the modification. Rotated/sheared geotransforms and sidecar `.prj` files may be misread. | R-E7, R-G1, R-G9 |
| G7 | Import: `layers.rs::crs_from_meta`, GeoJSON `crs`, GPKG `srs_id` 0/−1/custom, CSV | Flavours differ (ESRI vs OGC WKT; undefined SRS ids; legacy GeoJSON `crs`). A silent fallback to 4326 is the worst case. | R-C2, R-E1–E4 |
| G8 | Axis order: GDAL side uses `TraditionalGisOrder`, DuckDB side `always_xy` | CRSs whose authority order is north/east (EPSG:3035, 2193) or lat/lon (4326) must come out east-first from every importer, exporter and the map. | R-C1 |
| G9 | Export: `GeoJSON: some CRS` (TEST_PLAN 1.16) is weak | RFC 7946 GeoJSON is lon/lat WGS 84 only. A UTM layer exported to GeoJSON must either reproject (and say so) or write the legacy `crs` member. Undefined today. | R-E3 |
| G10 | Distance-unit parameters (Simplify, Densify, grids, Smooth, points along lines…) | In a geographic CRS a "CRS units" tolerance is in degrees. A parameter labelled metres but applied in degrees is a silent 111 km error. | R-F6 |
| G11 | Hillshade / slope / aspect (`gdaldem`) in a geographic CRS | Needs the horizontal-to-vertical scale (`-s`); a missing scale makes slopes wrong by orders of magnitude. | R-G5 |
| G12 | Node cache and attribute history keyed by SQL/params, not by CRS definition | Define Projection on a layer must invalidate downstream cached results. | R-F12 |
| G13 | PROJ/GDAL versions differ between local (Homebrew) and CI (conda) builds | A reference suite with tight tolerances needs the PROJ version in its log and one tolerance table (§3 RD-4). The layout golden SVG SHA already shows this risk. | R-K1 |
| G14 | PROJ contexts and GDAL handles across threads | Tile serving, display builds and tools run concurrently; one macOS CI SIGSEGV is open. Shares the M4 P17 stress rule. | R-K3 |
| G15 | Data outside a CRS's area of use | M1 reports dropped features for the *map*. Tools, exports and layouts need the same rule, and a "data is outside this CRS's recommended area" chip. | R-F7, R-H3, R-J9 |

**Out of scope (written down, so nobody assumes it):** time-dependent / dynamic datums and reference epochs (ITRF, WGS 84 realisations), geoid and vertical transformations beyond passing Z through unchanged (R-D8), and non-Earth bodies (phase F1 adds a `Body`; here we only keep PROJ's refusal of cross-body transforms, R-B9).

## 3. Decisions to review (Opus)

- **RD-1. Reference hierarchy.** A test's expected value comes from, in order: (1) an analytic fact (central meridian easting 500000 m; Web Mercator half-world 20037508.343 m; a pole is the origin of a polar stereographic; 1 ftUS = 1200/3937 m; WGS 84 meridian quadrant 10 001 965.729 m); (2) `cs2cs`/`projinfo` output generated by `scripts/gen_crs_reference.py` and committed as `crates/engine/tests/fixtures/crs/reference.json` together with the PROJ version; (3) GeographicLib (`geographiclib-rs`, already a dependency) for geodesic length, area and direct problems; (4) another GIS (QGIS/ArcGIS) in the manual cases. **Never the engine's own output** (a test that compares the engine to itself finds nothing).
- **RD-2. CRS identity = a canonical key.** Add `crs::key(crs) -> String` (the M1 `crs_key`: SHA-256 prefix of the sorted PROJJSON, axis order ignored) and `crs::same(a, b)`. Every string comparison of CRSs in the engine and UI is replaced by it. Names, remarks and axis order don't matter; datum, projection parameters and units do. Open question for Opus: treat WGS 84 realisations / datum ensembles as equal to EPSG:4326 (PROJ's own `is_equivalent_to` with ensembles ignored), yes by default.
- **RD-3. Transform quality is surfaced.** The `reprojected` flag becomes a record: `{ from, to, operation, accuracyM | "ballpark" | "exact" }` kept on the result and shown in the tool result and the layer's Source section. "Ballpark" raises a warning and, for datum shifts above 1 m of potential error, an error pop-up with the fix ("this install is missing the NADCON grid; see Help"). An app build without its datum grids fails `--self-test` (R-K2).
- **RD-4. Tolerance table** (one place, `tests/common/crs_tol.rs`):

  | Quantity | Tolerance |
  |---|---|
  | Projected coordinates vs reference | 1 mm |
  | Geographic coordinates vs reference | 1e-8° (≈ 1 mm) |
  | Round trip A→B→A | 1 mm / 1e-9° |
  | Geodesic length vs GeographicLib | 1e-5 relative (set to ten times the first measured error, then fixed) |
  | Geodesic area vs GeographicLib | 1e-4 relative |
  | Same measure from two stored CRSs | length 1e-5, area 1e-4 |
  | Scale bar / `1:N` vs engine factors | 0.1 % |
  | Raster value after Warp of an analytic plane | 1e-6 of the plane's range |
  | Pixel alignment (raster vs vector) | 1 px |

  Looser only with a written reason in the test.
- **RD-5. Metamorphic tests over a CRS "zoo".** Instead of hand-writing a case per tool and CRS, one harness stores the same fixture shapes in each zoo CRS, runs each tool, converts the result to a common frame and compares with the answer from the reference CRS (§4). New tools join the matrix automatically (I12).
- **RD-6. Out-of-domain rule.** A transform that PROJ can't compute (non-finite, error, beyond ±5e8 as in M1) is counted in the result (`dropped`, with a sample of fids), never silently turned into an empty geometry, and shown to the user (`progress`/error pop-up rules in `CLAUDE.md`).
- **RD-7. GeoJSON export default.** Reproject to EPSG:4326 (RFC 7946) and tell the user; an "Keep layer CRS (legacy `crs` member)" option writes the old form. Other formats keep the layer CRS.
- **RD-8. Z and M.** Z is carried through unchanged by horizontal transforms and the result says "vertical datum not transformed" when a layer's CRS is compound or has a vertical component; M is dropped with a message. (Opus may choose to refuse instead.)
- **RD-9. Distance parameters.** A tool parameter that is a distance has an explicit unit in `ToolSpec` (`metres`, `feet`, or `crs units`). `crs units` is refused for a geographic input with "use a projected CRS or metres" (G10).

## 4. The test CRS "zoo"

Each CRS earns its place by exercising something different. Fixtures are generated at test time from the code (shapes) and from `reference.json` (numbers).

| # | CRS | Why |
|---|---|---|
| 1 | EPSG:4326 | geographic, authority order lat/lon, the pivot |
| 2 | EPSG:4269 | NAD83 geographic; 4326↔4269 has a 0.2–2 m operation |
| 3 | EPSG:4267 | NAD27, grid-based shift (needs NADCON/NTv2 grids) |
| 4 | EPSG:3857 | Web Mercator; undefined beyond ±85.05°; sphere on WGS 84 axes |
| 5 | EPSG:32615 / EPSG:26915 | UTM 15N on WGS 84 and NAD83 (same projection, different datum) |
| 6 | EPSG:32756 | UTM south (false northing 10 000 000) |
| 7 | EPSG:5070 | CONUS Albers, equal area |
| 8 | EPSG:2277 and 32139 | Texas Central in US survey feet and metres (unit twin) |
| 9 | an international-foot CRS (found in the catalog by unit factor 0.3048) | ft vs ftUS differ by 2 ppm |
| 10 | EPSG:3031 and EPSG:3413 | polar stereographic, pole at the origin |
| 11 | EPSG:3035 | ETRS89 LAEA Europe, axes north/east |
| 12 | EPSG:2193 | NZTM 2000, axes north/east, area of use crosses the antimeridian |
| 13 | EPSG:32660 and EPSG:32601 | UTM 60N / 1N, the antimeridian pair |
| 14 | EPSG:4807 | NTF (Paris): non-Greenwich prime meridian, grads |
| 15 | `+proj=aeqd +lat_0=… +lon_0=… +datum=WGS84` | custom string, no area of use |
| 16 | an ESRI-flavoured CRS (e.g. a `.prj` from `NAD_1983_UTM_Zone_15N`) | WKT flavour / AutoIdentify |
| 17 | a compound or 3D CRS (e.g. EPSG:5498, NAD83 + NAVD88 height; confirm the code with `projinfo` before pinning) | Z handling |
| 18 | `IAU_2015:49900` (Mars) | PROJ refuses Earth↔Mars transforms (guard stays when F1 lands) |

**Verified anchor values** (PROJ 9.8.1, `cs2cs`, 2026-10-27; they seed `reference.json` and the first tests):

| Input (lat, lon) | CRS | Result |
|---|---|---|
| 0, −93 | EPSG:32615 | 500000.000, 0.000 |
| 0, 180 | EPSG:3857 | 20037508.343, 0.000 |
| −71, 0 | EPSG:3031 | 0.000, 2082760.109 |
| −90, 0 | EPSG:3031 | 0, 0 (the pole) |
| 23, −96 | EPSG:5070 | 0.000, 0.000 |
| 35.2, −97.5 (NAD27 → NAD83) | EPSG:4267 → 4269 | 35.200066504, −97.500312182 (≈ 7.4 m N, 28.4 m W) |
| 35.2, −97.5 (WGS 84 → NAD27) | EPSG:4326 → 4267 | 35.199935272, −97.499683431 |
| 35.2, −97.5 (WGS 84 → NAD83) | EPSG:4326 → 4269 | 35.200001669, −97.499995543 (the 2.0 m Oklahoma operation) |

Analytic anchors: 1° of longitude at the WGS 84 equator = 6 378 137·π/180 = 111 319.491 m; Mars sphere (3 396 190 m) 59 274.70 m; Moon (1 737 400 m) 30 323.35 m; WGS 84 ellipsoid surface 510 065 621.7 km².

## 5. Sessions

One session = one bounded task (rules in `MASTER_PLAN.md` §2). Order is fixed; R2 and R3 change engine behaviour, so Opus reviews them (CRS and geodesy are on the risky list).

| # | Task | Effort | Model | Acceptance |
|---|---|---|---|---|
| R0 | This plan; Opus review of §3 and the tolerance table | 0.5 | Opus | Decisions RD-1–RD-9 accepted or changed in `DECISIONS.md` |
| R1 | **Reference fixtures and harness.** `scripts/gen_crs_reference.py` (cs2cs; records the PROJ version), `fixtures/crs/reference.json`, `tests/common/crs_tol.rs`, `tests/crs_reference.rs`. Cases R-A, R-C1/C4–C7, R-D1–D3, R-J1–J4/J6–J8. **Find, don't fix:** failures become `#[ignore]` bug tests and `docs/BUGS.md` rows `C-n`. | 1–1.5 | Sonnet | Every R-A/R-C/R-D/R-J case written; engine suite still green; bugs listed |
| R2 | **Identity and parsing.** `crs::key`/`crs::same`; replace every CRS string comparison; importer flavours (ESRI `.prj`, GeoJSON `crs`, GPKG custom/undefined, CSV); bad-definition hardening. Cases R-B, R-E1–E2, R-E4–E5, R-C2–C3. | 1–1.5 | Sonnet, Opus reviews | R-B all pass; no `eq_ignore_ascii_case` on a CRS remains (grep test); G1, G7 closed |
| R3 | **Transform quality.** Operation + accuracy record (RD-3); ballpark warning; `--self-test` asserts the datum grids; GeoJSON export policy (RD-7); Z/M policy (RD-8). Cases R-D4–D8, R-E3, R-J5, R-K2. | 1–2 | Sonnet, Opus reviews | R-D all pass; a PROJ_DATA without grids is caught, not silent |
| R4 | **Tool matrix.** `tests/crs_matrix.rs` runs every vector tool over the zoo (R-F), with the guard (R-F1), unit rule (RD-9) and out-of-domain rule (RD-6). Fix what it finds in the tools. | 1.5–2 | Sonnet | R-F all pass or are ignored with a `C-n` bug; guard passes |
| R5 | **Rasters and pole-safe queries.** R-G, R-E7–E8, R-J9, P18 (identify/select at the poles via `__disp_*`), `crs_of` improvements, the gdaldem scale fix. Engine side of M4's alignment check (R-G7). | 1–2 | Sonnet, Opus reviews P18 | R-G pass; South Pole identify correct |
| R6 | **UI and layouts.** CRS picker upgrades (custom entry with validation and preview of name, units, datum, area of use), CRS shown on layers and in the status bar, area-of-use chip, **distortion display** (optional Tissot indicatrix layer from the engine's `factors`, plus scale factor k and convergence in the status bar; user request 2026-10-27), layout CRS statement; R-H, R-I, R-E6, e2e additions. | 1.5–2 | Sonnet | R-H/R-I pass in vitest/e2e/engine; manual M-27–M-29 written |
| R7 | **Cross-platform and stress.** CI runs the reference suite on all three platforms and prints PROJ/GDAL versions; thread stress (shares M4 P17); performance baselines; manual run on Windows and Linux; mark the CRS rows in `ROADMAP.md`. | 1 (+ yours for the manual run) | Sonnet + you | R-K pass on macOS, Windows, Linux; M-30 done |

**Total:** 7–10 sessions (R6 grew by about half a session for the distortion display). **Not parallel with each other** (R2–R5 all touch `crs.rs` and `nodes/mod.rs`). R1 may run beside a feature session (new test files only). R6 conflicts with any map-UI task.

**Exit criteria (Phase R done):**
1. All TEST_PLAN §2A cases are `auto` (or `manual` with steps), with no `gap`; any case that exposed a bug is fixed or listed in `BUGS.md` with a decision.
2. `cargo test -p engine` runs the reference suite on all three CI platforms with one tolerance table.
3. The tool guard (R-F1) is in CI.
4. `--self-test` of the packaged app proves the datum grids are present.
5. Roadmap rows for CRS are ✅ with the test ids; `DECISIONS.md` has the RD-1–RD-9 entry; `PROJECT_STATUS.md` updated.

## 6. Effects on other plans

- **`MAP_PROJECTION_PLAN.md`:** P18 (pole-safe identify/select) moves into R5 because it is a CRS correctness bug that exists today. P14–P17 (rasters in the map view, GDAL thread safety) stay with M4, which remains on hold; R7 reuses the P17 stress harness for the engine side only.
- **`PLANETARY_PLAN.md`:** F1 replaces the `EPSG:4326` pivot and `aeqd_for`'s datum with a `Body`. The zoo, tolerance table and matrix are written so a second body can be added as one more row of fixtures.
- **`LAYOUTS_PLAN.md`:** nothing changes; R-I extends P4, P11 and P16 over more CRSs.
- **Releases:** recommend tagging `v0.2.0` after Phase R rather than after C3.2, so the first release with printable maps has verified coordinates. The tag stays your call.
