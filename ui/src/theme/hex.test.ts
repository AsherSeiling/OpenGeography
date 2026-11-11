import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * Chrome code (CSS and TSX that draw the app's own UI) takes colours from tokens. A raw hex colour fails this test unless the
 * file is listed below. Listed files hold colours that are map, layout or data *content* (symbol defaults, ramps, page overlays),
 * or chrome that a later work package of docs/UI_REDESIGN_PLAN.md §6 cleans; remove the entry when that package lands.
 */
const SRC = fileURLToPath(new URL("..", import.meta.url));

type Why = { why: string; owner?: string };
const ALLOW: Record<string, Why> = {
  // Content: colours the user's map, layout or data owns, never the app's chrome.
  "map/style.ts": { why: "map symbol and label defaults" },
  "map/olStyle.ts": { why: "map symbol defaults" },
  "edit/EditOverlay.ts": { why: "colours of the shapes being edited, drawn on the map (OpenLayers styles take colour values, not CSS variables)" },
  "map/usngGrid.ts": { why: "USNG grid line colours on the map" },
  "georef/maps.ts": { why: "colours of the control point markers drawn on the image and reference maps (OpenLayers styles take colour values, not CSS variables)" },
  "layers/nodeStyle.ts": { why: "label colour defaults of a layer's style" },
  "store/store.ts": { why: "default layer palette and label defaults" },
  "store/layout.ts": { why: "layout document defaults" },
  "topo/assign.ts": { why: "topographic symbol colours" },
  "layout/types.ts": { why: "page background default (white paper)" },
  "layout/topoData.ts": { why: "layout topo defaults" },
  "layout/actions.ts": { why: "layout element default colours" },
  "layout/TextSection.tsx": { why: "colour input fallbacks for layout text" },
  "layout/ElementProperties.tsx": { why: "colour input fallbacks for layout elements" },
  "layout.css": { why: "page overlays on the white page: selection outlines, handles, guides, pasteboard" },
  "dialogs/PageSetupDialog.tsx": { why: "page background colour (white paper)" },
  "ribbon/Ribbon.tsx": { why: "page background default (white paper)" },
  "nodes/ParamControl.tsx": { why: "<input type=color> fallback value" },
  "panes/LabelSettings.tsx": { why: "label background, outline and shadow defaults" },
  "panes/PropertiesPane.tsx": { why: "effect colour defaults and <input type=color> fallback" },
  "sources/cite.ts": { why: "print stylesheet of a generated document" },
  "theme/color.ts": { why: "Okabe-Ito colour-blind-safe set offered in the colour picker (content colours)" },
  "theme/colorMath.ts": { why: "ink, white and black: the candidates the readable-text rule picks from" },
  // Chrome that a later work package restyles.
  "panes/ContentsPane.tsx": { why: "raster swatch ramp (content)" },
  "raster/RasterSymbology.tsx": { why: "ramp fallback colour" },
};

const HEX = /#[0-9a-fA-F]{3,8}\b/g;

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

const files = walk(SRC)
  .map((p) => ({ rel: relative(SRC, p).split(sep).join("/"), path: p }))
  .filter(({ rel }) => /\.(css|ts|tsx)$/.test(rel) && !/\.test\.tsx?$/.test(rel) && rel !== "theme/tokens.css")
  .map(({ rel, path }) => ({ rel, text: readFileSync(path, "utf8") }));

const key = (rel: string) => (rel === "theme/layout.css" ? "layout.css" : rel);

describe("no raw hex colours in chrome code", () => {
  it("scans something", () => {
    expect(files.length).toBeGreaterThan(50);
    expect(files.some((f) => f.rel === "theme/base.css")).toBe(true);
  });

  it("only allow-listed files contain a raw hex colour", () => {
    const bad: string[] = [];
    for (const { text, rel } of files) {
      if (ALLOW[key(rel)]) continue;
      const hits = text.match(HEX);
      if (hits) bad.push(`${rel}: ${[...new Set(hits)].join(" ")}`);
    }
    expect(bad, "use a token from theme/tokens.css (var(--text), var(--rule), ...) or add a reasoned ALLOW entry").toEqual([]);
  });

  it("the allow-list has no stale entries", () => {
    const stale: string[] = [];
    for (const k of Object.keys(ALLOW)) {
      const f = files.find((x) => key(x.rel) === k);
      if (!f) stale.push(`${k}: file is gone`);
      else if (!HEX.test(f.text)) stale.push(`${k}: no hex left, remove the entry`);
      HEX.lastIndex = 0;
    }
    expect(stale).toEqual([]);
  });

  it("every chrome entry names the work package that cleans it", () => {
    for (const [k, v] of Object.entries(ALLOW)) {
      if (v.owner !== undefined) expect(v.owner, k).toMatch(/^WP \d/);
    }
  });
});
