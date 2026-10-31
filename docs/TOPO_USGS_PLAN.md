# Classic USGS topographic maps and a military installation preset — plan

_Sonnet session, 2026-10-30. User request: an old-school USGS style map maker based on the USGS "Topographic Map Symbols" sheet (`topomapsymbols.pdf`), and a military map preset based on the Fort Campbell Military Installation Map (DMA series V741S, 1:50,000). Builds on the Topo Map Builder (LW9, `docs/LAYOUT_REWORK_PLAN.md`), the layout symbol library (CG8) and themes (CG9). Sources of the artwork: the USGS sheet and the DMA map are works of the U.S. Government (public domain); symbols are redrawn here as plain geometry, nothing is copied as an image._

## What exists, what is missing
- Exists: Topo Map Builder → a **land-navigation** sheet (UTM grid, ticks, declination diagram, scale bars, grid-reference box, legend), 3DEP + contours + hillshade + National Map data, 4 presets.
- Missing for the USGS look: layered line symbols (road casings, railroad ties, double-line light-duty roads), the USGS symbol catalogue (colours, boundaries, hydrography, vegetation tints, built-up areas, PLSS lines, control points), the classic quadrangle sheet (neat line, title block, quad name corners, scale bars in ft/mi/km/m, contour-interval and datum notes, road classification legend, declination diagram, adjoining quads).
- Missing for the military look: the overlay classes (range boundary, impact area, drop zone, training area, landing zone, cantonment, restricted-area and air-traffic-control boundaries, firing and observation points), and the marginalia (military legend, range facilities list, mils declination, grid reference box, conversion note).

## Decisions (no UI-style change; all inside the existing Topo Map dialog and Symbology pane)
1. **Symbol stacks, not special cases.** `LayerStyle.strokes` (and `StyleClass.strokes`) draw a line or outline as several strokes bottom to top (casing, fill, ticks via a short thick dash). The USGS and military looks are *data* on top of that general feature, so any user can build their own roads, railways and boundaries.
2. **One catalogue, in the engine.** `topo_symbols.rs` holds every symbol (id, category as printed on the USGS sheet, geometry, recipe, classic and modern colours). The UI reads it with `topo_symbols` and never keeps its own list. Previews are drawn by the engine (`topo_symbol_svg`).
3. **"Old school" = the classic look**: red-filled primary roads with black casing, pink built-up-area tint, green woodland tint, brown contours, blue water. A switch picks the modern variant (grey built-up tint, thinner roads) from the same sheet.
4. **Sheets as builder styles.** `TopoSpec.style` = `landnav` (as now) | `usgs` | `military`. Each is a layout generator in `layout/topo.rs`; presets in the dialog pick scale, paper and symbols together.
5. **Auto-assign by layer.** One button applies symbols to the project's layers by dataset id/name (National Map flowlines, water bodies, roads, trails, names, contours; military overlay layers by keyword). Anything not recognised is left alone and listed.

## Tasks (each its own branch, merged `--no-ff`, `[skip ci]`)
- **T1 topo-strokes** — stroke stacks in the layout renderer, legend swatches and the map view; fixtures and tests.
- **T2 topo-symbols** — symbol catalogue (≈ 75 USGS symbols + 12 military), new fill patterns (marsh, orchard, vineyard, scrub, sand, gravel), new markers, previews, Symbology "Topo symbol…" picker, legend entries by symbol id.
- **T3 topo-usgs** — classic quadrangle sheet generator and dialog style; auto-assign button.
- **T4 topo-military** — military installation sheet and overlay symbols; preset.
- **T5 topo-protocol** — pixel tests of every symbol class, showcase pages for both looks, docs, OPEN_SOURCE rows, TEST_PLAN cases.

## Not in scope (decide later)
Purple revision overlays, provisional-edition hand lettering, bathymetric and glacier symbols beyond the catalogue entries, automatic label placement in the USGS typefaces (the bundled Noto Sans is used), GeoPDF.
