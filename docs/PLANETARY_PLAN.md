# Planetary GIS plan (Moon and Mars first)

_Plan only, written 2026-10-23; nothing here is implemented yet. Roadmap entry: P9 in `docs/ROADMAP.md`._

**Goal:** a **Planetary** ribbon tab that lets a geoscientist start a Moon or Mars project, pull USGS data for an area, and run the usual tools with correct geometry for that body. The same tab adds crater and terrain tools and produces planetary maps.

**Principle:** use USGS Astrogeology products and services wherever possible. Most users need USGS's already-processed data, so we consume it rather than re-running USGS's pipelines. The heavy USGS tools (ISIS, Ames Stereo Pipeline) are optional bridges, not bundled (§5).

**Status markers:**
- "verified" means it was checked against the live service or local toolchain on 2026-10-23.
- "check" means confirm before building.

---

## 1. What the toolchain already gives us (verified)

| Capability | Evidence |
|---|---|
| **IAU planetary coordinate systems.** PROJ 9.8.1 ships 2,201 `IAU_2015` CRSs. The body codes follow the IAU table; each body has many CRS codes. | `projinfo IAU_2015:49900` = Mars (2015) Sphere/Ocentric (R = 3,396,190 m). `IAU_2015:30100` = Moon (R = 1,737,400 m). Also Phobos 40100, Deimos 40200, Mercury 19900, Venus 29900, Io 50100. |
| **Projected IAU codes per body.** | Per-body suffixes: `…10` equirectangular clon 0, `…15` clon 180, `…20` sinusoidal, `…35` south polar, `…60` transverse Mercator. The ographic code is `…01` (e.g. 49901 Mars ographic), the ocentric ellipsoid `…02`. |
| **PROJ refuses cross-body transforms.** | `projinfo -s IAU_2015:49900 -t EPSG:4326` → "Source and target ellipsoid do not belong to the same celestial body (Mars vs Earth)". This is good: it enforces "never guess a CRS". |
| **GDAL 3.12 reads planetary formats.** Drivers: ISIS3 (rw), ISIS2, PDS (PDS3), PDS4 (raster + vector, rw), VICAR. | `gdalinfo --formats` locally. Check that conda-forge `libgdal-core` includes them on all three CI platforms. |
| **GDAL reads USGS cloud-optimized mosaics remotely.** | `gdalinfo /vsicurl/https://planetarymaps.usgs.gov/mosaic/Mars_MGS_MOLA_DEM_mosaic_global_463m.tif` returns 46080×23040 on a Mars sphere. |

### What assumes Earth today (must change; found by grep)
- **`EPSG:4326` is the pivot for geodesic and extent work** in `measure.rs`, `table.rs`, `nodes/mod.rs` (`aeqd_for`, which also hard-codes `+datum=WGS84`), `layers.rs` (tiles and extents), `lib.rs`, `raster.rs:173/329/430` and `nodes/raster_tools.rs`.
- **DuckDB's `ST_Area_Spheroid`, `ST_Length_Spheroid` and `ST_Perimeter_Spheroid` are WGS84-only** (used in `measure.rs` and `table.rs`).
- **`raster.rs:401/417` passes `gdaldem -s 111120`**, which is Earth's metres per degree. On a geographic Mars DEM, slope would be wrong by a factor of 1.88; on the Moon, by 3.67.
- **The map is MapLibre GL in Web Mercator** (`crs.rs` `MERC_R = 6378137`), and the basemaps are Earth basemaps.
- **`crs::suggest` only suggests Earth CRSs.**

---

## 2. Design decisions (settle in an Opus session before coding)

**D1 — one body per project.** A project gets a `body` (default `earth`). Every layer's CRS must belong to that body. An import whose CRS is on another body is refused with a clear message; we never reproject across bodies. PROJ forbids such transforms anyway, and mixing bodies in one map is meaningless. *Recommended.*

**D2 — how to draw other bodies in MapLibre.** Treat the body's planetocentric longitude and latitude as the map's lon/lat, and tile them with the same spherical-Mercator math. The display is a picture only; all measurement, the scale bar and the coordinate readout use the body's radius.
- This is the standard approach of the OpenPlanetaryMap community (<https://github.com/OpenPlanetary/opm>).
- **Risk: poles.** Mercator can't show above ~85°, and the lunar south pole is the main Artemis region.
- **Spike first:** test MapLibre's globe projection near the poles.
- **Fallback:** analysis in polar CRSs (`…35` south polar) works regardless of the display, and layouts/exports render in polar stereographic. A full polar map canvas would be a separate, larger project.

**D3 — geodesic maths for any body.** Swap the hard-coded WGS84 parts for the body's figure:
- `aeqd_for` takes `+R=` / `+a +b`.
- Lengths and areas use either a per-feature equal-area projection (`laea` with the body radius) or GeographicLib for an arbitrary ellipsoid. GeographicLib: Karney 2013, <https://doi.org/10.1007/s00190-012-0578-z>. Rust port: `geographiclib-rs`, MIT (verified on crates.io).
- Earth keeps using DuckDB's spheroid functions.

**D4 — USGS software is bridged, not bundled.**
- **ISIS** is public domain/CC0 (<https://github.com/DOI-USGS/ISIS3/blob/dev/LICENSE.md>) but very large. It installs through conda on Linux and macOS (check current Windows/WSL support).
- **Ames Stereo Pipeline** is NASA, Apache-2.0 (<https://github.com/NeoGeographyToolkit/StereoPipeline>).
- **Bridge design:** GIS Studio detects an existing install and runs its programs as nodes. Most users never need it, because USGS's analysis-ready data (ARD) is already processed with these tools.

**D5 — longitude and latitude conventions are explicit.** Every planetary layer records:
- planetocentric vs planetographic latitude;
- east- vs west-positive longitude;
- 0–360 vs −180–180 range.

Imports read these from labels/CRS. Conversions are an explicit tool, never silent. Mars planetographic uses west-positive longitude.

---

## 3. Phase 0 — body foundation (prerequisite for everything else)

| # | Work | Done when |
|---|---|---|
| 0.1 | `crates/engine/src/body.rs`: `Body { id, name, naif_id, a, b, geographic_crs, projected_presets }`. Generated from PROJ's `IAU_2015` table by a script (like `build-extra-crs.sh`), not typed by hand. Moon, Mars, Phobos, Deimos, Mercury, Venus and the Galilean moons are listed; Earth keeps EPSG. | unit test: Mars `a` = 3,396,190 m, Moon = 1,737,400 m |
| 0.2 | Project `body` field (saved in `project.json`). New-project choice: Earth / Moon / Mars / other. Body shown in the status bar. | old projects open as Earth |
| 0.3 | Replace every `EPSG:4326` pivot in §1 with `body.geographic_crs()`. Make `aeqd_for`, `measure.rs` and `table.rs` body-aware (D3). | Mars: 1° of longitude at the equator = 59.27 km (2π·3396.19/360); Moon = 30.32 km; Earth unchanged |
| 0.4 | `gdaldem` scale from the body (`2πR/360` m per degree) instead of `111120`. | slope of a synthetic 10° Mars ramp = 10° ± 0.1 |
| 0.5 | Map display per D2: tiles from body lon/lat; scale bar, measure tool and coordinate readout use the body; Earth basemaps hidden off-Earth; MapLibre globe toggle. | browser check: Mars project shows the MOLA basemap; the measure tool reads 59.27 km across 1° at the equator |
| 0.6 | Cross-body guard: an import or Project tool to another body fails with a plain message. `crs::suggest` offers the IAU codes for the project body. | test with a Moon file in a Mars project |
| 0.7 | Spike for D2 poles (MapLibre globe at the lunar south pole). | written result in this file |

---

## 4. Phase 1 — planetary data import ("Get Data" group)

Every importer:
- clips to an area of interest (AOI) and shows the size before downloading;
- reports through `progress::Task` (progress bar + Cancel);
- records a citation in `sources.rs`.

Remote COGs are read with GDAL `/vsicurl/` range requests and clipped (`-projwin`), so a 2 GB global mosaic never downloads in full.

| # | Importer | Service (verified 2026-10-23) | Notes |
|---|---|---|---|
| 1.1 | **Open planetary files** (Catalog + Add Data) | GDAL ISIS3 `.cub`/`.lbl`, PDS3 `.img`/`.lbl`, PDS4 `.xml`, VICAR | Catalog treats label+data pairs as one item (like shapefile sidecars). Body and conventions are read from the label. Test fixtures are made with `gdal_translate -of ISIS3/PDS4`. |
| 1.2 | **USGS Analysis Ready Data** (STAC browser) | STAC API 1.1 at <https://stac.astrogeology.usgs.gov/api/> (<https://stac.astrogeology.usgs.gov/docs/>) | Collections include:<br>• Mars: CTX controlled DTMs (44,628 items, each with DEM, geoid-adjusted DEM, orthoimage, hillshade and error COGs), HiRISE DTMs and observations, THEMIS IR controlled mosaics<br>• Moon: Kaguya TC DTMs v2 (equatorial/north/south polar), LOLA shots<br>• Galileo/Voyager Europa<br>All CC0-1.0. Items carry `proj:wkt2`/`proj:projjson`, so the CRS is explicit. The dialog shows thumbnails, and you pick which files to download. |
| 1.3 | **Global mosaics** (curated list) | USGS COGs at `planetarymaps.usgs.gov/mosaic/`, e.g. `Mars_MGS_MOLA_DEM_mosaic_global_463m.tif`, `Lunar_LRO_LROC-WAC_Mosaic_global_100m_June2013.tif` | Curate about 10 per body (DEMs, image mosaics, THEMIS day/night, SLDEM2015 — check each URL). A network test checks each URL (ignored by default, like TIGER). Also usable as the body's basemap. |
| 1.4 | **USGS vector services** (generic OGC API – Features client) | <https://astrogeology.usgs.gov/pygeoapi/collections> | Collections:<br>• `mars/sim3292_global_geologic_map/units`, `…/contacts`, `…/structure` — Tanaka et al. 2014, SIM 3292, <https://pubs.usgs.gov/sim/3292/><br>• `mars/robbinsv1` — Robbins & Hynek 2012, <https://doi.org/10.1029/2011JE003966><br>• `mars/hirise-observation-footprints-equatorial`<br>Storage CRS is IAU 49900; query by bbox with paging. The same client serves Earth OGC APIs later (public data toolkit). |
| 1.5 | **Place names** | IAU/USGS Gazetteer of Planetary Nomenclature, per-body shapefiles: <https://planetarynames.wr.usgs.gov/GIS_Downloads> (e.g. `MOON_nomenclature_center_pts.zip`) | Label layer plus "Go to place" search (e.g. "Gale", "Shackleton"). |
| 1.6 | **Geologic maps** | Mars: 1.4. Moon: Unified Geologic Map of the Moon (Fortezzo, Spudis & Harrel 2020, LPSC 51 #2760), <https://astrogeology.usgs.gov/search/map/moon_geologic_map> | Ships with unit colours from the map's attributes. |
| 1.7 | **PDS image search** | PDS Geosciences Node Orbital Data Explorer REST (<https://oderest.rsl.wustl.edu/live2/>) | Footprints for HiRISE, CTX, LROC NAC and others over the AOI become a layer with product links; download the selected ones. Prefer map-projected RDR products; raw EDRs need ISIS (§5). |
| 1.8 | **Trek basemaps** (later) | NASA Trek WMTS, e.g. `trek.nasa.gov/tiles/Moon/EQ/LRO_WAC_Mosaic_Global_303ppd_v02/1.0.0/WMTSCapabilities.xml` | Equirectangular tile matrix, so it can't feed MapLibre directly. Use the 1.3 COGs through our `gis://` tile protocol first. |

Citation templates for each source: USGS ARD product, Robbins & Hynek 2012, Tanaka et al. 2014, Fortezzo et al. 2020, Gazetteer, and the PDS product ID/DOI.

---

## 5. Phase 2 — planetary analysis ("Analyze" group + Shift+A category "Planetary")

Existing tools (buffer, clip, area, slope, contours…) become correct on any body once Phase 0 lands.

| # | Tool | Method and citation |
|---|---|---|
| 2.1 | **Digitize crater** (3 rim points → geodesic circle; also 2-point diameter) | Approach of CraterTools (Kneissl, van Gasselt & Neukum 2011, <https://doi.org/10.1016/j.pss.2010.03.015>): a distortion-free circle on the body regardless of map projection. Reimplemented, no code reuse. |
| 2.2 | **Crater counting area + size–frequency distribution** (cumulative, differential, R-plot) | Crater Analysis Techniques Working Group 1979 (<https://doi.org/10.1016/0019-1035(79)90009-5>). Chart in the Results pane. |
| 2.3 | **Model ages** (Moon, Mars) | Neukum production function and chronology (Neukum, Ivanov & Hartmann 2001; Ivanov 2001; Hartmann & Neukum 2001, Space Sci. Rev. 96); Poisson age fitting (Michael et al. 2016, <https://doi.org/10.1016/j.icarus.2016.05.019>). Implemented from the published equations. Export a CraterStats-compatible `.scc` file for interoperability; check craterstats' license before reading its code. |
| 2.4 | **Crater morphometry from a DEM** | Depth, rim height, depth/diameter ratio from radial profiles. |
| 2.5 | **Height reference conversion** | Mars sphere ↔ areoid using the USGS ARD `geoid_adjusted_dem` approach and the MOLA areoid (check the source raster); Moon heights relative to the 1,737.4 km sphere. |
| 2.6 | **Longitude/latitude convention converter** (D5) | PROJ ocentric ↔ ographic; 0–360 ↔ ±180; east ↔ west. |
| 2.7 | **Image geometry from labels** | Read incidence, emission and phase angles and sub-solar azimuth from ISIS/PDS labels into attributes; hillshade from the image's actual sun azimuth/elevation. |
| 2.8 | **Coverage statistics** | How many footprints (HiRISE, CTX) overlap each part of the AOI; uses existing overlay tools. |
| 2.9 | **Site suitability** (landing or traverse) | Weighted overlay of slope, roughness and illumination. Generic tool; planetary presets. |

---

## 6. Phase 3 — USGS software bridges ("External" group, shown only when detected)

| # | Bridge | Scope |
|---|---|---|
| 3.1 | **ISIS** (USGS, public domain) | Detect `ISISROOT` or a conda env. Nodes for `spiceinit`, `cam2map`, `automos`, `photomet` and `isis2std`. Process-based, with progress parsed from ISIS output and Cancel = terminate. On Windows, show that ISIS needs WSL (check). |
| 3.2 | **Ames Stereo Pipeline** (NASA, Apache-2.0) | `parallel_stereo` → `point2dem` as a "Stereo DEM" node. |
| 3.3 | **USGS CSM / ALE** (camera models) | Later: footprints and orthorectification without full ISIS. Check licenses and Windows builds first (<https://github.com/DOI-USGS/usgscsm>). |

---

## 7. Phase 4 — planetary cartography (after Layouts M2)

- Body-aware scale bar and graticule; 0–360 labels; north arrow for polar maps.
- Geologic symbols per FGDC-STD-013-2006, the Digital Cartographic Standard for Geologic Map Symbolization, also used for planetary geologic maps (public domain; reimplement the symbols).
- USGS planetary map template: title block, body, CRS statement, data credits from `sources.rs`.

**Layouts M2 should take a `body` parameter from the start**, so this phase is styling, not rework.

---

## 8. Ribbon layout

**Planetary** tab:
- **Body:** body picker · Lon/Lat convention · Globe.
- **Get Data:** USGS Analysis Ready Data · Global Mosaics · Geologic Maps · Craters · Place Names · PDS Image Search · Open Planetary File.
- **Analyze:** Digitize Crater · Crater Count & Model Age · Morphometry · Height Reference · Convention Converter · Suitability.
- **External:** ISIS · Stereo Pipeline (only when installed).

All tools are also in Shift+A → Planetary.

---

## 9. Order, size and who does it

| Step | Size | Model |
|---|---|---|
| D1–D5 + Phase 0 design and pole spike | 1 session | Opus |
| Phase 0 implementation | 2–3 sessions | Sonnet (Opus reviews the geodesic diff) |
| 1.1–1.3 (files, STAC, mosaics) | 2 sessions | Sonnet |
| 1.4–1.7 | 2 sessions | Sonnet |
| Phase 2 crater statistics (2.2–2.3) | 1 Opus session for the method review, then Sonnet | both |
| Phase 3 bridges | 1–2 sessions | Sonnet |
| Phase 4 | after Layouts M2 | — |

**Tests at each step:**
- engine unit tests with body distances and areas;
- fixture ISIS/PDS4 files;
- ignored network tests for each USGS endpoint;
- a GDAL-driver check added to `--self-test` on all three platforms;
- a browser check of the tab.
