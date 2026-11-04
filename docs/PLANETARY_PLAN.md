# Planetary GIS plan

_Rewritten 2026-11-04, replacing the 2026-10-23 plan. Plan only; nothing here is built. This is Phase F in `docs/MASTER_PLAN.md`. The user decided on 2026-11-04 that it is the last feature built before the first release and fine tuning._

**Goal:** make a map of another world as easily as a map of Earth. The user chooses **Insert › New Map › Extraterrestrial Map**, picks a body, and the app works in that body's mode for that map: its coordinate systems, basemaps, data, measurements and layouts. USGS and NASA data is built in, and native planetary files open directly.

**Rules this plan keeps** (`CLAUDE.md`): explicit CRS, never guessed; geodesic by default; report dropped features; never overwrite source data; every import records a citation; never require QGIS; one `ToolSpec` per tool; every new Tauri command is added to `crates/devserver`.

---

## 1. Decisions

### Made by the user (2026-11-04)
| Decision | Effect |
|---|---|
| Planetary is a separate mode, entered when a map is created | The mode belongs to **the map**, not the project (replaces the old plan's "one body per project") |
| "Extraterrestrial" is a map option at creation | §3 |
| Native data import integration | §5 and §6 |
| This is the last feature before the first release | **Phase G (Python SDK + CLI) moves to after the first release** |
| Bundle Craterstats, with crater model ages in the first release | About 90–100 MB more installer; §13 |
| Integrate open tools directly when they and all their libraries can be packaged inside the app | §13 |

### Recommended; the user can change these (§12 lists what's still open)
| Decision | Recommendation | Why |
|---|---|---|
| Bodies | Moon and Mars get everything. Mercury, Venus and Europa get a basemap and curated data. Every other body in PROJ's IAU table works with your own files. | That matches where USGS data exists |
| Map drawing | 2D only, in the body's own CRS | The map view already draws in any CRS; a 3D globe is a separate project |
| ISIS and Ames Stereo Pipeline bridges | After the first release | They're large optional installs, and most users need USGS's processed products instead |
| Earth and Mars in one project | Allowed, each in its own map. A layer can only be on a map of its own body. | Multiple maps already exist, and PROJ refuses cross-body transforms anyway |

---

## 2. What was tested on 2026-11-04

Everything here was run against the current `ui-redesign` engine, PROJ 9.8.1 and GDAL 3.12.3, or the live service.

### What already works
| Check | Result |
|---|---|
| Bundled `proj.db` | 127 IAU_2015 geographic and 2,074 projected CRSs; 176 celestial bodies |
| Transform within one body, DuckDB Spatial | Mars geographic to south polar matches the hand calculation (0, −80° gives y = 594,256.25 m) |
| Cross-body transform | Refused by both PROJ and DuckDB, which is what we want |
| Importing a Mars GeoTIFF | Works, and the layer reports `IAU_2015:49900` |
| USGS mosaic labels | The mosaics have no IAU code (`Mars_localRadius`), but PROJ still identifies the body: a transform to Mars succeeds and to Earth is refused. Moon labels still need the same test. |
| Layout body support | `layout/geo.rs` already has a `Body` type, though it only knows Earth |

### What fails today (each becomes a test in §10)
| Check | Result | Cause |
|---|---|---|
| Import a Mars GeoPackage | **Refused:** "Unknown or invalid coordinate system" | `validate_crs` tests a transform from `EPSG:4326` |
| `validate_crs` on Mars south polar | **Refused** | Same |
| `crs_display_info` on Mars south polar | **Refused:** no known area of use | The map grid needs an area of use, and PROJ has none for IAU CRSs |
| Identify on a Mars raster | **Error** | Pivots through `EPSG:4326` |
| Import an ISIS3 `.cub` or PDS4 `.xml` | **Refused:** "Could not read … Query returned no rows" | The importer routes these files to the vector reader, though GDAL itself reads them |

### Earth assumptions still in the code
- 42 references to `4326` in the engine, and 35 to `3857` across the engine and UI.
- `ST_Area_Spheroid`, `ST_Length_Spheroid` and `ST_Perimeter_Spheroid` are WGS 84 only (`measure.rs`, `table.rs`).
- `raster.rs` passes `gdaldem -s 111120`, Earth's metres per degree. A geographic Mars DEM would get the wrong slope.
- `aeqd_for` in `nodes/mod.rs` hard-codes WGS 84.
- OpenLayers assumes Earth's metres per degree for geographic views, so the scale bar would be wrong without an explicit `metersPerUnit`.
- `crs::suggest` only suggests Earth CRSs.

### Live services
| Service | Result | Licence |
|---|---|---|
| USGS STAC, `stac.astrogeology.usgs.gov/api` | 18 collections. CTX DTMs: 44,628 items with DEM, geoid-adjusted DEM, orthoimage, hillshade and error assets. Items carry `proj:wkt2` and `proj:projjson`. Search by bbox works. | CC0-1.0 on every collection |
| USGS OGC API, `astrogeology.usgs.gov/pygeoapi` | 5 Mars collections: geologic map units, contacts and structures (SIM 3292), Robbins craters, HiRISE footprints | Not stated in the API; USGS-authored data is public domain. Check per collection before building. |
| USGS global mosaics, `planetarymaps.usgs.gov/mosaic` | MOLA DEM and colour shade, Viking MDIM, THEMIS day, LROC WAC, LOLA DEM and colour shade, Mercury, Venus and Europa mosaics all answer. **Three names in earlier notes returned 404**, so every URL gets a network test. | Public domain, credit USGS |
| IAU/USGS place names | Per-body shapefiles. Moon: 9,087 features with name, type, diameter and link. | Check the Gazetteer's terms before building |
| PDS Orbital Data Explorer REST | Service answers. A test query was rejected for a wrong product type, so valid combinations must be worked out. | NASA PDS; check |
| USGS WMS, `planetarymaps.usgs.gov/cgi-bin/mapserv` | Mars map answers with MOLA, THEMIS, MDIM and HRSC layers, offered in `IAU_2015:49900`. Other bodies to be checked. | USGS, public domain; check per layer |
| NASA Trek WMTS | Capabilities document answers | Check terms; only needed if USGS WMS falls short |

### The finding that changes the old plan
**The USGS global mosaics are not cloud-optimized.** MOLA has one-row strips and no overviews, and there's no `.ovr` file. Drawing it at 512×256 took **2 minutes 13 seconds**. They can't be used as live basemaps, though clipping a small area remains practical. So:
- Each main body gets a **small bundled basemap**, the way Natural Earth works today. It draws offline, in any CRS, and prints in layouts.
- Detail while zoomed in comes from the **USGS WMS** on screen, which already serves Mars in its IAU CRS. NASA Trek is the backup, and bundled basemaps plus downloaded clips are the fallback.

---

## 3. Creating an extraterrestrial map

**Where:** the **New Map** button on Insert › Maps becomes a menu: **Earth Map** and **Extraterrestrial Map…**. The same menu appears on the **+** in the map tabs and on Maps in the Catalog. Pressing the plain button on an Earth map behaves as today.

**The dialog** asks, in one step:

| Field | Default | Notes |
|---|---|---|
| Body | Mars | Moon, Mars, Mercury, Venus and Europa first, then "All bodies" (searchable) from PROJ's table |
| Map view | Whole body | Whole body (equirectangular), North pole or South pole (polar stereographic), or Choose a coordinate system… |
| Coordinate convention | Planetocentric, east-positive, −180 to 180 | Hidden behind More for most users. Mars planetographic and 0–360 are offered with an explanation. |
| Basemap | The body's bundled basemap | Or none, for bodies without one |
| Name | "Mars Map" | Editable |

The CRS is set explicitly from the user's choice, so nothing is guessed. A map's body can't be changed afterwards, because its layers belong to that body. **Duplicate Map** keeps the body.

**Opening a native file also leads here.** If you add a Mars file with no Mars map open, the app doesn't put it on an Earth map. It asks: "This file is on Mars. Create a Mars map for it?"

---

## 4. What the mode changes

The mode follows the **active map**, so clicking between an Earth tab and a Mars tab switches everything below.

| Area | On an extraterrestrial map |
|---|---|
| Map tab and status bar | Show the body name, so you can always tell which mode you're in. No other colour or theme change. |
| Ribbon | A **Planetary** tab appears (§7). Earth-only commands are hidden. |
| Basemap menu | That body's basemaps only |
| Catalog › Public Data | Shows **Planetary Data** for that body instead of Earth data (§6) |
| Coordinate system picker | That body's CRSs only |
| Map Layers | **+** offers only project layers of that body; the rest can't be added, and the reason is stated |
| Shift+A tool menu | Hides Earth-only tools; shows the Planetary category |
| Status bar | Coordinates, scale, scale factor and convergence use the body's figure, in the map's convention |
| Scale bar, measuring | Use the body's radius or ellipsoid |
| Top search | Finds places in the body's place names, if downloaded |

**Earth-only, hidden in this mode and refused by the engine:** USNG/MGRS tools and grids, declination element, Topo Map Builder, TIGER, National Map, 3DEP, Census, Overture, ArcGIS and earthquake/soil downloads, Earth basemaps, and the Natural Earth world outline.

**One tool definition stays the rule:** `ToolSpec` gains one field, `bodies: Any | EarthOnly`. The menu and the engine both read it. A guard test fails if a new tool doesn't declare it. Each tool's default gets checked once, not assumed: hydrology and spectral indices need a specific decision.

**Across the project:**
- A layer's body comes from its CRS. It isn't stored separately, so the two can't disagree.
- A tool with inputs from two bodies stops with a sentence naming both. Nothing is reprojected across bodies.
- A layout can hold frames from different bodies, for example Earth beside Mars at the same scale. Each frame uses its own map's body.
- Project format goes to 3, so an older build refuses a file with a Mars map instead of drawing it wrongly. Older files open with every map as Earth.

---

## 5. Native planetary files

| Format | Extensions | What's needed |
|---|---|---|
| ISIS3 cube | `.cub`, `.lbl` | Route to the raster reader (it fails today) |
| PDS4 | `.xml` with its data file | Same; vector tables too |
| PDS3 | `.img`, `.lbl` | Same |
| VICAR | `.vic`, `.img` | Same |
| GeoTIFF, GeoPackage, Shapefile with an IAU or other body CRS | Existing extensions | Works once `validate_crs` is fixed |

**Importing:**
- Add Data and drag-and-drop detect these formats by their contents, not just the extension. `.img` and `.xml` are too common to trust.
- A label and data pair counts as one item, and stays beside the original. The original is never modified.
- Body, CRS, longitude direction and latitude type come from the label. **If a label doesn't say, the app asks and doesn't guess.**
- Raw, unprojected camera images (for example raw EDRs) are explained and refused: "This image has no map projection yet. It needs processing in ISIS first."
- Files are copied into the raster store like other rasters, so projects stay portable.
- Label fields such as incidence, emission and phase angles, and the product ID, appear in Properties and go in the citation.

**`--self-test` gains a check** that the ISIS3, PDS, PDS4 and VICAR drivers exist in the packaged build. The conda-forge `libgdal-core` used for Windows and Linux hasn't been confirmed to include them. R7 will confirm that, so it's a risk until then.

---

## 6. Built-in planetary data

**Planetary Data** is one browser in the Catalog, filtered by the map's body. Every source follows the existing Public Data rules: area of interest from the map view or a layer, size shown before downloading, progress bar and Cancel, a citation in `sources.rs`, and CRS taken from the source.

| Source | Bodies | Built on | Gives you |
|---|---|---|---|
| **USGS Analysis Ready Data** | Mars, Moon, Europa and others | Existing `stac.rs`, with the USGS API added | Search by area and collection, thumbnails, then choose DEM, orthoimage, hillshade or other assets. Reads CRS from `proj:wkt2`. |
| **Global Mosaics** | Moon, Mars, Mercury, Venus, Europa | Curated list, clip through GDAL | A clip of a global DEM or image for your area. **Refuses a whole-body clip of a strip mosaic with a size warning.** |
| **Geologic Maps and Craters** | Mars first; Moon from the Unified Geologic Map | Existing `ogcapi.rs` | Units, contacts, structures, Robbins craters. Unit colours where the source has them. |
| **Place Names** | All with Gazetteer files | Download and cache a shapefile | A label layer and "Go to place" (Gale, Shackleton) |
| **Image Footprints** | Mars, Moon | PDS Orbital Data Explorer | A layer of HiRISE, CTX or LROC NAC footprints with links. Downloads only map-projected products. |

`stac.rs` and `ogcapi.rs` currently work in Earth longitude and latitude. They need three changes: a bbox in the body's own terms, 0–360 sources handled correctly (including areas that cross the seam), and clips that use the source's CRS with no pivot through Earth.

**Bundled basemaps.** Moon (LOLA shaded relief or LROC WAC), Mars (MOLA shaded relief), Mercury, Venus and Europa, each a small raster of about 1 MB or less, built by a script like `scripts/make-natural-earth.sh`. They generalize today's `__naturalEarth` tile id, so they draw in any CRS of that body and print in layouts. Installer growth: about 5 MB.

**Citations to record:** the USGS ARD product and item ID, Tanaka et al. 2014 (SIM 3292, <https://pubs.usgs.gov/sim/3292/>), Robbins & Hynek 2012 (<https://doi.org/10.1029/2011JE003966>), Fortezzo, Spudis & Harrel 2020 (Unified Geologic Map of the Moon), the Gazetteer of Planetary Nomenclature, and the PDS product ID. All sources go in `docs/OPEN_SOURCE.md` in the same commit.

---

## 7. Planetary ribbon tab

Appears only while an extraterrestrial map is active.

| Group | Commands |
|---|---|
| **Body** | Body name (read-only), Coordinate Convention, Go to Place |
| **Get Data** | USGS Data, Global Mosaics, Geologic Maps, Craters, Place Names, Image Footprints, Open Planetary File |
| **Analyze** | Digitize Crater, Crater Count, Crater Morphometry, Height Reference, Convention Converter |

All analysis commands are also in Shift+A › Planetary.

---

## 8. Correct geometry on any body (the core work)

| Item | Change | Test value |
|---|---|---|
| **Body registry** | New `crates/engine/src/body.rs`, generated from `proj.db` by a script, not typed by hand. Includes `body_of(crs)`, using PROJ's own answer. Earth stays EPSG. | Mars a = 3,396,190 m; Moon = 1,737,400 m |
| **Pivot** | Replace each `EPSG:4326` pivot with the layer's or map's own geographic CRS. **Every one of the 42 gets looked at.** Earth's pivot stays 4326. | Mars GeoPackage imports; identify on a Mars raster returns a value |
| **Lengths and areas** | Off Earth, use `geographiclib-rs` (arbitrary ellipsoids and polygon area; check licence is MIT before adding). Earth keeps the DuckDB functions, so its results can't change. | Mars: 1° along the equator = 59.27 km. Moon: 30.32 km. Mars 1°×1° box at the equator = 3,513 km². |
| **`aeqd_for`, buffers** | Take the body's figure | Mars 100 km buffer has the same area in any CRS |
| **Terrain** | `gdaldem -s` from the body: 2πR/360 m per degree | 10° synthetic Mars ramp gives slope 10° ± 0.1 |
| **Map grid** | With no area of use, IAU CRSs get one from the projection: whole body for equirectangular, a hemisphere for polar | South polar Mars map opens and draws |
| **Vector display** | Non-Earth layers always use display tables, never the Mercator tile path | Tiles draw in Mars equirectangular and polar |
| **OpenLayers** | Pass PROJ's definition and the body's `metersPerUnit`. Hide the datum-offset notice. | Scale bar across 1° reads 59.27 km |
| **Conventions** | Each layer states planetocentric or planetographic, east or west positive, 0–360 or ±180. The Convention Converter tool is explicit, never silent. | Round trip returns the same coordinates; Mars planetographic differs from planetocentric by the expected amount |
| **Seam** | Features crossing 180° or 0°/360° are cut (existing world-view method with the body's radius) | A polygon across the seam draws whole |
| **Earth unchanged** | **Guard: existing engine, CRS and pixel tests must pass with no change in expected values.** | No golden test updates allowed in this work |

**Layouts.** `layout/geo.rs` gets every body from the registry. Each frame takes its body from its map. The scale bar, grid and CRS text use it, grid labels support 0–360, and text tokens add `{body}`. A planetary template gets a title block, body, CRS statement, and credits from `sources.rs`. Geologic symbols per FGDC-STD-013-2006 are reimplemented only if time allows.

---

## 9. Planetary analysis

Existing tools become correct once §8 lands. The new tools below are built from the published methods and cited, except where they run a bundled program, as noted.

| Tool | Method | In first release? |
|---|---|---|
| **Convention Converter** | PROJ ocentric ↔ ographic; 0–360 ↔ ±180; east ↔ west | Yes |
| **Height Reference** | Mars sphere ↔ areoid; Moon relative to 1,737.4 km sphere | Yes; areoid source must be checked first |
| **Digitize Crater** | 3 rim points or 2-point diameter gives a true circle on the body (Kneissl, van Gasselt & Neukum 2011, <https://doi.org/10.1016/j.pss.2010.03.015>) | Yes |
| **Crater Count** | Counting area and size–frequency plots: cumulative, differential, R-plot (Crater Analysis Techniques Working Group 1979, <https://doi.org/10.1016/0019-1035(79)90009-5>). Digitizing is native; statistics and plots run in the bundled Craterstats. | Yes |
| **Crater Morphometry** | Depth, rim height and depth/diameter from a DEM | Yes |
| **Model Ages** | Neukum production function and chronology; Poisson fitting (Michael et al. 2016, <https://doi.org/10.1016/j.icarus.2016.05.019>). **Runs the bundled Craterstats program (§13)**, so results match the tool the field uses. | **Yes (user, 2026-11-04).** Must reproduce its sample results. |
| **Geologic mapping tools** | The USGS PGM toolbox's workflow rebuilt as native tools (§13) | Yes |
| Coverage statistics, site suitability | Existing overlay tools with presets | After release |
| ISIS, Ames Stereo Pipeline, USGS CSM bridges | Detect an install and run as nodes | After release |

---

## 10. Work packages

Same way of working as the redesign: Opus reviews and merges, Sonnet writes code, one commit per package, `[skip ci]`, and one full test run and one real-app review per batch. **Read closely: pivots, geodesics, project format, licences.**

**Starts after** the UI redesign merges into `main`, because this builds on multiple maps and basemaps, and after the open bugs are fixed and a rebuilt Mac app has been tested (user, 2026-11-04).

| WP | Work | Done when | Sessions |
|---|---|---|---|
| **P0** | Spike on real files: Moon label body detection, ISIS3 and PDS4 fixtures, polar grid, seam, 0–360 source. Settle anything this plan got wrong. | Written results in this file | 0.5 (Opus) |
| **P1** | Body registry, `body_of`, pivots, geodesics, terrain, grid, display (§8) | Every failure in §2 is a passing test, and Earth values are unchanged | 2 |
| **P2** | Extraterrestrial Map dialog, `MapDoc.body`, format 3, mode switching, body filters, `ToolSpec.bodies`, Planetary tab (§3, §4, §7) | Earth and Mars maps side by side, each behaving correctly; old projects open | 1.5 |
| **P3** | Native files (§5) and bundled basemaps (§6) | Open an ISIS3, PDS3, PDS4 and VICAR fixture; the "Create a Mars map?" prompt works; a basemap draws offline in polar and equirectangular | 1 |
| **P4** | Planetary Data browser (§6) | Each source downloads a real clip with a correct CRS and citation; ignored network test per endpoint | 1.5 |
| **P5** | Layouts (§8) | A Mars PDF with correct scale bar, grid and credits (pixel test); an Earth and Mars frame on one page | 1 |
| **P6** | Analysis tools (§9) and geologic mapping tools (§13) | Each tool checked against a published or hand-calculated value; a USGS-style map database created and checked | 2, plus an Opus method review |
| **P7** | Final check: walk-through on Moon and Mars, `docs/TEST_PLAN.md`, `docs/OPEN_SOURCE.md`, `THIRD_PARTY_NOTICES.md`, Earth regression | All tests pass; screenshots saved; docs updated | 0.5 |

**Total: about 9 sessions.** P1 comes first, alone, since everything depends on it. After that, P2 and P3 can run side by side only if P2 owns `ribbon/*` and `store/*`. P4, P5 and P6 can run side by side once P2 and P3 merge.

### Tests
- **Reference values:** the numbers in §8 from hand calculation or published figures, never the engine's own output.
- **Fixtures:** small ISIS3, PDS4 and VICAR files made with `gdal_translate` and checked in. A PDS3 fixture needs another source, because GDAL can't write PDS3.
- **Network:** an ignored test for each endpoint and mosaic URL, as with TIGER.
- **e2e:** create each kind of map, switch tabs, confirm hidden commands, cross-body refusal, save and reopen, Earth regression.
- **WebKit:** the Mac app uses it, so run the new specs with `E2E_WEBKIT=1`.

---

## 11. Risks

| Risk | Plan |
|---|---|
| A missed 4326 or 3857 pivot gives a wrong answer without an error | PROJ refuses cross-body transforms, so most fail loudly. A guard test runs every `Any` tool on a Mars layer and fails on errors. |
| Changing pivots breaks Earth | Earth keeps its code paths, and no golden values may change |
| Planetary drivers are missing from the Windows or Linux build | `--self-test` check; R7 confirms. If missing, adjust the conda build or fix in R7. |
| Services change or disappear | Every URL has a network test, failures read as sentences, and bundled basemaps keep working offline |
| The USGS WMS is slow or lacks a body | Use Trek if its terms allow, or the bundled basemap plus clips |
| Bundled Craterstats grows the installer by about 90 MB (DMG is 170 MB today), or its Linux build fails | Accepted for now; fallback in §13 |
| Size creep delays the release | §9 and the Mercury, Venus and Europa data are the first things to cut. Moon and Mars core are not. |
| Crater ages look authoritative | Ages stay out unless reviewed against published test cases |

---

## 12. Decisions still needed from you

1. **Bodies at first release:** Moon and Mars full, Mercury, Venus and Europa with basemap and curated data? *Recommended: yes.*
2. **Python SDK after release:** this follows from "final build feature", and the plan assumes it. Tell me if that's wrong.

---

## 13. USGS tools and other open-source software to reuse

Checked 2026-11-04 on the USGS Astrogeology Software page, `code.usgs.gov/planetary-geologic-mapping`, and GitHub. Licences were read from each project's own files.

**Nothing from USGS's planetary toolkit is integrated yet.** The USGS tools are mostly ArcGIS add-ins, so they can't run in this app, but their licences allow rebuilding the same workflow natively.

### Rule for direct integration (user, 2026-11-04)
A tool can be integrated directly if **it and everything it needs can be packaged inside the app**: no install, no separate runtime, all three platforms, licence allows it. WhiteboxTools is the model: a pinned build script, a separate program in `resources/`, a licence file, and a `--self-test` check. The GPL rule in `CLAUDE.md` still applies.

| Tool | Packaged whole? | Decision |
|---|---|---|
| **Craterstats** v3.6.11 (BSD-3-Clause) | **Yes.** Command-line program, with a PyInstaller spec and releases for Windows and macOS (macOS marked beta). Dependencies are permissive as far as known; the notices script confirms. **We must build Linux ourselves.** About 90–100 MB per platform. | **Integrate directly** as `resources/craterstats/`, built by `scripts/build-craterstats.sh` from a pinned tag. App writes its input files, runs it, and reads its CSV, PDF and SVG. |
| **Geologic symbols, Planetcantile grids** | Yes, data files | Bundle |
| **WhiteboxTools** | Already bundled | Expose more tools (below) |
| **USGS ARD search plugin** (CC0) | Python inside QGIS; its `stac_client.py` is standard-library only | Copy its logic into Rust, no Python needed. CC0 allows it. |
| **USGS PGM Toolbox, OpenCraterTool for ArcGIS** (CC0) | **No:** need ArcGIS, which is proprietary | Rebuild natively |
| **OpenCraterTool for QGIS, Mappy** (GPL) | **No:** need QGIS, and `CLAUDE.md` says never to require it | Rebuild from the method |
| **ISIS** | **No:** large conda install, no native Windows | Optional bridge later |
| **Ames Stereo Pipeline** (Apache-2.0) | Self-contained, but 346–702 MB, with no Windows build in the latest release | Optional bridge later |
| **pdr** (BSD-3-Clause) | Possible, needs Python | Later, only if GDAL can't read a format users need. Could share Craterstats' runtime. |

**Craterstats packaging work (in P6):** build script for all three platforms, licence files and `THIRD_PARTY_NOTICES.md`, a `--self-test` check that it runs, and a test reproducing its sample results. Its command-line options must be pinned to the version, because the app depends on them. If the Linux build fails, Linux falls back to a native port of the fitting using its `functions.txt`, or ages wait on Linux.

### USGS Planetary Geologic Mapping (PGM) toolkit
| USGS item | Licence | What it does | How to use it here | Effort |
|---|---|---|---|---|
| **PGM ArcGIS Toolbox** › New Map GDB | CC0 | Creates a map database: GeoContacts, LinearFeatures, LocationFeatures, SurfaceFeatures, MapBoundary, each with defined fields | **New Geologic Map** command creating the same layers with the same field names | Low |
| › Build Polygons | CC0 | Builds unit polygons from contacts | Tool using the existing `lines_to_polygons`/`ST_Polygonize`, with optional label points | Low |
| › Topology Check | CC0 | Gaps and overlaps in polygons; dangles, pseudo-nodes and intersections in lines | Tool writing a layer of errors, which the app lacks today | Medium |
| › Longitude Conversion | CC0 | 0–360 ↔ ±180, including layers crossing the seam | Becomes the Convention Converter (§9) | Low |
| › Merge Files | CC0 | Merges layers, recording the source | The existing `merge` tool, plus one source field if missing | Very low |
| › Basemap Clip-N-Ship | CC0 | Clips basemap rasters to a map area | Covered by Global Mosaics (§6) | Done by plan |
| › Slope & Aspect Test | CC0 | Tests a DEM's slope and aspect | Existing tools; check what its test adds first | Very low |
| **Planetary ARD Search Tools** (QGIS, ArcGIS) | CC0 | Target › Collection › Asset search with everything read live from the catalogue | Follow its flow for the §6 browser, so new USGS data appears without an app update | Low |
| **OpenCraterTool for ArcGIS Pro** | CC0 as published | Crater mapping and size-frequency workflow | Use its workflow as a guide only. **It's described as an implementation of the GPL-2.0 QGIS plugin, so don't copy its code**; build from the published methods. | Guide only |
| **FGDC Section 25 symbols** | Federal standard | Planetary geologic symbols | Draw natively with the `topo_symbols.rs` system. `afrigeri/geologic-symbols` has 38 planetary SVGs under CC BY 3.0, a permissive starting point. Avoid `geologic-symbols-qgis` (CC-BY-SA) as a bundle. | Medium |
| **Instructions for Authors** templates, `pgm-documentation` | Public domain | Map submission structure and procedures | Shape the planetary layout template and in-app help | Very low |
| **ISIS** | Public domain | Processes raw mission images | Optional bridge after release (§9) | High |
| **ALE, USGS CSM, SpiceQL, Knoten, Plio** | Public domain | Camera models and ephemerides | Not needed for map-projected data; skip | — |
| **PyHAT** | Not confirmed | Spectral analysis, in Python | Skip for now | — |

### Other projects that fit
| Project | Licence | Use | Effort |
|---|---|---|---|
| **geographiclib-rs** | MIT | Geodesics on any ellipsoid. **Already a dependency**, so §8 adds nothing new. | None |
| **Planetcantile** | BSD-3-Clause | IAU tile grid definitions, a reference for §8's map grids | Very low |
| **Circle Craters** | BSD-3-Clause | Three-point crater digitizing, a small reference | Very low |
| **GeMS tools** (USGS) | CC0 | Earth geologic map schema, a later option alongside the planetary one | After release |

### Reuse the idea only, or skip
| Project | Reason |
|---|---|
| OpenCraterTool (QGIS), Mappy, GRASS, DeepLandforms, Cratermaker | GPL: reimplement from the method and cite, as `CLAUDE.md` requires |
| pdr, PlanetaryPy, PlanetMapper, SpiceyPy | Permissive, but Python. They'd fit Phase G. GDAL already reads the main formats. |
| Ames Stereo Pipeline | Apache-2.0, but a large install; optional bridge after release |
| MMGIS, JMARS, QuickMap | Separate applications, not components |
| YOLOLens and other crater detectors | Needs PyTorch and adds uncertainty to scientific counts; not before release |

### Biggest low-effort win for the whole app
**WhiteboxTools is already bundled with 459 tools, and the app exposes only six hydrology nodes.** Wrapping more takes little work per tool, needs no new dependency, and works for Earth too.

| Priority | Tools | Why |
|---|---|---|
| **Before release (planetary needs them)** | `Profile`, `MultiscaleRoughness`, `DevFromMeanElev`, `PlanCurvature`, `ProfileCurvature`, `Geomorphons`, `WeightedOverlay`, `Viewshed` | Crater and terrain work, and landing or traverse suitability. Roughness is a standard landing-site measure. |
| **After release** | `CostDistance`, `CostPathway`, `EuclideanDistance`, `FillMissingData`, `HypsometricAnalysis`, `PrincipalComponentAnalysis`, `Clump` | General analysis |
| **Not without checking** | `TimeInDaylight`, `HorizonAngle` | They may assume Earth's sun geometry, so test before offering them off Earth |

P0 must confirm WhiteboxTools accepts rasters in an IAU CRS and uses correct cell sizes. It works in projected metres, so geographic planetary DEMs need the same treatment as Earth's.
