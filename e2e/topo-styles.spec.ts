// USGS classic quadrangle and military installation sheets (docs/TOPO_USGS_PLAN.md): built through the Topo Map Builder, drawn with synthetic
// National Map-like and military overlay layers, symbols assigned by layer name, and the rendered pages checked pixel by pixel and saved for a look.
import { test, expect, gis } from "./fixtures";
import { analyse } from "./pixels";
import { assignSymbols } from "../ui/src/topo/assign";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

let dir: string;
test.beforeAll(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), "topo-"));
});

const feature = (geometry: object, properties: object = {}) => ({ type: "Feature", properties, geometry });
const poly = (ring: [number, number][]) => ({ type: "Polygon", coordinates: [[...ring, ring[0]]] });
const rect = (x0: number, y0: number, x1: number, y1: number) => poly([[x0, y0], [x1, y0], [x1, y1], [x0, y1]]);
const line = (pts: [number, number][]) => ({ type: "LineString", coordinates: pts });
const ellipse = (cx: number, cy: number, rx: number, ry: number) => poly(Array.from({ length: 28 }, (_, i) => [cx + rx * Math.cos((i / 28) * 2 * Math.PI), cy + ry * Math.sin((i / 28) * 2 * Math.PI)] as [number, number]));

async function addLayer(app: any, name: string, features: object[]) {
  const file = path.join(dir, `${name.replace(/[^a-z0-9]+/gi, "_")}.geojson`);
  fs.writeFileSync(file, JSON.stringify({ type: "FeatureCollection", features }));
  const l = (await gis(app, async (g, a: any) => {
    const [info] = await g.api.importFiles([a.file]);
    await g.api.renameLayer?.(info.id, a.name);
    return info;
  }, { file, name })) as any;
  const renamed = (await gis(app, async (g, a: any) => {
    const infos = g.useApp.getState().layers;
    const cur = infos[a.id] ?? a.info;
    const info = { ...cur, name: a.name };
    g.useApp.getState().addLayers([info]);
    return info;
  }, { id: l.id, name, info: l })) as any;
  return renamed;
}

/** Layers like National Map data for a sheet centred on (lon0, lat0) with half-extents (hx, hy) degrees. */
async function baseLayers(app: any, lon0: number, lat0: number, hx: number, hy: number) {
  const x = (t: number) => lon0 - hx + 2 * hx * t;
  const y = (t: number) => lat0 - hy + 2 * hy * t;
  const contours = Array.from({ length: 16 }, (_, k) => feature(line(Array.from({ length: 41 }, (_, i) => [x(i / 40), y(k / 15) + hy * 0.05 * Math.sin(i / 5 + k)] as [number, number])), { ELEV: 800 + 40 * k, INDEX: k % 5 === 0 ? 1 : 0 }));
  await addLayer(app, "Woodland", [feature(poly([[x(0.5), y(0.45)], [x(1), y(0.45)], [x(1), y(1)], [x(0.6), y(1)], [x(0.45), y(0.7)]]))]);
  await addLayer(app, "Built-up area", [feature(rect(x(0.12), y(0.15), x(0.3), y(0.4)))]);
  await addLayer(app, "Lakes, ponds and reservoirs (NHD waterbodies)", [feature(ellipse(x(0.72), y(0.25), hx * 0.22, hy * 0.2))]);
  await addLayer(app, "Contours 40 ft", contours);
  await addLayer(app, "Streams and rivers (NHD flowlines)", [feature(line(Array.from({ length: 30 }, (_, i) => [x(i / 29), y(0.8 - 0.55 * (i / 29)) + hy * 0.08 * Math.sin(i / 3)] as [number, number])))]);
  await addLayer(app, "Local roads and streets", [feature(line([[x(0.05), y(0.5)], [x(0.95), y(0.5)]])), feature(line([[x(0.35), y(0.05)], [x(0.35), y(0.95)]])), feature(line([[x(0.2), y(0.9)], [x(0.6), y(0.2)]]))]);
  await addLayer(app, "Highways (interstate, US and state routes, ramps)", [feature(line([[x(0), y(0.62)], [x(0.5), y(0.58)], [x(1), y(0.7)]]))]);
  await addLayer(app, "Trails", [feature(line([[x(0.1), y(0.9)], [x(0.3), y(0.7)], [x(0.5), y(0.8)], [x(0.8), y(0.55)]]))]);
  await addLayer(app, "Railroads", [feature(line([[x(0), y(0.2)], [x(0.5), y(0.3)], [x(1), y(0.12)]]))]);
}

async function applySymbols(app: any, era: "classic" | "modern") {
  const st = (await gis(app, (g) => { const s = g.useApp.getState(); return { order: s.order, layers: s.layers, styles: s.styles }; })) as any;
  const symbols = (await gis(app, (g, a: any) => g.api.topoSymbols(a.era), { era })) as any[];
  const r = assignSymbols(st.order, st.layers, st.styles, symbols);
  await gis(app, (g, a: any) => { for (const [id, s] of Object.entries(a.changes)) g.useApp.getState().setStyle(id, s); }, { changes: r.changes });
  return r;
}

async function sheet(app: any, spec: object) {
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  await gis(app, async (g, a: any) => { await g.layoutActions.buildTopoSheet(a.spec); }, { spec });
  const out = (await gis(app, async (g, a: any) => {
    const s = g.useLayouts.getState();
    return g.api.renderLayoutPreview(g.layoutActions.layoutRequest(s.doc), a.dpi);
  }, { dpi: 80 })) as any;
  return out as { png: string; warnings: string[] };
}

const count = (rgb: [number, number, number], region: [number, number, number, number], tol = 22) => ({ count: region, rgb, tol });

test("TU-1 the USGS classic quadrangle: layers wear the catalogue's symbols and the marginalia are drawn", async ({ app }) => {
  await baseLayers(app, -93.0, 45.0, 0.06, 0.019);
  const r = await applySymbols(app, "classic");
  expect(r.assigned.map((a) => a.name).sort()).toEqual(["Built-up area", "Contours 40 ft", "Highways (interstate, US and state routes, ramps)", "Lakes, ponds and reservoirs (NHD waterbodies)", "Local roads and streets", "Railroads", "Streams and rivers (NHD flowlines)", "Trails", "Woodland"].sort());
  expect(r.skipped).toEqual([]);
  const { png, warnings } = await sheet(app, { center: [-93.0, 45.0], scale: 24000, paper: "tabloid", landscape: true, units: "imperial", datum: "nad83", title: "Test Lake", date: "2026-01-01", style: "usgs", era: "classic" });
  expect(warnings.filter((w) => !/scale varies|geographic/.test(w))).toEqual([]);
  fs.writeFileSync(path.join(process.env.E2E_ARTIFACTS ?? os.tmpdir(), "topo-usgs-classic.png"), Buffer.from(png, "base64"));
  const [size] = (await analyse(app, png, [{ size: true }])) as [number[]];
  const [W, H] = size;
  const k = W / 431.8; // preview pixels per mm
  const frame: [number, number, number, number] = [18 * k, 24 * k, 400 * k, 179 * k];
  const a = await analyse(app, png, [
    count([207, 232, 197], frame), // woodland tint #cfe8c5
    count([246, 211, 202], frame), // classic pink built-up tint
    count([200, 235, 247], frame), // lake tint
    count([200, 32, 47], frame, 70), // primary highway red fill
    count([22, 155, 208], frame, 90), // blue: streams, lake outline, UTM ticks (thin lines blend with white)
    count([168, 102, 47], frame, 90), // brown contours
    count([200, 32, 47], [340 * k, 215 * k, 90 * k, 50 * k], 100), // red in the road legend: the highway swatch
    { nonwhite: [8 * k, 235 * k, 110 * k, 40 * k] }, // title block + adjoining quads
  ]);
  expect(a[0]).toBeGreaterThan(2000);
  expect(a[1]).toBeGreaterThan(300);
  expect(a[2]).toBeGreaterThan(300);
  expect(a[3]).toBeGreaterThan(20);
  expect(a[4]).toBeGreaterThan(150);
  expect(a[5]).toBeGreaterThan(300);
  expect(a[6]).toBeGreaterThan(5);
  expect(a[7]).toBeGreaterThan(200);
  expect(H).toBeGreaterThan(200);
});

test("TU-2 the modern look changes the built-up tint and the road fill; the military sheet draws its overlay and legend", async ({ app }) => {
  await baseLayers(app, -87.5, 36.6, 0.18, 0.07);
  const feats = {
    imp: [feature(poly([[-87.62, 36.62], [-87.55, 36.64], [-87.52, 36.58], [-87.6, 36.56]]))],
    dz: [feature(rect(-87.52, 36.53, -87.48, 36.56))],
    train: [feature(rect(-87.55, 36.5, -87.35, 36.55))],
    lz: [feature(rect(-87.42, 36.6, -87.39, 36.62))],
    fire: [feature({ type: "Point", coordinates: [-87.5, 36.6] }), feature({ type: "Point", coordinates: [-87.48, 36.57] })],
    cant: [feature(rect(-87.5, 36.63, -87.4, 36.67))],
  };
  await addLayer(app, "North Impact Area", feats.imp);
  await addLayer(app, "Drop Zone", feats.dz);
  await addLayer(app, "Training Areas", feats.train);
  await addLayer(app, "Landing Zone", feats.lz);
  await addLayer(app, "Firing points", feats.fire);
  await addLayer(app, "Cantonment", feats.cant);
  const r = await applySymbols(app, "classic");
  const symbols = r.assigned.map((x) => x.name);
  for (const n of ["North Impact Area", "Drop Zone", "Training Areas", "Landing Zone", "Firing points", "Cantonment", "Built-up area"]) expect(symbols, n).toContain(n);
  const { png, warnings } = await sheet(app, { center: [-87.5, 36.6], scale: 50000, paper: "a2", landscape: true, units: "metric", datum: "nad83", title: "Fort Test Military Installation Map", date: "2026-01-01", style: "military", era: "classic" });
  expect(warnings.filter((w) => !/scale varies|geographic/.test(w))).toEqual([]);
  fs.writeFileSync(path.join(process.env.E2E_ARTIFACTS ?? os.tmpdir(), "topo-military.png"), Buffer.from(png, "base64"));
  const [[W]] = (await analyse(app, png, [{ size: true }])) as [number[]][];
  const k = W / 594;
  const frame: [number, number, number, number] = [18 * k, 24 * k, 560 * k, 324 * k];
  const a = await analyse(app, png, [
    count([242, 169, 169], frame, 40), // impact area pink (with its 55 % opacity over white it is lighter: allow tolerance)
    count([200, 32, 47], frame, 40), // impact outline, drop zone, firing points: red
    count([205, 231, 187], frame, 40), // training area
    count([122, 122, 122], frame, 40), // landing zone grey
    count([159, 185, 204], frame, 40), // cantonment
    count([242, 169, 169], [8 * k, 372 * k, 180 * k, 48 * k], 60), // the legend's impact-area swatch
    { nonwhite: [8 * k, 372 * k, 580 * k, 48 * k] },
  ]);
  expect(a[0]).toBeGreaterThan(500);
  expect(a[1]).toBeGreaterThan(100);
  expect(a[2]).toBeGreaterThan(500);
  expect(a[3]).toBeGreaterThan(50);
  expect(a[4]).toBeGreaterThan(200);
  expect(a[5]).toBeGreaterThan(10);
  expect(a[6]).toBeGreaterThan(1000);
});

test("TU-3 the Topo Map Builder offers the new sheet styles and the Symbology pane the symbol picker", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /Topo Map/ }).click();
  const dlg = app.getByRole("dialog", { name: "Topo Map Builder" });
  await dlg.getByRole("combobox", { name: "Preset" }).click();
  await app.getByRole("option", { name: "USGS quadrangle 1:24,000 (classic look) Tabloid" }).click();
  await expect(dlg.getByRole("combobox", { name: "Sheet style" })).toContainText(/USGS quadrangle/);
  await dlg.getByRole("combobox", { name: "Area" }).click();
  await app.getByRole("option", { name: "A position I type" }).click();
  await dlg.getByRole("textbox", { name: "Position" }).fill("45.0, -93.0");
  await dlg.getByRole("button", { name: "Build the sheet" }).click();
  await expect(dlg.getByTestId("topo-checks")).toBeVisible();
  await expect(dlg.getByRole("button", { name: /4\. Apply topographic symbols/ })).toBeVisible();
  const doc = (await gis(app, (g) => g.useLayouts.getState().doc)) as any;
  expect(doc.elements.find((e: any) => e.id === "block").text).toContain("7.5-MINUTE SERIES (TOPOGRAPHIC)");
  const legend = doc.elements.find((e: any) => e.kind === "legend");
  expect(legend.auto).toBe(false);
  expect(legend.custom.map((c: any) => c.symbol)).toEqual(["road_primary", "road_secondary", "road_light_paved", "road_unimproved", "road_trail", "rail_standard"]);
  await dlg.getByRole("button", { name: "Close" }).click();

  // The symbol picker in a line layer's Symbology.
  const file = path.join(dir, "roads.geojson");
  fs.writeFileSync(file, JSON.stringify({ type: "FeatureCollection", features: [feature(line([[-93.05, 45.0], [-92.95, 45.02]]))] }));
  const layer = (await gis(app, async (g, a: any) => { const [l] = await g.api.importFiles([a.file]); g.useApp.getState().addLayers([l]); g.useApp.getState().setProperties({ kind: "layer", id: l.id }); return l; }, { file })) as any;
  await app.getByText("Topographic symbol (USGS sheet, military overlay)").click();
  await app.getByRole("combobox", { name: "Symbol", exact: true }).click();
  await app.getByRole("option", { name: /Primary highway/ }).click();
  const st = (await gis(app, (g, a: any) => g.useApp.getState().styles[a.id], { id: layer.id })) as any;
  expect(st.symbol).toBe("road_primary");
  expect(st.strokes.map((s: any) => s.color)).toEqual(["#000000", "#c8202f"]);
  await app.getByRole("button", { name: "Remove symbol" }).click();
  expect(((await gis(app, (g, a: any) => g.useApp.getState().styles[a.id], { id: layer.id })) as any).strokes).toBeUndefined();
});
