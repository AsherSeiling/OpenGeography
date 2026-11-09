# Built-in colour picker, transparency and outline opacity

Written 2026-11-09 from the user's request: "a different color picker that is native to the software that allows transparency" and "opacity as an outline option". Planned, not built. Packages CP1–CP3; order in `docs/MASTER_PLAN.md`.

## What exists today

- 26 `<input type="color">` controls in 11 components. Each opens the operating system's own colour panel, which looks different on each platform and has no transparency. The value is always `#rrggbb`.
- A polygon's outline can't be made transparent. `style.opacity` is applied to the fill only: `line-opacity` is forced to 1 unless the layer is a line (`ui/src/map/style.ts`), and layouts do the same (`crates/engine/src/layout/frame.rs`).
- A point's outline isn't a setting at all: it is a fixed `#333333` ring on the map and in layouts.
- Layout shapes already have a separate `fillOpacity` number. Their outlines have no opacity.
- `svg::color` already lets `#rrggbbaa` through, but nothing produces it and no renderer has been checked with it.
- `@fluentui/react-color-picker` (MIT) is already installed with Fluent UI, so no new supplier is needed. Confirm during CP1 that its version is usable and that it's in the shipped set.

## Decisions

1. **"Native" means drawn by the app.** One Fluent-styled picker that looks and works the same on Mac, Windows and Linux. The operating system's panel is no longer used anywhere.
2. **Transparency is stored in the colour, as `#rrggbbaa`.** Existing `#rrggbb` values stay valid and mean opaque, so old projects open and look the same with no migration. Classes, stroke stacks and labels get transparency without a new field each.
3. **Outline opacity is the alpha of the outline colour**, not a second number. The Outline section shows an "Opacity" percentage and slider next to the colour. It and the picker's alpha edit the same stored value, so they can never disagree.
4. **Existing sliders keep their meaning, and the two multiply.** Final opacity = colour alpha × the slider that already applies to that part (the layer slider for fills and for lines). A polygon's outline has no such slider, so old maps don't change. For polygons the current slider is relabelled **Fill opacity**, which is what it actually does.
5. **No control that does nothing.** The picker takes an `alpha` setting per use. Where a renderer can't honour transparency, the alpha control isn't shown.
6. **Selection highlights stay opaque**, so a selected transparent feature is still visible.

## The picker

A popover opened from a colour swatch (transparent colours show over a checkerboard):
- saturation and brightness area, hue slider, alpha slider;
- hex field that accepts 3, 4, 6 or 8 digits; RGB fields; alpha in percent;
- recent colours (per computer), this project's colours, and the Okabe–Ito colour-blind-safe set the charts already use;
- a "None" button where the setting is optional (today handled by each control on its own);
- live preview on the map while dragging, one undo step per open-and-change, Escape cancels;
- full keyboard use and screen-reader labels, passing the existing AA and focus checks.

Not included: an eyedropper. The browser API for it isn't in the Mac WebKit view, and cross-platform is a requirement. It can come later as "pick from the map".

## Where alpha is offered

| Control | Alpha |
|---|---|
| Layer fill and colour ramps (`PropertiesPane`) | Yes |
| Layer outline (`PropertiesPane`) | Yes, with the Opacity field |
| Unique classes (`UniqueClasses`) | Yes |
| Stroke stacks | Yes |
| Label text, halo, background (`LabelSettings`) | Yes |
| Layout shape fill (`ElementProperties`) | Yes, reading and writing the existing `fillOpacity` |
| Layout shape and frame outlines, text (`ElementProperties`, `TextSection`, `LayoutMenus`) | Yes |
| Tool parameters (`ParamControl`, `ParamKind::Color`) | Only if the `ToolSpec` parameter opts in; the Symbology node does |
| Raster colours (`RasterSymbology`) | No at first: `RasterSymbology` has its own transparency setting |
| Paper colour (`PageProperties`, `PageSetupDialog`) | No |
| App theme (`AppearancePanel`) | No |

## Packages

| Task | Effort | What |
|---|---|---|
| CP1 | 0.5 | `ColorField` component and `ui/src/theme/color.ts` (one parser and formatter). vitest for parsing, rounding (alpha 0–255 and percent round-trip) and the None state. |
| CP2 | 0.5–1 | Rendering. Accept `#rrggbbaa` in `nodeStyle.ts`, `ParamControl` and every other `#rrggbb` check. New `svg::color_alpha` splits a colour into RGB and alpha, written as `fill-opacity` and `stroke-opacity` so PNG (resvg) and PDF (svg2pdf) both get it. Map: `style.ts` and `olStyle.ts`. Outline opacity on polygons, class outlines and stroke stacks. Legends and layout patches show the same transparency. Add an optional point outline colour: if absent, today's `#333333` ring is drawn, so old maps don't change. |
| CP3 | 0.5 | Replace all the controls in the table (re-count after Wave 1 merges), relabel Fill opacity, add the Outline opacity field. Copying styles (`multiselect.ts`) and style files keep alpha. |

**Total:** about 1.5–2 sessions.

## Tests (add to `docs/TEST_PLAN.md`)

- An old project's style renders pixel-identically.
- A polygon with a 50% red outline over white gives the expected colour on the map, in a layout PNG, and in a layout PDF rendered back to pixels. Keep a tolerance for anti-aliasing and test away from corners.
- Alpha multiplies with the layer slider on a line layer.
- Outline opacity and the picker's alpha stay equal after editing either one.
- A style with alpha survives save, reopen, copy to another layer, and export to a style file and back.
- Control audit: no `input[type=color]` remains, and the new swatches all open.

A known effect to document, not fix: where two transparent outlines overlap, such as a shared polygon edge, the line looks darker there.
