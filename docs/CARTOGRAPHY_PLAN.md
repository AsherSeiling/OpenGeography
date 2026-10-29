# Raster symbology + cartography suite — plan

_Sonnet session, 2026-10-29 (task from the lead session). Builds on `LAYOUTS_PLAN.md` (C1–C3), `LABELS_PLAN.md`. Ideas follow the QGIS raster renderers (Singleband gray, Singleband pseudocolor, Paletted/Unique values, Multiband color, Hillshade) and Print Layout, and ArcGIS Pro symbology (Stretch, Classify, Unique Values, RGB) and Layouts; everything is reimplemented, no QGIS code is copied (GPL)._

## Gap audit (2026-10-29, from the code)

### Raster display today
- `raster::render_extent` (`crates/engine/src/raster.rs`) is the only renderer for the map tiles, projected map view and layouts. One hard-coded 6-stop hypsometric ramp over the layer's full `min..max`, fixed hillshade (az 315°, alt 45°, 0.55+0.6·shade multiply), or raw 8-bit RGB for `kind == "rgb"`. NoData → transparent.
- Style is **not** per layer: `LayerStyle` has no raster fields (opacity is forced to 1); tile cache key is `(layer, version, z, x, y, fields)`, so a style needs to join the key.
- Missing: any stretch, ramps, classification, categorical/paletted, band assignment, hillshade options, transparency options, brightness/contrast/gamma/saturation, display resampling choice, histogram, raster legend, style save/load.

### Layouts today
Elements: `map_frame` (graticule, grid, basemap, border, label overrides, series), `text` (tokens, font), `legend` (frame, title, columns), `scale_bar` (units, segments, ratio), `north_arrow` (one `style` string), `sources`. Export PDF/PNG/SVG with dpi, templates, map series, rulers, align/arrange.

Missing (cartographer expectations): picture/logo frame; shapes, lines, arrows; scale bar styles and RF text; north-arrow library, custom SVG and grid-north; legend editing (items, groups, patch shapes, raster colour bars, frames); overview/inset map with extent indicator; rich text; attribute table and chart elements; frame clipping/masks, drop shadows, backgrounds; page margins/bleed and guides + snapping; grouping; layer effects; symbol library (markers, line patterns, fill patterns, SVG import); style presets/themes; world file / GeoPDF.

## Decisions
- One `RasterStyle` definition (Rust `crates/engine/src/raster_style.rs`, mirrored in TS `ui/src/store/store.ts` as `LayerStyle.raster`). Optional + `serde(default)`: old projects load unchanged. The default style reproduces today's look exactly (kept as the "Hypsometric + hillshade" preset), so nothing changes for existing data.
- The style travels with the tile request (compact JSON, hashed into the tile cache key and the URL) and with layout render requests (like `LayerStyle` does for vectors). The engine never stores it separately.
- Ramps are a table in the engine (`raster_style::ramps()`), served to the UI by a command (one catalog, no parallel UI list).
- Stats for stretches (percentiles, std dev, histogram) come from one cached pass per raster (`raster_stats`), cancellable via `progress::Task` on big files.
- Layout elements stay in the one `Element` enum (serde tag `kind`) and one TS mirror; each new element is additive. No UI style change (ribbon + panes).

## Task breakdown (each its own branch `claude/<task>`, merged `--no-ff`, `[skip ci]`)

### Raster symbology (RS)
1. **rs-core** — `RasterStyle` + renderer: singleband gray, pseudocolour with ramp library (viridis family, terrain, diverging, sequential, qualitative; custom stops with opacity, invert), stretch modes (min-max, percentile, std dev, manual), discrete/interpolated/exact, global opacity, nodata + value-range transparency, brightness/contrast/gamma/saturation. Tile key includes the style. Engine tests.
2. **rs-classify** — classification (equal interval, quantile, Jenks, std dev, manual breaks) over `raster_stats`; histogram data command.
3. **rs-ui** — Symbology pane for rasters (renderer picker, ramp picker, histogram widget with draggable min/max, class table), wired to the map and Apply.
4. **rs-categorical** — paletted/unique values (colour table, RAT or user labels).
5. **rs-multiband** — band assignment, per-band stretch, true/false-colour presets.
6. **rs-shading** — hillshade/multidirectional blended under a ramp, slope/aspect shading, contour overlay, display resampling (nearest/bilinear/cubic).
7. **rs-legend-io** — raster legend (colour bar, class swatches) in layouts; save/load style files and project-level presets.

### Cartography (CG)
1. **cg-picture-shapes** — picture/logo frame (PNG/JPEG/SVG), rectangle/ellipse/line/arrow shapes, fills, strokes, background/frame/shadow on every element.
2. **cg-northarrow-scalebar** — north-arrow library + custom SVG + true/grid north; scale-bar styles (alternating, line, ticks, stepped, double), units, RF text.
3. **cg-legend** — editable legend items, groups, columns, patch shapes, frame/title style, raster bars.
4. **cg-overview** — overview/inset frame with extent indicator, linked to another frame.
5. **cg-text-table-chart** — rich text with more variables; attribute table element; bar/pie chart element.
6. **cg-page-guides** — margins/bleed, guides, snapping, grouping, align/distribute extensions, layer panel.
7. **cg-frame-clip** — frame clipping to a polygon/mask, layer effects.
8. **cg-symbols** — symbol library (markers, line patterns, fill patterns, SVG import) usable by layer styles and layouts.
9. **cg-themes** — style presets (topographic, thematic, minimalist).
10. **cg-export** — world file, embedded font notes, CMYK-ish notes, GeoPDF (if feasible; with the C4 plan).

Order: RS 1→2→3 first (everything else in RS builds on them), then CG 1, 2, 3 in parallel-safe order, then the rest. Cases for each task go into `docs/TEST_PLAN.md` (sections RS and CG), decisions into `docs/DECISIONS.md`.

## Status
- [x] RS1 rs-core (2026-10-29): engine `raster_style.rs`, tile/layout wiring, `raster_ramps`/`raster_histogram`, TS types. UI comes in RS3.
- [x] RS2 rs-classify (2026-10-29): equal/quantile/Jenks/std-dev breaks, classed rendering (`breaks`), `raster_classify`, colour-blind-safe flag on ramps.
- [x] RS3 rs-ui (2026-10-29): `ui/src/raster/RasterSymbology.tsx` in Properties › Symbology for raster layers; histogram widget with draggable handles.
- [x] RS4 rs-categorical (2026-10-29): paletted renderer, `raster_unique_values`, colour-table colours. RAT labels not built (gdal crate has no RAT API).
- [x] RS5 rs-multiband (2026-10-29): multiband renderer, band presets (stretch per channel from each band's statistics; per-channel manual stretch UI not built).
- [x] RS6 rs-shading (2026-10-29): hillshade / multidirectional / slope shading under a ramp, display resampling. Contour overlay stays the Contour tool + Symbology node (decision: no duplicate in the raster renderer).
- [ ] RS7 …
