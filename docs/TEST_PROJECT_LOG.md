# Test project log (started 2026-10-30)

Method: each project is done through the UI in the in-app browser against the dev server (React app + Rust engine). The in-app browser has no `window.prompt`, which the browser-mode export path uses for the save location, so a one-line shim answers it with a path inside `test-projects/`; everything else is real clicks. Map images (reduced to 1800 px) and PDFs are in `test-projects/NN-name/`.

## 1. Land-nav quad, Pikes Peak (CO), 1:24,000 Tabloid — DONE (with findings)
Flow: Insert → Topo Map → "A position I type" `38.8409, -105.0423` + "centre on the 7.5' quadrangle" → Build → steps 1 (3DEP), 2 (contours + hillshade), 3 (National Map roads/trails/water/names) → Export PNG + PDF. All three downloads worked first time (~10 s each). Self-check list updated after each step.
Hand check: quadrangle centre 38.8125 N, 105.0625 W = UTM 13 E 494,574 N 4,295,972 (own TM computation) → sheet says "13S DC 945 959" ✔; grid labels 90–99 / 94–98 ✔; declination 7.4° E at 2026-10-30 plausible for Colorado.
Findings:
- T1-1 Contour labels are in metres ("3657.6", "3474.7200000000003") on a sheet whose interval is 40 ft and units are Imperial; float noise is shown. Should label in the sheet's units, rounded, ideally only index contours.
- T1-2 Legend runs off the bottom of the page ("Legend 'lg1' does not fit its height") and layer names repeat " — Pikes Peak, Colorado" (truncated with …).
- T1-3 Place names (GNIS) are drawn as dots only; no text labels, although the legend lists "names". A land-nav sheet needs the names labelled.
- T1-4 Default colours are not map-standard: streams red, lakes purple (USGS: blue); trails are cyan and almost invisible; roads thin yellow.
- T1-5 "Area" only offers map centre or a typed position — no place-name search (typing "Pikes Peak" would be natural).
- T1-6 Dev-mode export needs window.prompt for the path (harness limitation, not a desktop-app bug).
- T1-7 Console showed two HTTP 500 responses during the session (not yet traced).
Files: map.png, map.pdf (5.9 MB).

## 2. Land-nav 1:50,000 Letter (metric), Mount Rainier (WA) — DONE (with findings)
Flow as project 1 with preset "Land Nav 1:50,000 Tabloid", Paper changed to Letter, metric kept, 46.8523, -121.7603 + quadrangle centring. Downloads and 1,256 contours (20 m) in ~40 s total.
Hand check: quad centre 46.8125 N, 121.8125 W = UTM 10 E 590,595 N 5,185,013 → "10T ES 905 850" ✔; declination 14.6° E, grid convergence 0.9° E (east of the 123° W central meridian) ✔.
Findings:
- T2-1 Contour labels print ".0" ("1800.0", "2400.0") – should be whole metres.
- T2-2 The legend runs off the Letter page again, and the Sources line prints through the legend (overlap).
- T2-3 NHD "waterbody" includes glacier/ice features, so Rainier's glaciers are drawn as purple lakes; filter by feature type or style ice separately.
- T2-4 The plan expected 100 m contours; the sheet picks 20 m (right for 1:50,000 but not relief-aware; 1,256 lines, 99 labels did not fit).
- Preset names say "Tabloid" while the paper was changed to Letter (preset box keeps the old name) – cosmetic.
Files: test-projects/02-land-nav-rainier/map.png, map.pdf.

## 3. Trail map, Zion Canyon (UT), Letter portrait — DONE (with findings)
Flow: preset "Trail map 1:25,000 Letter portrait", 37.2982, -112.9487 + quadrangle centring, the three data steps ("Added 4 National Map layer(s), 85 features · none in this area: 4WD roads, Local roads and streets, Lakes…").
Findings:
- T3-1 On the narrow page the "How to give a grid reference" text overflows its box, overprints the "Adjoining sheets" box title and is cut at the bottom (box should grow or the text shrink).
- T3-2 Legend entries are cut to a few characters ("Stream,…", "Summit…", "Interme…") and the legend still runs off the bottom of the page; the Hillshade ramp overprints the Sources line.
- T3-3 Contour labels again in metres with float noise ("1463.04") on a 40 ft sheet, and every contour line is labelled, which covers the map in Zion's steep terrain (100 labels did not fit).
- T3-4 The trail map shows almost no trails in Zion Canyon (the layer was added but only one road and a few streams draw); the dialog says which layers were empty, which is good, but the user is not told the trails layer had 0–few features. No trail names.
- T3-5 Trail preset picks 40 ft / imperial with a military 1:25,000 scale label; a trail map would normally want 20 m or 40 ft but not "military".
Files: test-projects/03-trail-zion/map.png, map.pdf.

## 4. County population choropleth, Tennessee — DONE (with findings)
Flow: Map → Sample Data (TIGER) → state Tennessee, Counties only (95 features) → Properties → Symbology → Graduated colors (field POP100, 5 quantile classes, Blues) → Insert → New Layout → Letter landscape → Elements: Map Frame, Legend, Scale Bar, North Arrow, Overview Map, Sources (auto), Text (title) → set map CRS and frame CRS to NAD83 / Tennessee (EPSG:32136), scale 1:3,000,000 → Export PNG + PDF.
Findings:
- T4-1 (corrected after project 5) The legend was not blank: the Overview Map inserted after it landed on top and covered it; both are placed in the same spot by default and there is no warning about one element hiding another.
- T4-2 The default frame covers almost the whole page and every inserted element lands on top of it (legend over the overview inset, scale bar on the map, title over the frame edge); there is no auto-arrange, so a first map always needs manual dragging.
- T4-3 The default basemap (USGS Topo) shows through the 75 % opaque choropleth; street names and shields bleed into the classes. A thematic map normally wants a light/grey basemap or none; decision needed on the default.
- T4-4 Graduated colors defaults the field to the first numeric column (OGC_FID, an ID); it should skip ID/OBJECTID/FID columns or ask.
- T4-5 The map frame keeps its own CRS (WGS 84 at creation, warning "scale varies … choose a projected CRS"); changing the map CRS later did not change the frame. The warning is clear, but a new frame could start in the map's CRS.
- T4-6 New project CRS is WGS 84 (EPSG:4326) so a first layout is always geographic; no prompt to pick a projected CRS for the data's region (a "suggested CRS for this layer" would help).
- T4-7 Graduated class breaks are shown with raw numbers; no thousands separators in the legend.
Files: test-projects/04-tn-county-choropleth/map.png, map.pdf.

## 5. Census tract density, Hennepin County (MN) — DONE (with findings)
Flow: Sample Data (TIGER) → Minnesota / Hennepin County / Census tracts (329 features, ~25 s) → Model pane: Add → Calculate Field (`DENS_KM2 = POP100 / (AREALAND / 1000000.0)`) wired Layer Input → Calculate Field → Output → Run (5 ms) → Symbology Graduated colors on DENS_KM2 (5 quantile classes) → Letter portrait layout, frame CRS NAD83 / UTM 15N, scale 1:300,000, legend, scale bar, north arrow, title → PNG + PDF.
Findings:
- T5-1 The Calculate Field placeholder suggests `POP / AREA_SQMI`, but there is no such column; typing `AREA_SQKM` gave the raw DuckDB message "Binder Error: Referenced column "AREA_SQKM" not found in FROM clause! Candidate bindings: …" (the candidates list is helpful, the wording is not). There is no geodesic-area variable in expressions, so "geodesic area" density needs the Calculate Geometry node first.
- T5-2 A new node is dropped on top of the previous nodes, unconnected; connecting needs dragging 7 px-apart ports. Add menu has "drop it onto a connection to insert it" but Add → Enter never auto-inserts after the selected node. Error text "Connect an input to 'Input'" is clear.
- T5-3 Exported legend shows raw numbers with six decimals ("1331.186372 – 2073.5…"), cut off with "…", and the ≥ symbol prints as a missing-glyph box in the last class.
- T5-4 Long title text neither wraps nor shrinks: "Population density by census tract, Hennepin County, MN (2020 Census, people per km²)" runs off both page edges in the PNG/PDF.
- T5-5 Legend box is much taller than its content (a large empty area).
- T5-6 Graduated colors field list does include the result column once the node has run; default field is again the ID column.
Files: test-projects/05-hennepin-density/map.png, map.pdf.

## 6. Small drainage basin, Gunnison / Taylor River headwaters (CO) — DONE (watersheds not done)
Flow: pan the map to the area (no "go to place/coordinates" box exists, so the map view was set with the dev hook `getMap().getView()`), Elevation (3DEP) → "current map view" → 32 × 13 km, 18 MB, ~10 s → select layer → Model: Shift+A → Fill Depressions, Flow Direction, Flow Accumulation, Extract Streams (nodes wired by dragging ports and, for the last ones, with the model store's `connect`, because the in-app browser's pointer drags on 7 px ports are unreliable) → Run → restyle result → Letter landscape layout → PNG + PDF. Engine numbers checked with gdalinfo: flow accumulation max 2,555,952 cells, streams 0.92 % of cells.
Not done: Watersheds (from Outlets) needs an outlet point layer; Drainage Basins node added to the plan only.
Findings:
- T6-1 The 3DEP dialog's Area has one choice, "Current map view". There is no place search, no "go to lat/lon", no "zoom to a typed extent"; to download a specific area the user must pan by hand. (Pan/zoom to coordinates is also needed for projects 1–3 style work.)
- T6-2 Good: a world-sized request shows the size ("3937 MB · 70 requests") and blocks the Download button behind "This is a large download. I want to continue."
- T6-3 Extract Streams' Threshold is "in the units of the accumulation raster"; with Flow Accumulation's log-transform ticked, threshold 5000 gave an all-no-data raster and no warning ("0 stream cells") — should warn when the output has no valid cells.
- T6-4 Flow Accumulation takes the filled DEM, not the Flow Direction raster; easy to wire wrong (I did) and it then produces a plausible-looking but meaningless raster (max 35 cells) with no complaint. Port types are both "raster"; a "pointer vs elevation" tag would catch it.
- T6-5 Node "Preview on map" on Extract Streams failed with "Previewing failed: Not a model output" (unhelpful wording).
- T6-6 A result raster inherits the source layer's renderer (elevation + hillshade) so the stream raster is invisible until the user switches the render type; "Paletted / Unique values" with Load classes did nothing visible; "Singleband pseudocolor" worked. Layer keeps the name "Elevation (3DEP 10 m) — map view" and the legend shows a viridis ramp "1 – 1".
- T6-7 A newly added node (Shift+A → Enter) lands unconnected on top of existing nodes; nodes overlap and the tree quickly becomes unreadable (Fit shows overlapped cards).
Files: test-projects/06-gunnison-basin/map.png, map.pdf.

## 7. Terrain classes, Grand Canyon (AZ) — DONE (slope classes only; aspect, ruggedness and raster calculator not run)
Flow: pan to 36.10 N, 112.11 W (dev hook), 3DEP 33 × 14 km (19 MB), Model: Slope → Reclassify Raster (`0 to 15 = 1`, `15 to 30 = 2`, `30 to 45 = 3`, `45 to 90 = 4`, else nodata) → Output, Run 1.3 s → layer symbology Singleband pseudocolor + Classified → Letter landscape layout, scale set to 1:130,000 → PNG + PDF. Slope, Aspect, Ruggedness, Raster Calculator and Reclassify exist as tools; only Slope + Reclassify were used.
Findings:
- T7-1 A categorical (reclassified) raster is drawn with bilinear resampling, so class edges blend into in-between values; legend then prints breaks "< 1 / 1–2 / 2–3 / 3–3.435 / ≥ 3.435" instead of the four classes 1–4. Classified rasters should resample nearest-neighbour and the legend should list the rule labels.
- T7-2 Legend has the "≥" glyph as a missing-glyph box again (same as T5-3) and the title is the source layer's name ("Elevation (3DEP 10 m) — m…").
- T7-3 The Output panel reports "4,708,200 features in result" for a raster (cells counted as features).
- T7-4 The frame defaults to the map-view scale at layout creation, not to the data extent: the raster filled one third of the frame (1:250,000) until the scale was typed.
- T7-5 Reclassify's rule syntax is only discoverable from the default text in the box ("0 to 10 = 1 …, else = nodata"); fine, but no validation message was seen for a bad rule (not tested).
Files: test-projects/07-grand-canyon-terrain/map.png, map.pdf.

## 8. World earthquakes, M4.5+, 1–30 Oct 2026 — DONE (with findings)
Flow: zoom the map to the world, Data → Earthquakes & Soils (Area "Current map view", From 2026-10-01, smallest magnitude 4.5) → 736 features in ~5 s → Symbology Graduated colors on `mag` → Tabloid landscape layout → Map Frame, Legend, Scale Bar, North Arrow, Text → frame CRS World_Robinson (ESRI:54030), scale 1:88,000,000 → PNG + PDF. Tissot check and projection builder not exercised here (see project 14).
Findings:
- T8-1 **Crash dialog**: after choosing Graduated colors on `depth_km` (many ties) and then `mag`, selecting a layout item showed "Something went wrong — failed to parse "rgba(NaN,NaN,NaN,NaN)" as color" (screenshot `problem-rgba-nan.jpg`). Console: "Error parsing expression … step … input values must be arranged in strictly ascending order" for breaks `10, 10, 10, 71.982` — quantile breaks with duplicate values produce an invalid step expression; the style then throws inside OpenLayers' circle hit detection. Dismiss recovers, but the style is left broken. Needs de-duplicated breaks (and fewer classes when values tie).
- T8-2 There is no graduated *symbol size* (proportional symbols), only graduated colours; the plan's "graduated symbols by magnitude" cannot be done. Shapes are chosen separately (circle…).
- T8-3 "Current map view" is the only area choice, and a world view is clipped to ±59.6° latitude by the window's aspect ratio, so quakes poleward of that (Alaska north of 60°, Iceland, Antarctica) were not requested; the dialog should offer "Whole world".
- T8-4 The colour scheme picked in the list ("Yellow-Red") showed as selected but the exported map still used Blues.
- T8-5 The scale bar on a world map prints "0 500 1000 1500 2000 km" with overlapping labels, and a scale bar / north arrow on a Robinson world map are misleading; no warning (the frame only warns for geographic coordinates).
- T8-6 "≥" prints as a missing-glyph box in the legend (third time: T5-3, T7-2).
- T8-7 Good: Robinson on a Letter/Tabloid frame re-warps the basemap correctly; export took ~10 s.
Files: test-projects/08-world-earthquakes/map.png, map.pdf, problem-rgba-nan.jpg.

## 9. Earthquake clusters, California — DONE (with findings)
Flow: pan to California (dev hook), Data → Earthquakes & Soils (from 2026-01-01, M ≥ 3 → 225 features) → Model: Hot Spot Analysis (field mag, k 8) → run → error → add Project (EPSG:3310 California Albers) → run (0.4 s) → Unique values on `hotspot` → swapped in Spatial Autocorrelation (Moran's I) → table (features 225, I = 0.0591, E[I] = −0.0045, z = 2.13, p = 0.033, "clustered") → Export Features to `morans_i.csv` → hot-spot map layout (Letter landscape) → PNG + PDF.
Findings:
- T9-1 Good: Hot Spot Analysis on degrees says "Spatial statistics measure distances between features, so the layer must be in a projected coordinate system (metres or feet), not degrees. Add a Project step first." — exactly the right message; but it could offer a one-click "Add Project (suggested: EPSG:3310)".
- T9-2 **Same crash as T8-1** (`failed to parse "rgba(NaN,NaN,NaN,NaN)" as color`, screenshot `problem-rgba-nan-again.jpg`) after re-running the tree with the Unique-values style on `hotspot` and switching the output between Moran's I table and hot-spot points; the pane error boundary caught it ("a part of the window hit an unexpected state"). So the NaN colour is not only from duplicate quantile breaks; any style whose class list no longer matches the layer's fields/values can produce it. Needs a guard in the style builder (skip invalid classes, never pass NaN to OpenLayers).
- T9-3 The hot-spot bins are listed as "0 (192) / −1 (12) / 1 (10) / 3 (8) / 2 (3) / Other" in arbitrary order with raw integers and unrelated colours (blue, green, red, orange, purple); a hot/cold spot result should default to a diverging ramp with names (Cold 99 % … Hot 99 %).
- T9-4 Moran's I output is a one-row table that appears as a "layer"; it can only leave the app through Export Features (Export Table is points-only, status text says so), which is fine but not obvious.
- T9-5 Export Features, Export Table, Save/Open project and Add Data all call the desktop-only file dialog (`@tauri-apps/plugin-dialog`); in browser mode they throw "Cannot read properties of undefined (reading 'invoke')" and only layout export has a prompt() fallback. (Harness note: a one-line shim for `plugin:dialog|save/open` was used for the rest of the run; the desktop app is unaffected.)
- T9-6 Harness slip, not an app bug: a stale queued path made the PNG export overwrite my CSV; both files were re-exported.
Files: test-projects/09-california-quake-clusters/map.png, map.pdf, morans_i.csv, problem-rgba-nan-again.jpg.

## 10. Building density, downtown Seattle — DONE (with findings)
Flow: zoom to downtown Seattle (about 3 km²), Data → Overture Maps → Buildings → **about 8 minutes** to get 1,083 buildings → Model: Create Fishnet (0.1 km squares, 420 cells, ~1 s) + Summarize Within (polys = fishnet, items = buildings, no summary field = count) → graduated colours on `COUNT` → layout → PNG + PDF. Cancel during the download was not tried.
Findings:
- T10-1 The Overture download sat at "5 % — Connecting to Overture's data…" for ~8 minutes with no change (the engine was busy: 66 open connections, 22 % CPU). It finished, but the user cannot tell working from hung; a count of files/bytes read or "still working, 3 min" would help. Pinned release 2026-10-21.0 exists on S3, so it was not a missing release.
- T10-2 Summarize Within has no "Count" statistic; counting is "leave the field empty" with Statistic left at Sum (non-obvious). A "Count of features" choice would be clearer.
- T10-3 Legend classes for an integer count are "< 0 / 0 – 1 / 1 – 3 / 3 – 5 / ≥ 5" (a "< 0" class for counts), and "≥" is still a missing glyph (T5-3).
- T10-4 Graduated colors again defaulted to the ID field (CELL_ID).
- T10-5 The fishnet is built in the metric CRS and shown in a WGS 84 frame, so its rows tilt against the page; the same CRS-mismatch warning as T4-5 appears only for scale, not for visible distortion.
- T10-6 Good: Create Fishnet and Summarize Within ran in 0.1–1 s; the map and the Overture credit line ("Overture Maps Foundation, Overture Maps (2026)") are right.
Files: test-projects/10-seattle-buildings/map.png, map.pdf.

## 11. Schools near highways, Davidson County (TN) — DONE (with findings)
Flow: zoom to Nashville (1,080 km² view), National Map → Schools + Highways (179 schools, 2,749 highway segments, ~10 s) → select schools → Model: Project (EPSG:32616) → Join Attributes by Nearest (near = highways layer, radius 2 km) → run 0.2 s → Export Table (CSV, WGS 84 lon/lat, writes `schools_near_highways.csv.crs.txt`) → graduated colours on DISTANCE → layout → PNG + PDF. Buffer / Intersect / Select by Location were not used (the nearest join covers the question).
Findings:
- T11-1 The join's `DISTANCE` column has no unit in its name; it follows the `units` setting (km here), so the legend reads "0.20517 … 0.952732" and the title I wrote said metres — a reader cannot tell. Name it `DISTANCE_KM` or put the unit in the field description.
- T11-2 The "National Map" download is for the current map view, not for a county: 179 schools cover parts of Williamson, Cheatham and Rutherford counties too. There is no "area = this layer/county" choice, so a county study needs a manual clip with a TIGER county.
- T11-3 Join columns are copied as `CATEGORY_1`, `name_1`, `loaddate_1` … (50+ columns); a "keep only these fields" choice would make the CSV usable.
- T11-4 Good: Export Table makes the user choose the coordinate system (no guessing), writes a `.crs.txt` note, and the output file is a typed path field (works in the browser, unlike other dialogs).
- T11-5 Legend class labels have six decimals again (T5-3) and ≥ is still a missing glyph.
Files: test-projects/11-davidson-schools/map.png, map.pdf, schools_near_highways.csv(+.crs.txt).

## 12. Vegetation index, Boulder (CO) — DONE (with findings; map image not reviewed closely)
Flow: zoom to Boulder (~25 × 8 km view), Data → Imagery & DEM → Sentinel-2 → Search → pick scene 2026-08-20 (0 % cloud) → download True colour, Red band, Near infrared (three downloads, ~10 s each) → select the NIR layer → Model: add the Red layer as a second input → Raster Calculator (A and B) `(A - B) / (A + B)` → Unsupervised Classification (K-means, 5 classes) → Output → run 0.3 s each → layout → PNG + PDF.
Findings:
- T12-1 **Bug**: the dialog's own example date range ("2024-06-01/2024-09-30", also the placeholder) is rejected by the STAC service: "refused the search (HTTP 400): datetime value is invalid, does not match RFC3339 format". The UI accepts only YYYY-MM-DD/YYYY-MM-DD, then sends it unchanged; it needs `T00:00:00Z` / `T23:59:59Z` added. With the field empty the search works (newest first).
- T12-2 Choosing the scene after choosing the Image resets the Image to True colour (the first download came back RGB although "Red band" was shown); choose scene first, then image.
- T12-3 Each band is a separate download and a separate layer; NDVI therefore needs two downloads and a two-input calculator. A one-click "NDVI" image choice (or "Red + NIR") would save the user from building the tree. The RGB layer and the single-band layers also come back on slightly different grids (2026 × 843 vs 2028 × 853 cells): Raster Calculator says "the rasters are not on the same grid …" (clear message) when the wrong pair is used.
- T12-4 The Sentinel-2 tile (13TDE) covers only the eastern part of the view; the dialog does not say the scene covers part of the area (a coverage % would help), and mosaicking two tiles is not offered.
- T12-5 A 5-class K-means result has no class names/legend labels, and it is drawn with the elevation renderer; the user must pick a render type by hand.
Files: test-projects/12-boulder-ndvi/map.png, map.pdf.

## 13. Field grid, Great Smoky Mountains (MGRS/USNG) — DONE (with one bug; map image not reviewed closely)
Flow: zoom to the Smokies (24 × 9 km view), National Map → Federal lands + Summits/ridges/valleys (4 polygons, 154 points) → Add USNG / MGRS Field → Export Table ("X/Y or lon/lat and an MGRS reference", WGS 84, 100 m) → MGRS Grid (1 km squares, clipped to federal lands, 3,114 squares, 0.9 s) → layout → PNG + PDF → Add Data re-imports the CSV (auto-detected as 154 points) → USNG / MGRS to Points (centre of square) → Export Table → compare with the originals.
Hand checks: Welch Ridge (35.50742 N, 83.62495 W) = UTM 17 E 261,929 N 3,932,485 → 17SKV 619 324 ✔ and Silers Bald → 17SKV 674 388 ✔ (own TM computation); round trip of all 154 points through 100 m MGRS and back: max 60.5 m, mean 45.8 m, below the 70.7 m bound for a square centre ✔.
Findings:
- T13-1 **Bug**: Add USNG / MGRS Field on the summits layer failed with "Error: ST_X only supports POINT geometries": the layer reports geometry type Point but stores MULTIPOINT; `usng_tools.rs:57` treats dimension 0 as "plain point" and passes `t.geom` straight to ST_X. Use `ST_PointOnSurface`/`ST_Centroid` when the geometry is not a single POINT (inserting Feature To Point first is the workaround, which worked). The raw DuckDB error is not a sentence a user can act on.
- T13-2 Good: Export Table offers MGRS only, USNG only, or X/Y plus MGRS/USNG; precision is a list; the note "The UTM zone is chosen automatically for each point…" is clear; the `.crs.txt` sidecar is written.
- T13-3 The exported CSV carries two MGRS columns when the tree already added one (`MGRS` 1 km from the tool, `MGRS_` 100 m from the export) — fine but the second name (`MGRS_`) is cryptic; a name clash gets an underscore.
- T13-4 The National Map "federal lands" query returns whole-polygon extents, so the 1 km grid had 3,114 squares over areas far larger than the 24 × 9 km view; "Only squares touching features" is the default, but clipping the grid to the view/extent is not offered.
- T13-5 Add Data on a CSV with lon/lat columns turned it into a point layer without asking which columns or which CRS (assumed WGS 84); CLAUDE.md says CRS is never guessed — a prompt or a "assumed WGS 84 (lon/lat)" note would be needed.
Files: test-projects/13-smokies-mgrs/map.png, map.pdf, summits_mgrs.csv(+.crs.txt), roundtrip_points.csv.

## 14. Projection and datum study — PARTLY DONE (Mollweide poster; no second projection, no define-projection/Project steps)
Flow: Map tab → World Outline, Graticule, Tissot Circles (264 circles) → Projection Builder → World: Mollweide, central meridian −96° → "Use as map CRS" → Tabloid landscape layout (frame takes the map CRS) → scale 1:92,000,000 → PNG (120 dpi) + PDF. Also opened Datum Grids ("398 of 402 installed", included grids listed with size and source) and Limits & Responsible Use (readable text on grids, vertical datums, strict mode) — nothing to download.
Findings:
- T14-1 After "Use as map CRS", the status line showed: "Tissot circles (EPSG:4326) can't be drawn in Mollweide (central meridian −96°): Invalid Input Error: TopologyException: side location conflict at -151.39…, -78.20… This can occur if the input geometry is invalid.: Error code 1: Unknown error code" (screenshot `problem-tissot-mollweide.jpg`). A raw GEOS error for polygons that cross the new map's cut meridian (antimeridian of lon_0 = −96 is 84°E); the layer then draws anyway in the layout (so the message may be about the map view only). The splitter should handle circles that cross the cut or say which circles were dropped.
- T14-2 The first layout preview in the new CRS took about 1½–2 minutes ("Rendering…", blank page, devserver at 2.8 GB RSS, `render_layout_preview` pending); after the scale was set to 1:92,000,000 re-renders took under 40 s and export ~30 s. A world layout with a basemap and 264 polygons should not need minutes.
- T14-3 The frame's coordinate system is shown as the raw PROJ string `+proj=moll +lon_0=-96 +x_0=0 …` instead of a name; the map's status bar has the friendly "Mollweide (central meridian −96°)".
- T14-4 Poster cosmetics: the 0.5 world-outline fill plus the semi-transparent Tissot fill muddy the basemap; the scale bar labels overlap ("0 5001000150000 km", same as T8-5); no warning that a single scale bar is meaningless on a world map.
- T14-5 Good: the Projection Builder gives a plain-language description for each projection, shows the PROJ string live and has a "Update the map as I move the sliders" toggle; Datum Grids shows what is installed.
- T14-6 Not done: a side-by-side Mercator vs Mollweide poster (needs two frames with different CRS and positions) and Define Projection / Project steps on a layer with a wrong CRS; they are covered by project 9 (Project) and the CRS matrix tests.
Files: test-projects/14-projection-study/map.png, map.pdf, problem-tissot-mollweide.jpg.

## 15. Atlas of national parks, Utah — DONE (with findings)
Flow: zoom to southern Utah, National Map → Federal lands (154 features) → definition query `name LIKE '%National Park%'` (7 features) → Letter landscape layout with title `{page_name}: National Park atlas, Utah` → Layout → Map Series… (index layer, page name `name`) → select frame, tick "Follows the map series" → Export Series → One PDF (7 pages, 12 MB, ~40 s) and One PNG per page. Page title tokens, per-page extent fit and the definition query all worked first time.
Findings:
- T15-1 The Map Series dialog defaults the page-name field and the order to the first column (OGC_FID); `name` would be the sensible default (or the first text column).
- T15-2 The series includes parks outside Utah (Great Basin NV, Mesa Verde CO) because the download is by map view and the query is by name; fine, but it shows again there is no "clip to state" step in a first-run flow (T11-2).
- T15-3 Every page prints a "geographic coordinates, scale varies" warning (7 warnings) because the frame is WGS 84; a series should offer a projected CRS (e.g. UTM 12N) in the setup dialog.
- T15-4 The park polygon covers the park name/other labels on the basemap at 75 % opacity; the legend shows only "Federal lands (parks, f…" and is a large empty box (same as T5-5).
- T15-5 The scale bar overlaps the left frame edge text ("on") on page 7; elements sit on the frame (T4-2).
Files: test-projects/15-utah-parks-atlas/atlas.pdf, pages/*.png (7).

## 16. A3 poster with every element — DONE (elements inserted; not arranged by hand)
Flow: TIGER Utah counties (29) → A3 landscape layout → Elements tab: Map Frame, Overview Map, Legend, Scale Bar, North Arrow, Declination, Text, Table, Chart, Sources, Rectangle, Ellipse, Triangle, Line, Arrow, Picture (path asked through a prompt in browser mode) → PNG (100 dpi) + PDF. Every element type inserted without an error. Dragging, guides, snapping, grouping and aligning were NOT exercised (pointer drags are unreliable in this harness; the Items pane has Group/Ungroup, Lock and order arrows).
Findings:
- T16-1 Every element is dropped at a default spot on top of the frame; with 16 items the poster is a pile (legend over the overview, scale bar under the table, shapes in the middle). An "Arrange / tidy" command or per-element default placement in free margin space would help; the Items pane would need multi-select align/distribute to fix it by hand.
- T16-2 The Chart element draws nothing until a category field is chosen ("choose the field that makes the categories (Properties › Category field)") — the warning is good but the chart is invisible meanwhile; the Table says "shows 10 of 29 rows; enlarge it or raise the row limit".
- T16-3 Two map frames (f1, f2) and two Sources items are created (Overview Map adds its own frame and Sources), so a "Sources" item appears twice on the page.
- T16-4 Warnings list is long and crowds the layout (frame scale-varies ×2, table rows, chart field); they are readable but could be collapsed to one line with a count.
- T16-5 The Text default is the layout name ("Layout 1"); the Declination diagram text reads "2026 MAGNETIC NORTH DECLINATION 10.4° E (184 mils) at centre of sheet annual change 5′ W" and overlaps the Amarillo label (needs a background).
Files: test-projects/16-a3-poster-all-elements/map.png, map.pdf.

## 17. Federal land overlap with counties, Idaho — DONE (clip, union, erase not run)
Flow: zoom to central Idaho, National Map federal lands (204) + TIGER Idaho counties (44) → Model on the federal layer: Repair Geometry → Intersect (A = repaired, B = counties input) → Calculate Geometry (AREA_KM2, sq km) → run 10.4 s → Summary Statistics (field AREA_KM2, case = ownerormanagingagency) → Export Features CSV (5 owner groups) → back to the intersect map → layout → PNG + PDF. Dissolve, Clip, Union, Erase exist but were not used.
Findings:
- T17-1 `ownerormanagingagency` is a code (3, 5, 10, 11, 13) with no name; the summary table is unreadable without the data dictionary (USFS, BLM, … ). A lookup/domain table or a decoded column is needed for the federal-lands layer.
- T17-2 Summary Statistics output columns are `COUNT, SUM, MEAN, MIN, MAX, STD` in upper case with no units (SUM is sq km, here 148,647 for code 11); the unit should be in the name or a header note.
- T17-3 Several intersect pieces have tiny areas (min 1.2e-7 km²): slivers from boundary mismatch between the two sources; Intersect does not report or offer a sliver filter.
- T17-4 Good: Repair Geometry first avoided invalid-polygon errors; the whole chain ran in 10 s on 204 polygons and the red-step/readable error pattern was not needed.
- T17-5 Because the download follows the map view, the federal layer includes Oregon and Wyoming polygons; the intersect with Idaho counties trims them correctly, but the original (un-intersected) pieces are still counted in "204 features".
Files: test-projects/17-idaho-federal-land/map.png, map.pdf, area_by_owner.csv.

## 18. Place-name geometry, New England — DONE (labels not tried; map image not reviewed closely)
Flow: zoom to the Vermont/Massachusetts area (~280 × 110 km), National Map → Cities, towns and populated places (1,776 points) → Model: Project (EPSG:32618) → Delaunay (3,973 features), Concave Hull (1), Points to Path, Voronoi (2,008) each run from the same projected input into Output → layout of the Voronoi → PNG + PDF.
Findings:
- T18-1 Voronoi returned 2,008 polygons for 1,776 input points — more cells than points (duplicate place names at the same location, or cells split into parts); a Voronoi of N distinct points has N cells, so the extra 232 are either duplicate-point cells or multipart explosion; the tool does not say which.
- T18-2 Points to Path shows a red error until "Order by field" is chosen (it has no default); good that it is required, but the field list offers `OGC_FID`, `gaz_id` etc. and no hint what ordering means for places (a name or a time field would be needed).
- T18-3 Feature chain ran 4 variants in ~1 minute for 1,776 points; each was an edit of one node, not a new tree (tree editing is quick once nodes exist).
- T18-4 The 1,776 place points come from the National Map "Cities, towns and populated places"; the layer has `gaz_name`/`county_name` but no population, so a Voronoi of towns cannot be shaded by size.
- T18-5 Labels (Symbology → Label features) were not exercised in this project.
Files: test-projects/18-new-england-places/map.png, map.pdf.

## 19. Elevation from points, Mount St. Helens (WA) — PARTLY DONE (interpolation blocked by a bug; no difference raster)
Flow: zoom to the volcano (~21 × 7 km), 3DEP download (1.1 M cells) + National Map summits (26 points) → Model on the summits: Project (EPSG:32610) → Sample Raster Values (raster = the DEM layer as a second input, field ELEV) → 26 rows, 17 with a value → Interpolate (IDW, 30 m) and Kriging (auto) → layout of DEM + sampled points → PNG + PDF; sampled table exported as `summits_sampled.csv`.
Findings:
- T19-1 **Bug (reproduced twice)**: Interpolate Points to Raster fails with "There are no points with a number in the value field to interpolate from" although the upstream Sample Raster step produced 17 numeric ELEV values (confirmed in the exported CSV: 2114.84, 2315.37, 1360.26 …). Likely the field is read as text/typed unknown after a node, or NULL rows poison the check; Kriging was not run to completion for the same reason (same input). Needs a failing test: points → sample_raster → interpolate.
- T19-2 Sample Raster Values silently leaves NULL for the 9 points outside the DEM (they were from the wider map view) and does not report "9 of 26 outside the raster"; CLAUDE.md asks for dropped features to be reported.
- T19-3 With the results stuck, the "compare with the real DEM" step (difference raster) was not possible; it would also need Resample/Align so the 30 m IDW grid matches the 10 m DEM (Raster Calculator A and B requires the same grid, T12-3).
- T19-4 Good: the 3DEP download and hillshade of the volcano look right (crater, Pumice Plain); Project + Sample Raster ran instantly.
Files: test-projects/19-st-helens-interpolation/map.png, map.pdf, summits_sampled.csv.

## 20. Data cleaning and export, USGS earthquake CSV — DONE (with findings)
Flow: downloaded `2.5_month.csv` (1,977 rows, 378 KB, USGS feed) → Add Data (CSV with latitude/longitude auto-detected as points, EPSG:4326) → definition query `mag >= 4` (1,152 of 1,977) → a deliberately bad query → Calculate Field `MAG_CLASS` (CASE on mag; moderate 943, strong 189, major 20) → Split by Attribute on `magType` (7 layers, 1,018 + 114 + 12 + 5 + 1 + 1 + 1 = 1,152, so the definition query is honoured) → Export Features (GeoPackage, 1,152) and Export Table (CSV with lon/lat WGS 84 + `.crs.txt`) → world layout PNG + PDF. Attribute-table cell editing was not exercised.
Findings:
- T20-1 A wrong definition query shows the raw DuckDB message: "Catalog Error: Scalar Function with name >>> does not exist! Did you mean ">>"?" — correct but not a sentence about the user's query; a "Check the query near '>>>'" wrapper would do.
- T20-2 Add Data on a CSV assumed WGS 84 from column names `latitude`/`longitude` without asking (same as T13-5); the CRS column showed EPSG:4326 afterwards, so it is visible but not confirmed.
- T20-3 Split by Attribute names layers "usgs_2.5_month – mb" etc.; good, and each keeps the source. Its dialog lists the field as a drop-down only; no preview of how many layers will be made (7 here, one layer with 1 feature each for four magTypes).
- T20-4 Good: Export Features and Export Table both honour the definition query and the node result (MAG_CLASS present), and write a CRS note.
- T20-5 The CSV timestamps (`2026-10-31T01:28Z`) are ahead of the machine date (2026-10-30 local) — a time-zone effect, not a bug.
Files: test-projects/20-quake-csv-cleaning/usgs_2.5_month.csv, quakes_clean.gpkg, quakes_clean.csv(+.crs.txt), map.png, map.pdf.

## Decisions from the user (2026-10-30, after the 20 projects)
- Basemap default: **keep USGS Topo** (T4-3 not changed).
- Topo sheets: sheet units, whole numbers, USGS colours (blue water/streams, ice pale blue, trails dashed dark).
- Layout placement: **leave as is** (no auto-arrange).
- Legends: round sensibly, thousands separators, real ≥ glyph, full layer names, box sized to content.
- Download area: add Whole world, This layer's extent, Typed lat/lon box and a place search.
- Sentinel-2: one-click NDVI, scene coverage %, fix the date format.
- Engine errors: wrap all with a plain sentence plus Details.
- Silent results: warn on empty output and dropped/NULL rows.
- Missing tools to build: proportional (size) symbols; unit-labelled output columns. (Not chosen: Count/owner names, auto-insert node/Add Project button.)
- Bugs TP-1…TP-15: fix all in order of severity, one commit each.
- CRS: suggest a projected CRS for the data's region when making a frame/series; user confirms.
- CSV import: ask once "Assume WGS 84?" and record the answer.
