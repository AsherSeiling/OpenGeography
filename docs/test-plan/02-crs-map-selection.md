# 2. CRS, map view, symbology, selection, measure, land navigation

Existing: TP §2 (CRS), §3 (map), §4 (selection, identify, measure). Conventions: [`../TEST_PLAN_FULL.md`](../TEST_PLAN_FULL.md#conventions).

## 2.1 Coordinate reference systems (`validate_crs`, `suggest_crs`, `define_projection`, `transform_point`, `map_scale_factor`, `crs_display_info`)

| ID | Pre | Steps | Expected (user-facing) | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| CRS-01 | — | `validate_crs` with `EPSG:4326`, `4326`, lowercase, WKT, PROJ string, `ESRI:102003`, `IAU:30100` (Moon), empty, whitespace, 5 000-char junk, `EPSG:-1`, `EPSG:1e3` | Valid ones normalised; invalid → "Unknown or invalid coordinate system …" sentence | E | partial: TP 2.7 | P0 |
| CRS-02 | Layer in EPSG:4326 | Define Projection to a projected CRS (wrong on purpose), then Project to 4326 | Allowed (user's choice) but suggestions list warns when coordinates are out of the CRS area of use | E | to automate | P1 |
| CRS-03 | Mid-workflow | Change the map CRS while a tile build is running, a download is running, and a layout is open | Old build cancelled, no stale tiles shown, layout unaffected (layout has its own CRS), no error pop-up | 2E | partial: TP 3.5; to automate | P0 |
| CRS-04 | Layer with tree | Change a layer's CRS with Define Projection after running its tree | Tree flagged stale; cached results not reused (CRS is part of the hash) | E | to automate | P0 |
| CRS-05 | Two layers (4326, 26914) | Overlay in map CRS 3857, 26914, 3031, 4326, 4269 | Both line up (compare a known landmark pixel); NAD27↔NAD83 shift visible only with a note | E+2E | partial: TP 2.13, 3.2 | P1 |
| CRS-06 | — | Data crossing the antimeridian (Fiji, Chukotka, Aleutians) in 4326, 3857, 3832 (PDC Mercator), UTM 60/1, 3832 | Features not stretched across the world; either split or flagged in `dropped`; extent sensible | E+2E | partial: TP 2.2 (picker), 3.4; to automate tiles | P0 |
| CRS-07 | — | Polar data: Antarctica in 3031, Arctic in 3413, lines through the pole in 4326, point exactly at ±90 | Valid tiles; no NaN; Mercator tile reports the cap as dropped/clamped (±85.0511°) | E | partial: TP 2.12, 3.3; to automate Mercator clamp | P0 |
| CRS-08 | — | Custom PROJ strings: `+proj=ortho`, `+proj=laea +lat_0=…`, `+proj=aeqd` with and without `+units` | Usable as map CRS; area-of-use fallback; hemisphere beyond horizon dropped with count | E | partial: TP 2.11, bugs doc M3 note; to automate ortho | P1 |
| CRS-09 | — | Unusual datums/units: ftUS vs international foot, links, chains, grads | Scale bar, measure, buffer distance in correct metres | E | partial: TP 2.10; to automate international foot and non-metre buffers | P1 |
| CRS-10 | — | Vertical / compound CRS (EPSG:5498+…) on import | Horizontal part used with a notice; no crash | E | to automate | P2 |
| CRS-11 | — | Picker UI: 7 195 entries search latency, typing `utm 14`, `nad83 / texas`, accent, empty, clear, pick then Esc | List < 100 ms; selection applied only on confirm | V+2E | partial: crs.test.ts, TP 2.18 manual | P1 |
| CRS-12 | — | Grid files absent (NADCON / NTv2 not installed) for a datum shift | Transform falls back with a visible "approximate (no grid)" note, or refuses; never silent metres of error | E | to automate | P1 |
| CRS-13 | Packaged app | `PROJ_DATA`, `GDAL_DATA` set to garbage | Self-test fails with a readable message; app with bundled data works | PK | partial: TP self-test (clean env) | P1 |

## 2.2 Map view (OpenLayers, `gis://` tile protocol, `build_display`, `display_status`)

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| MAP-01 | — | Layer with 0 features / all NULL geometry / a single point / a 0-extent layer, Zoom to Layer | Map doesn't move to NaN; message "Layer has nothing to draw" | 2E | to automate | P0 |
| MAP-02 | — | Zoom beyond max zoom, below min, tiny window (200×200), huge window (8K) | Clamped; no blank page; tiles requested only for the viewport | 2E | to automate | P1 |
| MAP-03 | — | Pan quickly across 10 layers with 1 M points each | Stale tile requests dropped; memory stable; FPS reported | M | manual M-9 | P1 |
| MAP-04 | — | Reorder layers by drag in Contents; toggle visibility 100× | Draw order matches Contents; no leaks | 2E | to automate | P1 |
| MAP-05 | — | Delete a layer during its tile build | Build cancelled; no error pop-up; later tile requests return empty | E | partial: TP 3.8 | P0 |
| MAP-06 | — | Edit attributes during a build | Build restarts; tile shows edited value | E | partial: TP 3.5 | P1 |
| MAP-07 | — | `gis://` tile request with bad layer id, z/x/y out of range (negative, ≥ 2^z), non-numeric, very long URL | 404-like empty tile; log line; no panic | E | to automate | P0 |
| MAP-08 | — | 20 simultaneous tile requests for one layer being built | Single build shared; all return | E | to automate | P1 |
| MAP-09 | — | `display_status` while idle, building, failed (bad geometry), cancelled | Status text sentence per state; failure shows reason | E | to automate | P1 |
| MAP-10 | — | Layer with Invalid geometries in tile build | Features dropped & counted in notice ("3 invalid features not drawn – run Repair") | E | to automate | P0 |
| MAP-11 | — | HiDPI (devicePixelRatio 2/3), resize window, minimise/restore | Crisp tiles; no re-layout loop | M | manual | P2 |
| MAP-12 | — | Dev-server vs Tauri parity: same tile bytes from `/tiles` and `gis://` | Equal | E | to automate | P2 |
| MAP-13 | — | Map CRS invalid via store injection | Status message, old CRS kept (bug fix) | V | auto: crashes.test.ts "an invalid map CRS is refused" | P1 |
| MAP-14 | — | Map notices: two projections of one layer at once | No duplicate-key error (E2E-1) | 2E | auto: table.spec.ts "M-12 (E2E-1 fixed)" | P2 |

## 2.3 Symbology (Layer Properties → Symbology, `style.ts`, `olStyle.ts`)

Existing: TP 3.10–3.13. Feature B7 (category precedence) is **pending**.

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| SYM-01 | — | Single symbol: width 0, 0.01, 500, negative, NaN; opacity −1, 2; invalid colour text | Clamped/rejected with inline message; map still draws | V | partial: styleEdge.test.ts; to automate UI inputs | P1 |
| SYM-02 | Field with 5 000 unique values | Categorized | Class count capped with "Other" or a message; UI responsive | V+E | partial: TP 5.15; to automate cap | P1 |
| SYM-03 | Field all NULL / single value / mixed types / text numeric | Graduated with 5 classes | No division by zero; breaks unique; message "needs at least 2 different values" | E | partial: TP 5.15 | P0 |
| SYM-04 | Graduated, quantile vs equal interval vs natural | Heavy skew; outliers; ±Infinity | Breaks monotonic, finite, legend labels rounded sensibly | V+E | to automate | P1 |
| SYM-05 | Symbology saved in project | Save, reopen | Identical style; unknown fields from older project reset with a notice | E+V | partial: TP 12.1 (UI state) | P0 |
| SYM-06 | Style references a field then field is deleted/renamed | Apply | Style falls back to single symbol; message; no throw | V | to automate | P0 |
| SYM-07 | Style and layout | Change symbology with a layout open | Layout re-renders with the new style (LayerStyle mirror) | V+E | partial: TP 9.4 | P0 |
| SYM-08 | Labels L1 | Field, size, colour, halo, scale range; null labels; 100 k labels | Declutters; null skipped; polygons use anchor points | — | pending feature (L1) | P1 |
| SYM-09 | Category precedence B7 | Reorder categories | Draw order & legend follow | — | pending feature (B7) | P1 |
| SYM-10 | Raster symbology (stretch, min/max, ramp) if present; Table layer | Open Symbology | Table: disabled with tooltip; raster: min≥max refused | 2E | to automate | P2 |
| SYM-11 | Colour-blind ramps, transparency, dark theme | Legend shows | Contrast adequate | M | manual | P2 |

## 2.4 Selection, definition query, identify

Existing: TP 4.1–4.12, 5.19–5.21. New edge cases:

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| SEL-01 | 2 layers | Select on A, switch active layer to B, delete A | Selection cleared for A; no stale ids; status bar count correct | V | partial: selection.test.ts; to automate delete | P1 |
| SEL-02 | Selection active | Edit attributes (delete rows) | Selection drops removed ids | E+V | to automate | P0 |
| SEL-03 | Selected features | Run model that replaces layer | Selection kept only if fids stable, else cleared with notice | E | to automate | P1 |
| SEL-04 | — | Select by attributes on a 5 M-row layer; cancel | Cancellable or completes < 5 s; errors for bad SQL inline | E (perf) | to automate | P1 |
| SEL-05 | — | Select by location with a mask of another CRS; with no CRS; mask equal to layer | Reprojected; no CRS refused; self-selection selects all | E | partial: TP 4.7, 2.5 | P0 |
| SEL-06 | — | Rectangle across the antimeridian / dragged outside the world / at pole | Selects expected fids (box split) | E+2E | partial: TP 4.5; to automate antimeridian | P1 |
| SEL-07 | — | Definition query then Save → Reopen | Query restored and applied to table/map/export | E | auto: TP 12.1 | P1 |
| SEL-08 | — | Definition query referencing a field later deleted | Layer shows error badge "query refers to a missing field"; Clear query works | E+2E | to automate | P0 |
| SEL-09 | — | Query SQL injection text; 100 KB query; unicode field | Safely rejected or executed as data; no multi-statement | E | partial: TP 5.20; to automate engine side | P0 |
| IDN-01 | — | Identify on overlapping layers, hidden layers, layer with definition query hiding the feature | Hidden/filtered not reported | E+2E | partial: TP 4.8 | P1 |
| IDN-02 | — | Identify popup with 500 attributes, long text, null | Scrollable; layout intact | 2E | to automate | P2 |
| IDN-03 | — | Identify near the antimeridian | Finds feature on either side | E | to automate | P1 |

## 2.5 Measure and Land Nav (client-side geodesy)

Existing: TP 4.13–4.19 (`geodesy.test.ts`, `geodesyEdge.test.ts`, e2e M-10).

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| MSR-01 | — | Measure across antimeridian & over a pole in a polar CRS & UTM edge | Great-circle values (no 360° jump) | V | partial: geodesyEdge | P0 |
| MSR-02 | — | Measure Area self-intersecting polygon, 2 points, collinear | Message "needs 3 points"; self-intersecting warns | V | to automate | P1 |
| MSR-03 | — | Measure vs engine `measure_feature` for the same shape | Within 0.5 % | V+E | to automate cross-check | P1 |
| MSR-04 | — | Units: switch units mid-measure; locale with decimal comma | Values converted; format stable | V | partial: TP 4.15 | P2 |
| MSR-05 | — | Change map CRS while a measure is open | Sketch kept in lon/lat; same ground length | 2E | to automate | P1 |
| NAV-01 | — | Resection with 2 points, collinear, identical points, bearings with 360 wrap | Readable message, no NaN fix | V | partial: TP 4.18 | P1 |
| NAV-02 | — | Declination at WMM validity edge (year 2030+, poles); MGRS 100 km square letters, band X, zone 32V/31X exceptions | Correct or a clear out-of-range message | V | partial: TP 4.16–4.17; to automate exceptions | P1 |
