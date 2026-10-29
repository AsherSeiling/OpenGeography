# Test protocol — raster symbology and cartography suite

Covers `docs/CARTOGRAPHY_PLAN.md` (RS1–RS7, CG1–CG10). Case IDs match `docs/TEST_PLAN.md` (sections RS and CG); this file says **how each is proven**, which cases need a human, and what the last run showed.

Last executed: 2026-10-29, macOS arm64, headless Chromium, local build (no CI). Environment: GDAL/PROJ as bundled by the project, bundled Noto Sans.

## 1. Layers of proof

| Layer | What it proves | Where | Run |
|---|---|---|---|
| **L1 Engine unit** | colour maths, ramps, classification, geometry, SVG pieces | `crates/engine/src/{raster_style,layout/*}.rs` tests | `cargo test -p engine --lib` |
| **L2 Engine integration** | real rasters and layers through the renderer, PNG/PDF export, model validation, file I/O | `crates/engine/tests/{raster_style,layout,layout_shapes}.rs` | `cargo test -p engine` |
| **L3 UI logic** | snapping, distribute, group, overrides, themes, histogram maths, page-setup parsing | `ui/src/**/*.test.ts` | `npx vitest run` |
| **L4 UI behaviour (e2e)** | the panes and ribbon actually edit the stored style/layout; the engine accepts it | `e2e/raster-symbology.spec.ts`, `e2e/layout-graphics.spec.ts` | `scripts/e2e.sh <out> <spec>` |
| **L5 Pixel protocol (e2e)** | the colours that come out: tile PNGs of real GeoTIFF/ASCII rasters through the real tile URL, and layout previews of real layers | `e2e/raster-pixels.spec.ts`, `e2e/cartography-pixels.spec.ts` (helpers `e2e/pixels.ts`) | `scripts/e2e.sh <out> e2e/raster-pixels.spec.ts e2e/cartography-pixels.spec.ts` |
| **L6 Visual** | a person (or Claude reading the PNG) looks at a showcase page | `cartography-showcase.png` written by CP-5 into the e2e artifacts folder | open the file |
| **L7 Manual (packaged app)** | native file dialogs, real-screen look, other OSes | §5 checklist | by hand |

Run everything: `cargo test -p engine && npx tsc -p tsconfig.json && npx vitest run && scripts/e2e.sh out/`.

## 2. Section A — raster symbology

| Case | Objective | Proof (layer → test) | Pass criterion |
|---|---|---|---|
| RS1-1 | Ramp library is sound | L1 `ramps_parse_and_are_unique` | ≥ 25 ramps, unique names, all five groups present, every colour parses |
| RS1-2 | Ramp mechanics | L1 `linear_ramp_*`, `invert_*`, `discrete_*`, `custom_stops_*` · L5 RP-1 | viridis left/right pixels are purple/yellow; inverted swaps them; discrete makes ≤ 5 colours, continuous > 30 |
| RS1-3 | Transparency | L1 `transparency_rules` · L5 RP-2 | value range 45–55 → alpha 0 in the tile; opacity 0.5 → alpha ≈ 128; outside the raster alpha 0 |
| RS1-4 | Brightness/contrast/gamma/saturation | L1 `adjustments` · L5 RP-2 | brightness lifts black; gamma 2 lightens the mid-tone; saturation −1 gives R≈G≈B |
| RS1-5 | Stretch modes on a real raster | L2 `stretch_modes_set_the_range`, `band_choice_*` · L5 RP-2 | manual 40–60 clips left black / right white; percentile ≥ min-max at the top |
| RS1-6 | Default look unchanged | L2 `discrete_mode_and_default_unchanged` (byte-identical) + all pre-existing layout golden tests | no golden changed |
| RS1-7 | Style token TS↔Rust | L3 `style.test.ts` · L1 `token_round_trip` · L5 (every RP test uses the token over HTTP) | tiles render for the token the UI would send |
| RS2-1/2/3 | Classification, classed rendering, colour-blind flag | L1 `classification_methods`, `classed_rendering_*`, `colour_blind_safe_flags` · L5 RP-1 | breaks as expected; one colour per class |
| RS3-1 | Symbology pane | L4 `raster-symbology.spec.ts` (render type, ramp picker with ✓ marks, histogram handle drag → manual stretch, classified breaks, paletted, multiband, shading, resampling, style files via API) · L3 `histogram.test.ts` | stored style matches the clicks; errors appear as text |
| RS4-1 | Unique values | L1 `paletted_*` · L2 `unique_values_and_paletted_rendering` · L5 RP-3 | counts exact (2160/2080/2160); hidden class transparent; no blended edge colours (nearest) |
| RS5-1 | Multiband | L1 `multiband_*` · L2 `multiband_composite_uses_chosen_bands` · L5 RP-3 | bands [3,2,1] swap red/blue; per-channel stretch; band 9 refused with "Band 9 …" |
| RS6-1 | Relief shading, resampling | L1 `shading_*` · L2 `shading_and_resampling_*` · L5 RP-4 | flat ground unchanged, slope changed, slope mode darker |
| RS7-1 | Raster legend in layouts | L1 `legend_content` · L2 `rs7_raster_legend_*` · L5 CP-3 (viridis bar present, classes, unique values) | bar of 32 stops with min/max; class labels |
| RS7-2 | Style files | L1 `style_files_*` · L4 (API round trip, refusal text) · **L7 dialogs** | round trip equal; foreign JSON refused with words |
| RS-1 | Imagery recognised as RGB | L2 `three_band_bytes_are_imagery_*` · L5 RP-3 | a 3-band Byte GeoTIFF draws as its colours, not as a DEM |

## 3. Section B — cartography

| Case | Objective | Proof | Pass criterion |
|---|---|---|---|
| CG1-1 | Shapes | L1 `each_shape_*` · L2 `shapes_pictures_and_decor_*`, `every_line_style_of_every_shape_*` · L5 CP-1 | rectangle red at centre / white outside; ellipse and triangle corners empty; line has ink; PNG and PDF export for all shapes × solid/dashed/dotted |
| CG1-2 | Pictures | L1 `read_picture_*`, `pictures_are_validated_*` · L5 CP-1 | stretched picture shows its colour; opacity 0.5 blends to ≈ 150; bad/foreign sources refused in words · **L7 file dialog** |
| CG1-3 | Background/frame/shadow | L1 `decor_*` · L5 CP-1 | background colour at a point; grey shadow band beyond the box, white further out |
| CG2-1 | North arrows | L2 `cg2_every_north_arrow_style_*`, `cg2_custom_*` · L5 CP-3 (7 styles ≥ 15 ink pixels in the chosen colour) | each style draws, rose has N/E/S/W, grid vs true north angles |
| CG2-2 | Scale bars | L2 `cg2_scale_bar_styles_*`, `cg2_verbal_*` · L5 CP-3 | alternating > hollow, line; double > alternating; ground length unchanged (P11) |
| CG3-1 | Legend editing | L2 `cg3_legend_entries_*` · L3 `legendRows.test.ts` · L4 | rename/hide/extra entry persist; swatch shape; hidden layer hides its block |
| CG4-1 | Overview map | L2 `cg4_overview_*`, `cg4_indicator_is_a_rectangle_*` · L5 CP-2 | rectangle is 1.2 × 0.96 mm for the 100 × 80 mm, 1:24,000 over 1:2,000,000 case; outline not a blob; none without the indicator |
| CG5-1/2 | Table, chart | L2 `cg5_*` · L5 CP-3 | header grey `#e6e6e6`, rows by sort; bars `#0072b2`; pie slices Okabe–Ito blue/orange/green; truncation and bad-field warnings |
| CG5-3 | Rich text, tokens | L1 `rich_runs` · L2 `cg5_rich_text_*` · L5 CP-1, CP-5 (title shows layer count and scale) | bold/italic runs; `{layer_count}`, `{layer_names}`, `{author}` |
| CG6-1 | Page setup, bleed, crop marks | L2 `cg6_bleed_*` · L3 `pageSetup.test.ts` · L4 · L5 CP-1 | image grows by 2 × (bleed + 10 mm slug); page colour in the bleed, white slug, mark pixels present |
| CG6-2/3 | Guides, snapping, spread, group, element list | L3 `snap.test.ts`, `arrange.test.ts` · L4 (drag to a guide lands exactly on it) | snapped x = 100; distribute equal gaps; one member selects the group |
| CG7-1 | Frame shapes | L2 `cg7_frames_*` · L5 CP-2 | corners white for ellipse / rounded / polygon outline, centre red; line layer falls back with a message |
| CG7-2 | Layer shadow | L2 `cg7_a_layer_can_cast_*` · L5 CP-2 | opaque black pixels appear only with the effect (> 200) |
| CG8-1 | Symbols | L1 `symbols` tests · L2 `cg8_*` · L5 CP-2 | hatch lines present only with a pattern; star/cross cover < 70 % of a circle; dashed < 85 % and dotted < dashed of solid |
| CG9-1 | Themes | L1 `themes.rs` tests · L3 `themes.test.ts` · L4 · L5 CP-4 | topographic page `#fbf9f2`, border colour; layer palette only on single-symbol layers |
| CG10-1 | World file | L2 `cg10_*` | pixel under the frame's corner has the frame's corner coordinates; rotated frame refused; no sidecars for PDF |
| CG10-2 | Export notes | text in Page Setup; fonts embedded checked by layout test P2 | — |
| CG-1 | Dotted line crash | L2 `every_line_style_of_every_shape_*` · L5 CP-2 | no "SVG could not be read" |

## 4. Defects found while building this protocol (all fixed, all with regression tests)

| ID | What | Found by | Fix |
|---|---|---|---|
| RS-1 | Three-band 8-bit imagery was drawn with the elevation ramp; Properties showed a struct dump as data type (pre-existing, `gdal` crate type-name change) | L5 RP-3 | `raster::describe` uses GDAL's name ("Byte") — `docs/BUGS.md` RS-1 |
| CG-1 | Dotted line style duplicated `stroke-linecap` → the whole page failed to render (shape lines and layer lines) | L5 CP-2 | dashes carry only `stroke-dasharray`, caps added where free — BUGS CG-1 |
| CG-2 | Overview extent rectangle drew as a zig-zag blob (ring built from an unordered point set) | L6 visual check of the showcase page | ordered ring — BUGS CG-2 |

Observations that are **by design**, recorded so they are not mistaken for bugs: a constant multiband channel stretches to black (empty range); a raster legend or vector legend swatch shows colour only, not the marker/pattern (documented in DECISIONS CG8); the map view keeps plain symbols.

## 5. Manual checklist (L7, packaged app; fill in when run)

Run on the packaged build (`scripts/package-macos.sh`) and repeat on Windows and Linux after R7.

| # | Step | Expected | Result |
|---|---|---|---|
| M1 | Add a DEM, open Properties › Symbology, pick *Singleband pseudocolor*, choose viridis | map redraws in viridis; ✓ marks on colour-blind-safe ramps | |
| M2 | Drag the histogram handles | stretch changes live; fields show Min/Max | |
| M3 | Classified colours with Jenks, 5 classes | 5 flat bands; breaks editable | |
| M4 | Add a land-cover raster with a colour table → *Paletted* → Load classes | the file's colours appear | |
| M5 | Add a 3-band image; Multiband presets | image looks natural by default; presets change colours | |
| M6 | *Save style…* then *Load style…* on another raster (native dialogs) | same look; wrong file → readable message | |
| M7 | New layout → Picture → choose a PNG, a JPEG and an SVG (native dialog) | each embeds; Replace works | |
| M8 | Frame, Overview Map, Legend, Scale Bar (double), North Arrow (rose), Table, Chart | all appear; nothing overlaps unreadably at Letter landscape | |
| M9 | Page Setup: bleed 3, crop marks, world file; export PNG | file + `.pgw` + `.prj`; load the PNG in the GIS and check the frame lines up | |
| M10 | Export PDF; open in a viewer | text selectable, fonts embedded, patterns and shadows visible | |
| M11 | Drag an element near a guide; Alt-drag | snaps; Alt turns snapping off | |
| M12 | Apply each theme (page look; look + layer colours) | page and layers restyle; undo (Ctrl+Z) restores the page look | |
| M13 | Colour-vision check of the ramps/charts with a simulator (deuteranopia) | viridis family, Okabe–Ito chart colours stay distinguishable | |
| M14 | Dark theme of the app | new panes readable | |

## 6. Results of the last run (2026-10-29)

| Suite | Result |
|---|---|
| `cargo test -p engine` | **459 passed**, 0 failed, 25 ignored (network, performance and open-bug cases as before) |
| `npx tsc -p tsconfig.json` | clean |
| `npx vitest run` | **429 passed** (52 files) |
| `scripts/e2e.sh` (all specs, headless Chromium) | **77 passed**, 0 failed (~3.6 min); includes the 9 new pixel tests (RP-1…RP-4, CP-1…CP-5) |
| Visual check (L6) | showcase page read as an image: map frame (rounded clip, graticule labels, hatch pattern, raster with hillshade, shadowed polygons, star marker, dashed line), overview with extent rectangle (after fix CG-2), rose arrow, legend with raster colour bar, double scale bar, bar chart, table — all as intended; known limits listed in §4 |
| Manual checklist (L7) | **not run** — needs the packaged app and a person |

## 7. Not covered by this protocol

- Windows/Linux behaviour (fonts list, file dialogs, DPI scaling): R7.
- GeoPDF, CMYK, map-view symbols/patterns, blur effects: not built (see the plan).
- Performance of tables/charts on very large layers (tables read at most 500 rows; charts group in SQL, not measured).
- Accessibility beyond colour: keyboard-only operation of the new panes and screen-reader names (labels exist and are used by the tests, but there was no assistive-technology pass).
