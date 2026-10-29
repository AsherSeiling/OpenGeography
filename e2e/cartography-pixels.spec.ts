// Pixel-level protocol for the cartography suite (docs/CARTOGRAPHY_TEST_PROTOCOL.md §B): the engine renders real
// layouts and we look at the colours that come out.
import { test, expect, gis } from "./fixtures";
import { analyse, solidPngUri, writeAsc } from "./pixels";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

const K = 96 / 25.4; // preview pixels per mm
const mmx = (v: number) => v * K;
let dir: string;
test.beforeAll(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), "cpx-"));
});

const square = (x0: number, y0: number, x1: number, y1: number) => [[[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]];
const feature = (geometry: object, properties: object) => ({ type: "Feature", properties, geometry });
const writeGeojson = (name: string, features: object[]) => {
  const p = path.join(dir, name);
  fs.writeFileSync(p, JSON.stringify({ type: "FeatureCollection", features }));
  return p;
};

async function importLayer(app: any, file: string) {
  return gis(app, async (g, a: any) => {
    const [l] = await g.api.importFiles([a.file]);
    g.useApp.getState().addLayers([l]);
    return l;
  }, { file }) as Promise<any>;
}

type Setup = { cover: any; cats: any; pts: any; line: any; ras: any; centre: [number, number] };

/** Layers around lon -93, lat 45 (UTM 15N) and the frame centre in that CRS. */
async function setup(app: any): Promise<Setup> {
  const cover = await importLayer(app, writeGeojson("cover.geojson", [feature({ type: "Polygon", coordinates: square(-93.5, 44.8, -92.5, 45.3) }, { name: "cover" })]));
  const cats = await importLayer(app, writeGeojson("cats.geojson", [
    feature({ type: "Polygon", coordinates: square(-93.05, 45.0, -93.03, 45.02) }, { cls: "A", pop: 10 }),
    feature({ type: "Polygon", coordinates: square(-93.02, 45.0, -93.0, 45.02) }, { cls: "B", pop: 30 }),
    feature({ type: "Polygon", coordinates: square(-92.99, 45.0, -92.97, 45.02) }, { cls: "C", pop: 50 }),
  ]));
  const pts = await importLayer(app, writeGeojson("pts.geojson", [feature({ type: "Point", coordinates: [-93.0, 45.05] }, { n: 1 })]));
  const line = await importLayer(app, writeGeojson("line.geojson", [feature({ type: "LineString", coordinates: [[-93.06, 45.04], [-92.94, 45.04]] }, { n: 1 })]));
  const ras = await (async () => {
    const f = path.join(dir, "ras.asc");
    writeAsc(f, 50, 50, -93.1, 44.95, 0.002, (c) => c);
    return importLayer(app, f);
  })();
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  const centre = (await gis(app, (g) => g.api.transformPoint(-93.0, 45.03, "EPSG:4326", "EPSG:26915"))) as [number, number];
  return { cover, cats, pts, line, ras, centre };
}

const base = (extra: object[] = [], page: object = {}) => ({
  id: "L1", name: "Pixels", version: 1, body: "earth", page: { width: 200, height: 120, units: "mm", margin: 10, background: "#ffffff", ...page },
  elements: extra, series: null,
});
const place = (id: string, x: number, y: number, w: number, h: number, more: object = {}) => ({ id, x, y, w, h, rotation: 0, locked: false, visible: true, ...more });
const frame = (s: Setup, over: object = {}) => ({ kind: "map_frame", ...place("f1", 10, 10, 100, 60), crs: "EPSG:26915", center: s.centre, scale: 100000, layers: "map", basemap: "none", graticule: null, grid: null, border: null, ...over });
const sty = (fill: string, extra: object = {}) => ({ kind: "single", fill, stroke: "#000000", strokeWidth: 0, opacity: 1, radius: 4, ...extra });

async function draw(app: any, layout: object, styles: Record<string, object>, order: string[], queries: any[], dpi = 96) {
  const out = (await gis(app, (g, a: any) => g.api.renderLayoutPreview({ layout: a.layout, styles: a.styles, order: a.order, visible: {}, project: "p", date: "2026-01-01" }, a.dpi), { layout, styles, order, dpi })) as any;
  return { answers: await analyse(app, out.png, queries), warnings: out.warnings as string[], png: out.png as string };
}

const RED: [number, number, number] = [204, 0, 0];

test("CP-1 shapes, pictures, decoration and bleed are really drawn", async ({ app }) => {
  const layout = base([
    { kind: "shape", ...place("rect", 10, 10, 40, 20), shape: "rectangle", fill: "#cc0000", stroke: "#000000", strokeWidth: 0.3 },
    { kind: "shape", ...place("ell", 60, 10, 30, 20), shape: "ellipse", fill: "#00aa00", stroke: "#000000", strokeWidth: 0.2 },
    { kind: "shape", ...place("tri", 100, 10, 20, 20), shape: "triangle", fill: "#0000cc", stroke: "#0000cc", strokeWidth: 0.2 },
    { kind: "shape", ...place("ln", 130, 20, 40, 1), shape: "line", fill: "none", stroke: "#000000", strokeWidth: 0.8 },
    { kind: "shape", ...place("sh", 10, 40, 40, 20), shape: "rectangle", fill: "#ffffff", stroke: "#000000", strokeWidth: 0.2, decor: { shadow: true, shadowOffset: 3 } },
    { kind: "picture", ...place("pic", 60, 40, 30, 20), source: solidPngUri(8, 8, [51, 102, 153, 255]), fit: "stretch" },
    { kind: "picture", ...place("pic2", 100, 40, 30, 20), source: solidPngUri(8, 8, [51, 102, 153, 255]), fit: "stretch", opacity: 0.5 },
    { kind: "text", ...place("tx", 10, 70, 80, 14), text: "HELLO **bold** *it*", font: "sans", size: 24, align: "left", color: "#000000" },
    { kind: "text", ...place("bx", 100, 70, 40, 10), text: "", font: "sans", size: 10, align: "left", color: "#000000", decor: { background: "#ffff00" } },
  ]);
  const { answers, warnings } = await draw(app, layout, {}, [], [
    { px: [mmx(30), mmx(20)] }, { px: [mmx(5), mmx(5)] }, // rectangle centre / outside
    { px: [mmx(75), mmx(20)] }, { px: [mmx(61), mmx(11)] }, // ellipse centre / its empty corner
    { px: [mmx(110), mmx(26)] }, { px: [mmx(101), mmx(11)] }, // triangle base / empty corner
    { nonwhite: [mmx(132), mmx(19), mmx(36), mmx(3)] }, // the line
    { px: [mmx(51.5), mmx(58.5)] }, { px: [mmx(55), mmx(62)] }, // inside the shadow band / beyond it
    { px: [mmx(75), mmx(50)] }, { px: [mmx(115), mmx(50)] }, // picture / half-transparent picture
    { nonwhite: [mmx(10), mmx(70), mmx(80), mmx(14)] }, // text
    { px: [mmx(120), mmx(75)] }, // decor background
  ]);
  expect(warnings).toEqual([]);
  expect(answers[0].slice(0, 3)).toEqual([204, 0, 0]);
  expect(answers[1]).toEqual([255, 255, 255, 255]);
  expect(answers[2].slice(0, 3)).toEqual([0, 170, 0]);
  expect(answers[3].slice(0, 3)).toEqual([255, 255, 255]);
  expect(answers[4].slice(0, 3)).toEqual([0, 0, 204]);
  expect(answers[5].slice(0, 3)).toEqual([255, 255, 255]);
  expect(answers[6]).toBeGreaterThan(100);
  expect(answers[7][0]).toBeLessThan(220); // grey band of the shadow
  expect(answers[7][0]).toBeGreaterThan(120);
  expect(answers[8].slice(0, 3)).toEqual([255, 255, 255]);
  expect(answers[9].slice(0, 3)).toEqual([51, 102, 153]);
  expect(answers[10][0]).toBeGreaterThan(130); // 50 % blend with white
  expect(answers[10][0]).toBeLessThan(170);
  expect(answers[11]).toBeGreaterThan(200);
  expect(answers[12].slice(0, 3)).toEqual([255, 255, 0]);

  // Bleed grows the image; the page background fills it; crop marks are drawn outside it.
  const bleed = await draw(app, base([], { bleed: 3, cropMarks: true, background: "#ffeecc" }), {}, [], [{ size: true }, { px: [mmx(3) + 1, mmx(3) + 1] }, { px: [mmx(13) + 1, mmx(13) + 1] }, { nonwhite: [0, mmx(13) - 2, mmx(8), 4] }, { nonwhite: [mmx(13) + 5, mmx(13) + 5, 20, 20] }]);
  const [w, h] = bleed.answers[0];
  expect(Math.abs(w - (200 + 2 * 13) * K)).toBeLessThan(2);
  expect(Math.abs(h - (120 + 2 * 13) * K)).toBeLessThan(2);
  expect(bleed.answers[1].slice(0, 3)).toEqual([255, 255, 255]); // slug outside the bleed is white
  expect(bleed.answers[2].slice(0, 3)).toEqual([255, 238, 204]); // inside the bleed: page colour
  expect(bleed.answers[3]).toBeGreaterThan(5); // a crop mark
});

test("CP-2 map frame content: layers, clip shapes, overview rectangle, patterns, markers, line styles, shadows", async ({ app }) => {
  const s = await setup(app);
  const styles = { [s.cover.id]: sty("#cc0000"), [s.cats.id]: sty("#00aa00"), [s.pts.id]: sty("#0000cc", { radius: 6 }), [s.line.id]: sty("#000000", { stroke: "#000000", strokeWidth: 4 }) };
  const only = (...ids: string[]) => ids;
  const redIn = (x: number, y: number, w: number, h: number) => ({ count: [mmx(x), mmx(y), mmx(w), mmx(h)], rgb: RED });

  // The frame draws the cover polygon; clipped to an ellipse the corners stay white.
  const rect = await draw(app, base([frame(s)]), styles, only(s.cover.id), [{ px: [mmx(12), mmx(12)] }, { px: [mmx(60), mmx(40)] }]);
  expect(rect.answers[0].slice(0, 3)).toEqual([204, 0, 0]);
  expect(rect.answers[1].slice(0, 3)).toEqual([204, 0, 0]);
  for (const clip of [{ shape: "ellipse" }, { shape: "rounded", radius: 20 }, { shape: "layer", layer: s.cats.id }]) {
    const c = await draw(app, base([frame(s, { clip })]), styles, only(s.cover.id), [{ px: [mmx(11), mmx(11)] }, { px: [mmx(60), mmx(40)] }, { px: [mmx(109), mmx(69)] }]);
    expect(c.answers[0].slice(0, 3), JSON.stringify(clip)).toEqual([255, 255, 255]);
    if (clip.shape !== "layer") expect(c.answers[1].slice(0, 3)).toEqual([204, 0, 0]);
  }

  // Pattern: hatch lines in the outline colour over the fill; without a pattern there are none.
  const patStyle = { ...styles, [s.cover.id]: sty("#cc0000", { stroke: "#0000cc", strokeWidth: 1, pattern: "hatch", patternSpacing: 8 }) };
  const noPat = await draw(app, base([frame(s)]), { ...styles, [s.cover.id]: sty("#cc0000", { stroke: "#0000cc", strokeWidth: 1 }) }, only(s.cover.id), [{ count: [mmx(20), mmx(20), mmx(60), mmx(30)], rgb: [0, 0, 204] }]);
  const pat = await draw(app, base([frame(s)]), patStyle, only(s.cover.id), [{ count: [mmx(20), mmx(20), mmx(60), mmx(30)], rgb: [0, 0, 204] }, redIn(20, 20, 60, 30)]);
  expect(noPat.answers[0]).toBe(0);
  expect(pat.answers[0]).toBeGreaterThan(100);
  expect(pat.answers[1]).toBeGreaterThan(100);

  // Markers: a square covers more pixels than a circle of the same size; a star fewer.
  const marker = async (m?: string) => (await draw(app, base([frame(s)]), { ...styles, [s.pts.id]: sty("#0000cc", { radius: 8, marker: m }) }, only(s.pts.id), [{ count: [mmx(10), mmx(10), mmx(100), mmx(60)], rgb: [0, 0, 204] }])).answers[0] as number;
  const [circle, squareN, star, cross] = [await marker(), await marker("square"), await marker("star"), await marker("cross")];
  expect(squareN / circle).toBeGreaterThan(0.85); // a square and a circle of the same size cover about the same area
  expect(squareN / circle).toBeLessThan(1.25);
  expect(star).toBeLessThan(circle * 0.7);
  expect(cross).toBeLessThan(circle * 0.7);
  expect(star).toBeGreaterThan(20);
  expect(star).not.toBe(cross);

  // Line styles: dashed has gaps.
  const lineCount = async (dash?: string) => (await draw(app, base([frame(s)]), { ...styles, [s.line.id]: sty("#000000", { stroke: "#000000", strokeWidth: 4, dash }) }, only(s.line.id), [{ count: [mmx(10), mmx(10), mmx(100), mmx(60)], rgb: [0, 0, 0], tol: 30 }])).answers[0] as number;
  const [solid, dashed, dotted] = [await lineCount(), await lineCount("dashed"), await lineCount("dotted")];
  expect(dashed).toBeLessThan(solid * 0.85);
  expect(dotted).toBeLessThan(dashed);
  expect(dotted).toBeGreaterThan(5);

  // Layer shadow: opaque black offset copy appears only with the effect.
  const cc = (await gis(app, (g) => g.api.transformPoint(-93.01, 45.01, "EPSG:4326", "EPSG:26915"))) as [number, number];
  const shadow = async (effect?: object) => (await draw(app, base([frame(s, { center: cc, scale: 60000 })]), { ...styles, [s.cats.id]: sty("#00aa00", { effect }) }, only(s.cats.id), [{ count: [mmx(10), mmx(10), mmx(100), mmx(60)], rgb: [0, 0, 0], tol: 20 }, { count: [mmx(10), mmx(10), mmx(100), mmx(60)], rgb: [0, 170, 0], tol: 20 }])).answers as number[];
  const [noFx, fx] = [await shadow(), await shadow({ dx: 2, dy: 2, color: "#000000", opacity: 1 })];
  expect(noFx[0]).toBe(0);
  expect(fx[0]).toBeGreaterThan(200);
  expect(fx[1]).toBeGreaterThan(200);

  // Overview: a second frame outlines the first frame's extent in the indicator colour.
  const ov = { kind: "map_frame", ...place("ov", 120, 10, 60, 40), crs: "EPSG:26915", center: s.centre, scale: 400000, layers: "map", basemap: "none", graticule: null, grid: null, border: { width: 0.3, color: "#000000" }, indicator: { frame: "f1", color: "#d95f02", width: 0.5, fillOpacity: 0 } };
  const o = await draw(app, base([frame(s), ov]), styles, only(s.cover.id), [{ count: [mmx(120), mmx(10), mmx(60), mmx(40)], rgb: [217, 95, 2], tol: 40 }]);
  expect(o.answers[0]).toBeGreaterThan(30);
  expect(o.answers[0]).toBeLessThan(2000); // an outline, not a filled blob
  const noInd = await draw(app, base([frame(s), { ...ov, indicator: null }]), styles, only(s.cover.id), [{ count: [mmx(120), mmx(10), mmx(60), mmx(40)], rgb: [217, 95, 2], tol: 40 }]);
  expect(noInd.answers[0]).toBe(0);
});

test("CP-3 surround elements: north arrows, scale bars, legend (vector and raster), table, chart", async ({ app }) => {
  const s = await setup(app);
  const styles = { [s.cover.id]: sty("#cc0000"), [s.cats.id]: sty("#00aa00"), [s.ras.id]: { ...sty("#888888"), raster: { renderer: "pseudocolor", ramp: "viridis", stretch: { mode: "manual", min: 0, max: 49 } } } };
  const dark = (x: number, y: number, w: number, h: number) => ({ count: [mmx(x), mmx(y), mmx(w), mmx(h)], rgb: [0, 0, 0], tol: 60 });
  const at = (el: object) => [frame(s), el];

  // North arrows: each style draws something dark; the colour option works; "rose" has more ink than "triangle".
  const ink: Record<string, number> = {};
  for (const style of ["simple", "compass", "rose", "arrow", "triangle", "half", "circle"]) {
    const r = await draw(app, base(at({ kind: "north_arrow", ...place("na", 120, 10, 24, 36), frame: "f1", style, color: "#003366" })), styles, [], [{ count: [mmx(120), mmx(10), mmx(24), mmx(36)], rgb: [0, 51, 102], tol: 30 }]);
    ink[style] = r.answers[0];
    expect(ink[style], style).toBeGreaterThan(15);
  }
  expect(ink.triangle).toBeGreaterThan(ink.half);

  // Scale bars: alternating has solid black blocks, hollow has none; double adds a second bar.
  const bar = async (extra: object) => (await draw(app, base(at({ kind: "scale_bar", ...place("sb", 120, 60, 70, 24), frame: "f1", units: "km", segments: 4, ...extra })), styles, [], [dark(120, 60, 70, 24)])).answers[0] as number;
  const [alt, hollow, line, dbl] = [await bar({}), await bar({ style: "hollow" }), await bar({ style: "line" }), await bar({ style: "double", units2: "mi" })];
  expect(alt).toBeGreaterThan(hollow);
  expect(alt).toBeGreaterThan(line);
  expect(dbl).toBeGreaterThan(alt);

  // Legend with a vector layer swatch and a raster colour bar (many distinct colours).
  const lg = await draw(app, base(at({ kind: "legend", ...place("lg", 120, 10, 70, 60), frame: "f1", title: "Legend", patch: "circle" })), styles, [s.ras.id, s.cats.id, s.cover.id], [
    { count: [mmx(120), mmx(10), mmx(70), mmx(60)], rgb: [0, 170, 0], tol: 30 }, { count: [mmx(120), mmx(10), mmx(70), mmx(60)], rgb: RED, tol: 30 }, { nonwhite: [mmx(120), mmx(10), mmx(70), mmx(60)] },
  ]);
  expect(lg.answers[0]).toBeGreaterThan(10);
  expect(lg.answers[1]).toBeGreaterThan(10);
  expect(lg.answers[2]).toBeGreaterThan(500);
  const bar2 = await analyse(app, lg.png, Array.from({ length: 24 }, (_, i) => ({ px: [mmx(120) + 6 + i * 6, mmx(0)] })).map(() => ({ nonwhite: [0, 0, 1, 1] })));
  expect(bar2.length).toBe(24);

  // Table: header band and text; chart: Okabe–Ito blue bars and an orange second pie slice.
  const tb = await draw(app, base([frame(s), { kind: "table", ...place("tb", 120, 10, 70, 30), layer: s.cats.id, fields: ["cls", "pop"], sortField: "pop", descending: true, size: 8 }]), styles, [s.cats.id], [{ count: [mmx(120), mmx(10), mmx(70), mmx(8)], rgb: [230, 230, 230], tol: 6 }, { nonwhite: [mmx(120), mmx(10), mmx(70), mmx(30)] }]);
  expect(tb.answers[0]).toBeGreaterThan(300);
  expect(tb.answers[1]).toBeGreaterThan(300);
  const bars = await draw(app, base([frame(s), { kind: "chart", ...place("ch", 120, 10, 70, 40), layer: s.cats.id, chartType: "bar", category: "cls", value: "pop", title: "Pop" }]), styles, [s.cats.id], [{ count: [mmx(120), mmx(10), mmx(70), mmx(40)], rgb: [0, 114, 178], tol: 8 }]);
  expect(bars.answers[0]).toBeGreaterThan(200);
  const pie = await draw(app, base([frame(s), { kind: "chart", ...place("pi", 120, 10, 70, 40), layer: s.cats.id, chartType: "pie", category: "cls" }]), styles, [s.cats.id], [{ count: [mmx(120), mmx(10), mmx(70), mmx(40)], rgb: [0, 114, 178], tol: 8 }, { count: [mmx(120), mmx(10), mmx(70), mmx(40)], rgb: [230, 159, 0], tol: 8 }, { count: [mmx(120), mmx(10), mmx(70), mmx(40)], rgb: [0, 158, 115], tol: 8 }]);
  for (const n of pie.answers) expect(n).toBeGreaterThan(40);
});

test("CP-4 themes, raster layers in a frame, and overall render sanity", async ({ app }) => {
  const s = await setup(app);
  const styles = { [s.ras.id]: { ...sty("#888888"), raster: { renderer: "pseudocolor", ramp: "viridis", stretch: { mode: "manual", min: 0, max: 49 } } } };
  // A coloured raster layer shows many colours; the default look of a plain frame shows few.
  const rc = (await gis(app, (g) => g.api.transformPoint(-93.05, 45.0, "EPSG:4326", "EPSG:26915"))) as [number, number];
  const r = await draw(app, base([frame(s, { center: rc, scale: 90000 })]), styles, [s.ras.id], [{ count: [mmx(10), mmx(10), mmx(100), mmx(60)], rgb: [68, 1, 84], tol: 60 }, { count: [mmx(10), mmx(10), mmx(100), mmx(60)], rgb: [253, 231, 37], tol: 60 }]);
  expect(r.answers[0]).toBeGreaterThan(100);
  expect(r.answers[1]).toBeGreaterThan(100);
  // Themes change the page colour and the frame border.
  const themed = (await gis(app, (g, a: any) => g.api.applyLayoutTheme(a.layout, "topographic"), { layout: base([frame(s, { border: { width: 1, color: "#ff0000" } })]) })) as any;
  expect(themed.page.background).toBe("#fbf9f2");
  const t = await draw(app, themed, styles, [], [{ px: [mmx(150), mmx(100)] }, { count: [mmx(9), mmx(9), mmx(2), mmx(60)], rgb: [26, 26, 26], tol: 20 }]);
  expect(t.answers[0].slice(0, 3)).toEqual([251, 249, 242]);
  expect(t.answers[1]).toBeGreaterThan(20);
  for (const id of ["thematic", "minimalist"]) {
    const x = (await gis(app, (g, a: any) => g.api.applyLayoutTheme(a.layout, a.id), { layout: base([frame(s)]), id })) as any;
    expect(x.elements[0].kind).toBe("map_frame");
  }
});

test("CP-5 showcase page: everything together renders; the PNG is saved for a visual check", async ({ app }) => {
  const s = await setup(app);
  const styles = {
    [s.cover.id]: sty("#cfdcb4", { stroke: "#6b7a4f", strokeWidth: 1, pattern: "hatch2", patternSpacing: 10 }),
    [s.cats.id]: sty("#0072b2", { stroke: "#ffffff", strokeWidth: 1, effect: { dx: 0.8, dy: 0.8, color: "#000000", opacity: 0.35 } }),
    [s.pts.id]: sty("#d55e00", { radius: 7, marker: "star", stroke: "#000000" }),
    [s.line.id]: sty("#333333", { stroke: "#333333", strokeWidth: 3, dash: "dashed" }),
    [s.ras.id]: { ...sty("#888888"), raster: { renderer: "pseudocolor", ramp: "viridis", stretch: { mode: "manual", min: 0, max: 49 }, shading: { mode: "hillshade", strength: 0.4 } } },
  };
  const cc = (await gis(app, (g) => g.api.transformPoint(-93.01, 45.03, "EPSG:4326", "EPSG:26915"))) as [number, number];
  const layout = base([
    { kind: "text", ...place("title", 10, 6, 180, 12), text: "**Showcase map** — {layer_count} layers, {scale}", font: "sans", size: 20, align: "left", color: "#1f2a44" },
    frame(s, { ...place("f1", 10, 20, 110, 85), center: cc, scale: 60000, border: { width: 0.5, color: "#1a1a1a" }, graticule: { interval: "auto", labels: true }, clip: { shape: "rounded", radius: 6 } }),
    { kind: "map_frame", ...place("ov", 125, 20, 40, 30), crs: "EPSG:26915", center: cc, scale: 400000, layers: "map", basemap: "none", graticule: null, grid: null, border: { width: 0.3, color: "#000000" }, indicator: { frame: "f1", color: "#d95f02", width: 0.5, fillOpacity: 0.15 }, clip: { shape: "ellipse" } },
    { kind: "north_arrow", ...place("na", 170, 20, 20, 30), frame: "f1", style: "rose", color: "#1f2a44" },
    { kind: "legend", ...place("lg", 125, 55, 65, 50), frame: "f1", title: "Legend", patch: "rounded", decor: { background: "#ffffff", frameColor: "#999999", frameWidth: 0.25, shadow: true, shadowOffset: 1.2 } },
    { kind: "scale_bar", ...place("sb", 10, 108, 80, 24), frame: "f1", units: "km", segments: 4, style: "double", units2: "mi" },
    { kind: "chart", ...place("ch", 95, 108, 45, 24), layer: s.cats.id, chartType: "bar", category: "cls", value: "pop", size: 6 },
    { kind: "table", ...place("tb", 145, 108, 45, 24), layer: s.cats.id, fields: ["cls", "pop"], sortField: "pop", descending: true, size: 6 },
  ], { background: "#fbf9f2", height: 135 });
  const r = await draw(app, layout, styles, [s.pts.id, s.line.id, s.cats.id, s.ras.id, s.cover.id], [{ size: true }], 150);
  expect(r.warnings.filter((w) => !/scale varies|geographic/.test(w))).toEqual([]);
  const out = path.join(process.env.E2E_ARTIFACTS ?? os.tmpdir(), "cartography-showcase.png");
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, Buffer.from(r.png, "base64"));
  console.log("SHOWCASE", out);
});
