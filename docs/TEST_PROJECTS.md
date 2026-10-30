# Twenty test projects (proposed 2026-10-30; waiting for "execute")

Each project is done **through the UI** (the real screens, driven in the in-app browser against the dev server, the same React app and engine the Mac app runs), with **open data only**, from an empty project to a finished product (a layout exported to PDF, or a table/map saved). While doing them I keep a log of every point where I had to guess, work around something, or found something missing or awkward, and at the end I turn that log into **multiple-choice questions for you** (one form), so nothing is decided without you.

Data used (all free, no account unless noted): USGS 3DEP elevation, USGS The National Map (hydrography, transportation, structures, GNIS names, federal lands), Census TIGER/Line, Overture Maps, USGS earthquake catalog, Sentinel-2 via the public STAC, PROJ datum grids.

| # | Project | Open data | What it exercises | Finished product |
|---|---|---|---|---|
| 1 | **Land-nav quad, Pikes Peak (CO), 1:24,000 Tabloid** | 3DEP, National Map roads/trails/water/names | Topo Map Builder end to end: sheet, elevation, contours, hillshade, features, declination, MGRS grid, coordinate scale, adjoining sheets, self-check | PDF + a bearing and grid reading checked by hand |
| 2 | **Land-nav 1:50,000 Letter (metric), Mount Rainier (WA)** | 3DEP, National Map | 1:50,000 preset, metric units, 100 m contours, zone 10 | PDF |
| 3 | **Trail map, Zion Canyon (UT), Letter portrait** | 3DEP, National Map trails and 4WD roads | Trail preset, portrait layout, narrow-page margins, labels on trails | PDF |
| 4 | **County population choropleth, Tennessee** | TIGER counties | Thematic map (acceptance map 2): classes, legend, overview map, scale bar, north arrow, sources line, title block | PDF |
| 5 | **Census tract density map, Hennepin County (MN)** | TIGER tracts | Field calculation (density), graduated colours, Albers/UTM choice, geodesic area | PDF |
| 6 | **Small drainage basin, Gunnison River headwaters (CO)** | 3DEP | Hydrology tools: fill, flow direction/accumulation, streams, watersheds; node-tree editing, Shift+A, right-click | Basin map with streams |
| 7 | **Terrain classes, Grand Canyon (AZ)** | 3DEP | Slope, aspect, ruggedness, reclassify, raster calculator, raster symbology and legend | Terrain map PDF |
| 8 | **World earthquakes, last 30 days** | USGS earthquake catalog | World view, projection builder, graticule/outline, graduated symbols by magnitude, Tissot check | Poster PDF |
| 9 | **Earthquake clusters, California** | USGS earthquakes | Hot spots, Moran's I, spatial statistics, symbology on results | Map + statistics table |
| 10 | **Building density, downtown Seattle** | Overture buildings | Fishnet, summarize within, join, graduated map, large-layer speed and cancel | PDF |
| 11 | **Schools near highways, Davidson County (TN)** | National Map structures + roads | Buffer, intersect, near, join nearest, select by location, geodesic units | Table of schools + map |
| 12 | **Vegetation index, Boulder (CO)** | Sentinel-2 via STAC | Imagery & DEM dialog, NDVI, k-means classes, raster symbology | Map PDF |
| 13 | **Field grid, Great Smoky Mountains NP** | National Map, TIGER | MGRS/USNG: Add MGRS Field, MGRS Grid, USNG/MGRS to Points, Export Table with MGRS, polar-free round trip | CSV with MGRS + map |
| 14 | **Projection and datum study** | PROJ datum grids | Datum Grids menu, Tissot circles, responsible-use text, CRS warnings, define projection, Project tool | Comparison poster PDF |
| 15 | **Atlas of national parks, Utah** | National Map federal lands | Map series with an index layer, page names, follow-the-series frames, series PDF | Multi-page PDF |
| 16 | **A3 poster with every element** | TIGER + National Map | Layout workspace: Items catalog, text styles and sizes, picture, table, chart, shapes, guides, snapping, group, align | A3 PDF |
| 17 | **Federal land overlap with counties, Idaho** | TIGER, National Map federal lands | Clip, intersect, union, dissolve, erase, repair, summary statistics, area by owner | Map + table |
| 18 | **Place-name geometry, New England** | GNIS places | Voronoi, Delaunay, concave hull, points to path, feature to point, labels | Map PDF |
| 19 | **Elevation from points, Mount St. Helens (WA)** | 3DEP samples + GNIS summits | Sample raster, interpolate (IDW), kriging, contours from the surface, compare with the real DEM | Map + difference raster |
| 20 | **Data cleaning and export, USGS earthquake CSV** | USGS earthquakes CSV | Import CSV, define projection, attribute table edits, definition query, calculate field, split by attribute, Export Features and Export Table | Clean GeoPackage + CSV |

## How I will run them

1. For each project: a fresh project in the UI; do everything through ribbon, panes, dialogs and nodes (no scripts, no direct engine calls), screenshot the result, export what it ends in.
2. Log (in `docs/TEST_PROJECT_LOG.md`) for each: what worked, anything that errored, anything confusing or slow, anything I had to decide myself, and each bug found (also added to `docs/BUGS.md`).
3. At the end, one **decision form**: every open choice as a multiple-choice question with my recommended answer first, grouped by area (cartography, data sources, defaults, missing tools), so you can answer in one pass.
4. Things I will not do without asking: use an API key or account, download more than a few hundred MB in total, publish anything outside this repository.

## Open questions before I start

- Network: the 3DEP, National Map, STAC and Overture steps need internet from this Mac; if any server is down I will mark that project "blocked by source" rather than skip silently.
- Order: I will go in the table order unless you say otherwise.
