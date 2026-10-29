// Pixel-level protocol for raster symbology (docs/CARTOGRAPHY_TEST_PROTOCOL.md §A): real files, the real tile path
// (style token in the tile URL → engine → PNG), and the colours that come out.
import { test, expect, gis } from "./fixtures";
import { decodePng, tilePixel, tileOf, writeAsc, writeGeoTiff, type RGBA } from "./pixels";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

const Z = 8;
// 0.8° square at z8 inside one tile (x 57, y 96); value = column (0..79) + row/100
const LON0 = -99.8, LAT0 = 40, CELL = 0.01, N = 80;
const API = () => `http://127.0.0.1:${process.env.E2E_API_PORT}`;
const token = (style: object) => Buffer.from(JSON.stringify(style)).toString("base64url");

let dir: string;
test.beforeAll(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), "rpx-"));
});

async function addRaster(app: any, file: string) {
  return gis(app, async (g, a: any) => {
    const [l] = await g.api.importFiles([a.file]);
    g.useApp.getState().addLayers([l]);
    return l;
  }, { file }) as Promise<any>;
}

/** Fetch the tile with this style and sample the pixel at (lon, lat). */
async function sample(app: any, layer: any, style: object | null, pts: [number, number][]) {
  const t = tileOf(LON0 + CELL * N / 2, LAT0 + 0.4, Z);
  const f = style ? `&fields=${encodeURIComponent(token(style))}` : "";
  const res = await app.request.get(`${API()}/tiles/${layer.id}/${Z}/${t.x}/${t.y}.png?v=${layer.version}${f}`);
  expect(res.ok(), await res.text().catch(() => "")).toBeTruthy();
  const png = (await res.body()).toString("base64");
  const points = pts.map(([lon, lat]) => tilePixel(lon, lat, Z, t.x, t.y));
  return decodePng(app, png, points);
}

const LEFT: [number, number] = [-99.79, 40.4];
const RIGHT: [number, number] = [-99.01, 40.4];
const OUTSIDE: [number, number] = [-99.95, 40.4]; // in the tile, west of the raster
const lum = (c: RGBA) => 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];

test("RP-1 renderers: default, gray, pseudocolour (viridis, inverted), classed, with the colours the ramp defines", async ({ app }) => {
  const f = path.join(dir, "grad.asc");
  writeAsc(f, N, N, LON0, LAT0, CELL, (c, r) => c + r / 100);
  const layer = await addRaster(app, f);
  expect(layer.crs).toBeTruthy();

  const def = await sample(app, layer, null, [LEFT, RIGHT, OUTSIDE]);
  expect(def.px[0][3]).toBe(255);
  expect(def.px[2][3]).toBe(0); // outside the raster: transparent

  const gray = await sample(app, layer, { renderer: "gray" }, [LEFT, RIGHT]);
  for (const p of gray.px) expect(p[0] === p[1] && p[1] === p[2]).toBeTruthy();
  expect(gray.px[0][0]).toBeLessThan(25); // lowest values are black
  expect(gray.px[1][0]).toBeGreaterThan(230);

  const inv = await sample(app, layer, { renderer: "gray", invert: true }, [LEFT, RIGHT]);
  expect(inv.px[0][0]).toBeGreaterThan(230);
  expect(inv.px[1][0]).toBeLessThan(25);

  const vir = await sample(app, layer, { renderer: "pseudocolor", ramp: "viridis" }, [LEFT, RIGHT]);
  // viridis runs #440154 (purple) → #fde725 (yellow)
  expect(vir.px[0][2]).toBeGreaterThan(vir.px[0][0]); // purple: blue > red
  expect(vir.px[0][1]).toBeLessThan(40);
  expect(vir.px[1][0]).toBeGreaterThan(220); // yellow: red and green high, blue low
  expect(vir.px[1][1]).toBeGreaterThan(200);
  expect(vir.px[1][2]).toBeLessThan(80);

  const rev = await sample(app, layer, { renderer: "pseudocolor", ramp: "viridis", invert: true }, [LEFT, RIGHT]);
  expect(rev.px[0][0]).toBeGreaterThan(220);
  expect(rev.px[1][2]).toBeGreaterThan(rev.px[1][0]);

  // Classified into 4 classes: very few distinct colours (plus the transparent outside); continuous has many.
  const classed = await sample(app, layer, { renderer: "pseudocolor", ramp: "Blues", mode: "discrete", breaks: [20, 40, 60] }, [LEFT, RIGHT]);
  const cont = await sample(app, layer, { renderer: "pseudocolor", ramp: "Blues" }, [LEFT, RIGHT]);
  expect(classed.distinct).toBeLessThanOrEqual(5);
  expect(cont.distinct).toBeGreaterThan(30);
});

test("RP-2 stretch modes, transparency, opacity, adjustments", async ({ app }) => {
  const f = path.join(dir, "grad2.asc");
  writeAsc(f, N, N, LON0, LAT0, CELL, (c) => c);
  const layer = await addRaster(app, f);
  const mid: [number, number] = [-99.3, 40.4]; // value ≈ 50
  const at = async (style: object) => (await sample(app, layer, { renderer: "gray", ...style }, [LEFT, mid, RIGHT])).px;

  // Manual stretch 40..60: the left edge clips to black, the right to white, the middle is mid-grey.
  const manual = await at({ stretch: { mode: "manual", min: 40, max: 60 } });
  expect(manual[0][0]).toBeLessThan(5);
  expect(manual[2][0]).toBeGreaterThan(250);
  expect(Math.abs(manual[1][0] - 128)).toBeLessThan(40);
  // Percentile clipping brightens the same data relative to min-max near the top.
  const minmax = await at({});
  const pct = await at({ stretch: { mode: "percentile", pctLow: 10, pctHigh: 90 } });
  expect(pct[2][0]).toBeGreaterThanOrEqual(minmax[2][0]);
  expect(pct[0][0]).toBeLessThanOrEqual(minmax[0][0]);

  // Value-range transparency: cells 45..55 vanish, others stay.
  const hole = await at({ transparentRanges: [[45, 55]] });
  expect(hole[1][3]).toBe(0);
  expect(hole[0][3]).toBe(255);
  // Global opacity.
  const half = await at({ opacity: 0.5 });
  expect(Math.abs(half[0][3] - 128)).toBeLessThan(3);
  // Brightness lifts black; saturation -1 on a ramp gives grey; gamma > 1 lightens mid-tones.
  const bright = await at({ brightness: 0.3 });
  expect(bright[0][0]).toBeGreaterThan(60);
  const gam = await at({ gamma: 2 });
  expect(gam[1][0]).toBeGreaterThan(minmax[1][0]);
  const desat = (await sample(app, layer, { renderer: "pseudocolor", ramp: "viridis", saturation: -1 }, [mid])).px[0];
  expect(Math.max(desat[0], desat[1], desat[2]) - Math.min(desat[0], desat[1], desat[2])).toBeLessThan(4);
});

test("RP-3 unique values and multiband composites", async ({ app }) => {
  // Categorical: three vertical bands with values 1, 2, 3.
  const cat = path.join(dir, "cat.asc");
  writeAsc(cat, N, N, LON0, LAT0, CELL, (c) => (c < 27 ? 1 : c < 53 ? 2 : 3));
  const layer = await addRaster(app, cat);
  const uv = await gis(app, (g, a: any) => g.api.rasterUniqueValues(a.id, 1, 50), { id: layer.id }) as any;
  expect(uv.values.map((v: any) => v.value)).toEqual([1, 2, 3]);
  expect(uv.values.map((v: any) => v.count)).toEqual([2160, 2080, 2160]);
  const style = { renderer: "paletted", entries: [{ value: 1, color: "#ff0000" }, { value: 2, color: "#00ff00", hidden: true }, { value: 3, color: "#0000ff" }] };
  const mids: [number, number][] = [[-99.67, 40.4], [-99.4, 40.4], [-99.14, 40.4]];
  const p = (await sample(app, layer, style, mids)).px;
  expect(p[0]).toEqual([255, 0, 0, 255]);
  expect(p[1][3]).toBe(0); // hidden class
  expect(p[2]).toEqual([0, 0, 255, 255]);
  const sharp = await sample(app, layer, style, [[-99.5, 40.4]]); // nearest-neighbour: no blended edge colours
  expect(sharp.distinct).toBeLessThanOrEqual(4);

  // Multiband: R rises left→right, G rises more slowly, B falls. (A constant band would stretch to black: its range is empty.)
  const rgb = path.join(dir, "rgb.tif");
  writeGeoTiff(rgb, N, N, 3, LON0, LAT0 + 0.8, CELL, (c, _r, b) => (b === 0 ? c * 3 : b === 1 ? c * 1.5 : 250 - c * 3));
  const l3 = await addRaster(app, rgb);
  expect(l3.raster?.bands).toBe(3);
  const natural = (await sample(app, l3, null, [LEFT, RIGHT])).px;
  expect(natural[0][0]).toBeLessThan(30);
  expect(natural[1][0]).toBeGreaterThan(215);
  const swapped = (await sample(app, l3, { renderer: "multiband", rgbBands: [3, 2, 1] }, [LEFT, RIGHT])).px;
  expect(swapped[0][0]).toBeGreaterThan(215); // red now shows band 3 (high at the left)
  expect(swapped[0][2]).toBeLessThan(40);
  expect(swapped[0][1]).toBeLessThan(15); // green = band 2, still low at the left
  expect(swapped[1][1]).toBeGreaterThan(240); // stretched on its own range, so it reaches full at the right
  const oneBand = (await sample(app, l3, { renderer: "multiband", rgbBands: [2, 2, 2] }, [LEFT, RIGHT])).px;
  for (const px of oneBand) expect(px[0] === px[1] && px[1] === px[2]).toBe(true);
  // A band the file doesn't have is refused in words.
  const err = await app.request.get(`${API()}/tiles/${l3.id}/${Z}/${tileOf(-99.4, 40.4, Z).x}/${tileOf(-99.4, 40.4, Z).y}.png?v=${l3.version}&fields=${encodeURIComponent(token({ renderer: "multiband", rgbBands: [1, 2, 9] }))}`);
  expect(err.ok()).toBe(false);
  expect(await err.text()).toContain("Band 9");
});

test("RP-4 relief shading: flat ground keeps its colour, slopes change it, slope mode darkens", async ({ app }) => {
  const f = path.join(dir, "ramp3.asc");
  // West half flat (100 m), east half a steep ramp.
  writeAsc(f, N, N, LON0, LAT0, CELL, (c) => (c < 40 ? 100 : 100 + (c - 40) * 400));
  const layer = await addRaster(app, f);
  const flatPt: [number, number] = [-99.65, 40.4];
  const slopePt: [number, number] = [-99.2, 40.4];
  const base = { renderer: "gray", stretch: { mode: "manual", min: 0, max: 20000 } };
  const plain = (await sample(app, layer, base, [flatPt, slopePt])).px;
  const shaded = (await sample(app, layer, { ...base, shading: { mode: "hillshade", strength: 1 } }, [flatPt, slopePt])).px;
  expect(Math.abs(shaded[0][0] - plain[0][0] * 1)).toBeLessThanOrEqual(1 + plain[0][0] * 0.02); // flat: unchanged within rounding (shade factor 1)
  expect(Math.abs(shaded[1][0] - plain[1][0])).toBeGreaterThan(5); // slope: changed
  const slopeMode = (await sample(app, layer, { ...base, shading: { mode: "slope", strength: 1 } }, [flatPt, slopePt])).px;
  expect(lum(slopeMode[1])).toBeLessThan(lum(plain[1]));
  expect(Math.abs(slopeMode[0][0] - plain[0][0])).toBeLessThanOrEqual(1 + plain[0][0] * 0.02);
});
