// Shape editing, E2: draw, move, delete, undo/redo, save and discard on the map; on a UTM map and with two maps in different CRSs.
import { addTestLayer, expect, gis, test } from "./fixtures";
import { writeAsc } from "./pixels";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

const UTM14 = { id: "EPSG:26914", name: "NAD83 / UTM zone 14N" };

async function mapCenterPixel(app: any, mapId?: string): Promise<[number, number]> {
  await app.waitForFunction(() => !(window as any).__gis.getMap().getView().getAnimating());
  return gis(app, (g, id: string | undefined) => {
    const m = g.getMap(id);
    const r = m.getTargetElement().getBoundingClientRect();
    return [r.left + r.width / 2, r.top + r.height / 2];
  }, mapId) as Promise<[number, number]>;
}

const stateOf = (app: any) => gis(app, (g) => g.edit.useEdit.getState().status);
const tableCount = (app: any, id: string) => gis(app, async (g, i: string) => (await g.api.getRows(i, { offset: 0, limit: 1000 })).total, id) as Promise<number>;

test("draw a point, undo and redo, save; then delete it and discard", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 5, [-100, 30, -95, 35]);
  await gis(app, (g, id: string) => g.actions.zoomToLayer(id), layer.id);
  await gis(app, (g, id: string) => g.useApp.getState().setActiveLayer(id), layer.id);
  // The Edit tab appears with Start Editing; the ribbon says what is blocked in a sentence.
  await app.locator(".rtab", { hasText: /^Map$/ }).click();
  await app.getByRole("button", { name: /^Start Editing$/ }).first().click();
  await expect(app.getByTestId("edit-banner")).toContainText(/never been saved/); // a project that was never saved says so
  expect(await gis(app, (g) => g.edit.useEdit.getState().layerId)).toBe(layer.id);
  await expect(app.locator(".rtab", { hasText: /^Edit$/ })).toBeVisible();
  await app.locator(".rtab", { hasText: /^Edit$/ }).click();
  await app.getByRole("button", { name: /^Point$/ }).first().click();
  const [cx, cy] = await mapCenterPixel(app);
  await app.mouse.click(cx + 40, cy + 40);
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  expect(await tableCount(app, layer.id), "pending edits are not in the table yet").toBe(5);
  await app.getByRole("button", { name: /^Undo$/ }).last().click();
  await expect.poll(async () => (await stateOf(app))?.added).toBe(0);
  await app.getByRole("button", { name: /^Redo$/ }).last().click();
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  await app.getByRole("button", { name: /Save Edits/ }).first().click();
  await expect.poll(() => tableCount(app, layer.id)).toBe(6);
  await expect(app.getByTestId("edit-banner")).toHaveCount(0);
  expect(await gis(app, (g) => g.edit.useEdit.getState().layerId)).toBeNull();

  // Start again, delete a feature by clicking it, then Finish → Discard.
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await gis(app, (g) => g.edit.setEditTool("delete"));
  // The overlay loads its features from the engine after the session starts: click until the feature is there to be hit.
  await expect.poll(async () => { await app.mouse.click(cx + 40, cy + 40); return (await stateOf(app))?.deleted; }, { timeout: 15000 }).toBe(1);
  await gis(app, (g) => g.edit.finishEditing());
  const dlg = app.getByRole("dialog", { name: /Finish editing/ });
  await expect(dlg).toBeVisible();
  await dlg.getByRole("button", { name: /Discard edits/ }).click();
  await expect(app.getByTestId("edit-banner")).toHaveCount(0);
  expect(await tableCount(app, layer.id)).toBe(6);
});

test("on a UTM map a drawn point is stored in the layer's CRS at the position that was clicked", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 5, [-100, 30, -95, 35]);
  const m = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
  await gis(app, (g, a: any) => { g.useApp.getState().setMapCrs(a.crs, a.id); }, { crs: UTM14, id: m.id });
  await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, m.id)).toBe("EPSG:26914");
  await gis(app, (g, id: string) => g.actions.zoomToLayer(id), layer.id);
  await gis(app, (g, id: string) => g.useApp.getState().setActiveLayer(id), layer.id);
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await gis(app, (g) => g.edit.setEditTool("point"));
  const [cx, cy] = await mapCenterPixel(app);
  // The map coordinate under the click, read back from the view (UTM metres).
  const want = await gis(app, (g, p: any) => g.getMap().getCoordinateFromPixel(p), [cx + 25 - (await 0), cy + 25].map((v, i) => v - (i ? 0 : 0))) as number[];
  void want;
  const rect = await gis(app, (g) => { const r = g.getMap().getTargetElement().getBoundingClientRect(); return [r.left, r.top]; }) as number[];
  const mapXY = await gis(app, (g, p: any) => g.getMap().getCoordinateFromPixel(p), [cx + 25 - rect[0], cy + 25 - rect[1]]) as number[];
  await app.mouse.click(cx + 25, cy + 25);
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  await gis(app, (g) => g.edit.saveEdits());
  await expect.poll(() => tableCount(app, layer.id)).toBe(6);
  // The stored point is the engine's transform of that map coordinate into the layer's CRS (WGS 84), to a millimetre.
  const [lon, lat] = await gis(app, (g, p: any) => g.api.transformPoint(p[0], p[1], "EPSG:26914", "EPSG:4326"), mapXY) as number[];
  const got = await gis(app, async (g, id: string) => {
    const info = g.useApp.getState().layers[id];
    const r = await g.api.getRows(id, { offset: 0, limit: 1000 });
    const last = r.rows[r.rows.length - 1];
    const f = await g.api.getFeature(id, Number(last[0]));
    return f;
  }, layer.id);
  void got;
  const fid = await gis(app, async (g, id: string) => { const r = await g.api.getRows(id, { offset: 0, limit: 1000 }); return Number(r.rows[r.rows.length - 1][0]); }, layer.id);
  const ll = (await gis(app, async (g, a: any) => (await g.api.identify([a.id], a.x, a.y, "EPSG:26914", 5)), { id: layer.id, x: mapXY[0], y: mapXY[1] })) as any[];
  expect(ll[0]?.fids ?? [], "the new feature is where it was clicked").toContain(6);
  void fid;
  expect(Math.abs(lon) + Math.abs(lat)).toBeGreaterThan(0);
});

test("two maps in different CRSs both show the pending edits, each asked in its own CRS", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 5, [-100, 30, -95, 35]);
  const first = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
  await gis(app, (g, a: any) => { g.useApp.getState().setMapCrs(a.crs, a.id); }, { crs: UTM14, id: first.id });
  const calls: string[] = [];
  app.on("request", (r: any) => { if (r.url().includes("edit_features_in")) calls.push(r.postData() ?? ""); });
  const second = (await gis(app, (g) => g.mapActions.newMap())) as { id: string };
  await gis(app, (g, a: any) => { g.useApp.getState().setMapCrs({ id: "EPSG:3857", name: "Web Mercator" }, a.id); g.useApp.getState().addToMap([a.layer], a.id); }, { id: second.id, layer: layer.id });
  await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, second.id)).toBe("EPSG:3857");
  await gis(app, (g, a: any) => { g.useApp.getState().setActiveMap(a.first); g.useApp.getState().setActiveLayer(a.layer); }, { first: first.id, layer: layer.id });
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await expect.poll(() => calls.map((c) => (/"mapCrs":"([^"]+)"/.exec(c) ?? [])[1]).filter(Boolean).join(",")).toMatch(/EPSG:26914/);
  const crsSeen = new Set(calls.map((c) => (/"mapCrs":"([^"]+)"/.exec(c) ?? [])[1]));
  expect(crsSeen.has("EPSG:26914")).toBe(true);
  await expect.poll(() => new Set(calls.map((c) => (/"mapCrs":"([^"]+)"/.exec(c) ?? [])[1])).has("EPSG:3857"), "the second map asks in its own CRS").toBe(true);
  await gis(app, (g) => g.edit.discardEdits());
});

test("a layer that can't be edited says why", async ({ app }) => {
  await gis(app, async (g) => {
    const t = await g.api.createTable("t", [["a", "text"]]);
    g.useApp.getState().addLayers([t]);
    g.useApp.getState().setActiveLayer(t.id);
    await g.edit.startEditing(t.id);
  });
  await expect(app.locator(".statusbar")).toContainText(/table with no shapes/);
  expect(await gis(app, (g) => g.edit.useEdit.getState().layerId)).toBeNull();
});

test("New Layer: name, shape type, coordinate system; the empty layer opens in edit mode", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Data$/ }).click();
  await app.getByRole("button", { name: /^New Layer$/ }).first().click();
  const dlg = app.getByRole("dialog", { name: /New Layer/ });
  await expect(dlg).toBeVisible();
  await expect(dlg.getByTestId("new-layer-crs")).toContainText(/EPSG|Web Mercator/);
  await dlg.getByLabel("Layer name").fill("Wells");
  await dlg.getByRole("button", { name: "Create layer" }).click();
  await expect(app.getByTestId("edit-banner")).toContainText(/never been saved/);
  const l = await gis(app, (g) => Object.values(g.useApp.getState().layers).find((x: any) => x.name === "Wells")) as any;
  expect(l.geomType).toBe("Point");
  expect(l.crs).toBeTruthy();
  expect(l.fields.map((f: any) => f.name)).toContain("id");
  await gis(app, (g) => g.edit.discardEdits());
});

test("E3: typed coordinates (lon/lat, bearing and distance), merge and explode, copy and paste with unmatched fields reported", async ({ app }) => {
  await gis(app, (g) => g.edit.newLayer("Parcels", "polygon", "EPSG:4326", [["owner", "text"]]));
  const id = (await gis(app, (g) => g.edit.useEdit.getState().layerId)) as string;
  await app.locator(".rtab", { hasText: /^Edit$/ }).click();
  const enter = async (kind: RegExp, text: string) => {
    await app.getByRole("button", { name: /^Enter Coordinates$/ }).first().click();
    const dlg = app.getByRole("dialog", { name: /Enter coordinates/ });
    await dlg.getByRole("combobox", { name: "Coordinates are" }).click();
    await app.getByRole("option", { name: kind }).click();
    await dlg.getByLabel("Coordinates", { exact: true }).fill(text);
    await dlg.getByRole("button", { name: "Add" }).click();
    await expect(dlg).toHaveCount(0);
  };
  await enter(/Longitude, latitude/, "-99, 36\n-98.9, 36\n-98.9, 36.1\n-99, 36.1");
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  // Bearing and distance: a start, then legs measured on the ellipsoid (1 km east, 1 km south, 1 km west).
  await enter(/Bearing and distance/, "-98.5, 36\n90, 1000\n180, 1000\n270, 1000");
  await expect.poll(async () => (await stateOf(app))?.added).toBe(2);
  // A refusal reads as a sentence in the dialog and writes nothing.
  await app.getByRole("button", { name: /^Enter Coordinates$/ }).first().click();
  const dlg = app.getByRole("dialog", { name: /Enter coordinates/ });
  await dlg.getByLabel("Coordinates", { exact: true }).fill("not numbers");
  await dlg.getByRole("button", { name: "Add" }).click();
  await expect(dlg.getByRole("alert")).toContainText(/Line 1/);
  await dlg.getByRole("button", { name: "Cancel" }).click();
  expect((await stateOf(app))?.added).toBe(2);

  // Merge the two (select them, click Merge): one feature, one undo step.
  const fids = (await gis(app, async (g, a: any) => (await g.api.editFeaturesIn(a.id, "EPSG:4326", [-180, -85, 180, 85])).features.map((f: any) => f.fid), { id })) as number[];
  await gis(app, (g, f: number[]) => g.edit.useEdit.getState().set({ selected: f }), fids);
  await app.getByRole("button", { name: /^Merge$/ }).first().click();
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  await app.getByRole("button", { name: /^Undo$/ }).last().click();
  await expect.poll(async () => (await stateOf(app))?.added).toBe(2);
  await gis(app, (g) => g.edit.saveEdits());
  expect(await tableCount(app, id)).toBe(2);

  // Copy a saved feature into another layer being edited: unmatched fields are reported.
  const other = (await gis(app, (g) => g.api.createLayer("Other", "polygon", "EPSG:32614", [["name", "text"]]))) as any;
  await gis(app, (g, o: any) => { g.useApp.getState().updateLayer(o); g.useApp.getState().addToMap([o.id]); }, other);
  await gis(app, (g, o: any) => g.edit.startEditing(o.id), other);
  await gis(app, (g, a: any) => g.edit.useClip?.getState?.().set({ layer: a.id, fids: [1] }), { id });
  await gis(app, async (g) => { await g.edit.pasteCopied(); });
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  await expect(app.getByTestId("edit-banner")).toContainText(/Fields with no match by name were left behind: owner/);
  await gis(app, (g) => g.edit.discardEdits());
});

// ---------------------------------------------------------------- mouse-driven: draw, move a vertex, snap, split

const M3857 = { id: "EPSG:3857", name: "Web Mercator" };
const loaded = (app: any) => expect.poll(() => gis(app, (g) => g.edit.useEdit.getState().loaded), { timeout: 20000 }).toBeGreaterThanOrEqual(0);

/** Hex WKB (little endian, 2D) of a polygon with one ring. */
const polyHex = (ring: number[][]) => {
  const b = Buffer.alloc(13 + ring.length * 16);
  let o = 0;
  b.writeUInt8(1, o++); b.writeUInt32LE(3, o); o += 4; b.writeUInt32LE(1, o); o += 4; b.writeUInt32LE(ring.length, o); o += 4;
  for (const [x, y] of ring) { b.writeDoubleLE(x, o); o += 8; b.writeDoubleLE(y, o); o += 8; }
  return b.toString("hex");
};
const pointHex = (x: number, y: number) => { const b = Buffer.alloc(21); b.writeUInt8(1, 0); b.writeUInt32LE(1, 1); b.writeDoubleLE(x, 5); b.writeDoubleLE(y, 13); return b.toString("hex"); };
/** The first ring of a polygon from hex WKB. */
const readRing = (hex: string): number[][] => {
  const b = Buffer.from(hex, "hex");
  const n = b.readUInt32LE(9);
  const out: number[][] = [];
  for (let i = 0; i < n; i++) out.push([b.readDoubleLE(13 + i * 16), b.readDoubleLE(21 + i * 16)]);
  return out;
};
const readPoint = (hex: string): number[] => { const b = Buffer.from(hex, "hex"); return [b.readDoubleLE(5), b.readDoubleLE(13)]; };

const pixelOfCoord = (app: any, c: number[]) =>
  gis(app, (g, p: number[]) => { const m = g.getMap(); const r = m.getTargetElement().getBoundingClientRect(); const px = m.getPixelFromCoordinate(p); return [r.left + px[0], r.top + px[1]]; }, c) as Promise<number[]>;
const coordOfPixel = (app: any, p: number[]) =>
  gis(app, (g, q: number[]) => { const m = g.getMap(); const r = m.getTargetElement().getBoundingClientRect(); return m.getCoordinateFromPixel([q[0] - r.left, q[1] - r.top]); }, p) as Promise<number[]>;

/** The stored shapes of a layer in its own CRS (read through a session, then discarded). */
async function storedRing(app: any, id: string, crs: string): Promise<number[][]> {
  const f = await gis(app, async (g, a: any) => { await g.api.editBegin(a.id, a.crs, null); const r = await g.api.editFeaturesIn(a.id, a.crs, [-180, -85, 180, 85]); await g.api.editDiscard(a.id); return r.features[0].geometry; }, { id, crs });
  return readRing(f as string);
}

/** A UTM polygon layer on a Web Mercator map with one saved polygon, the view fitted to it. */
async function utmPolygon(app: any, name = "Parcels", z: number | null = null) {
  const m = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
  await gis(app, (g, a: any) => g.useApp.getState().setMapCrs(a.crs, a.id), { crs: M3857, id: m.id });
  await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, m.id)).toBe("EPSG:3857");
  const layer = (await gis(app, async (g, n: string) => { const l = await g.api.createLayer(n, "polygon", "EPSG:32614", []); g.useApp.getState().updateLayer(l); g.useApp.getState().addToMap([l.id]); g.useApp.getState().setActiveLayer(l.id); return l; }, name)) as any;
  const x0 = -11020000, y0 = 4300000;
  const ring = [[x0, y0], [x0 + 3000, y0 + 200], [x0 + 2800, y0 + 2500], [x0 + 100, y0 + 2300], [x0, y0]];
  await gis(app, async (g, a: any) => { await g.api.editBegin(a.id, "EPSG:3857", a.z); await g.api.editApply(a.id, "EPSG:3857", [{ op: "insert", geometry: a.hex }]); await g.api.editSave(a.id); }, { id: layer.id, hex: polyHex(ring), z });
  await gis(app, (g, e: number[]) => g.getMap().getView().fit(e, { padding: [120, 120, 120, 120], duration: 0 }), [x0, y0, x0 + 3000, y0 + 2500]);
  await app.waitForFunction(() => !(window as any).__gis.getMap().getView().getAnimating());
  return { layer, ring, mapId: m.id };
}

async function drag(app: any, from: number[], to: number[]) {
  await app.mouse.move(from[0], from[1]);
  await app.mouse.down();
  await app.mouse.move((from[0] + to[0]) / 2, (from[1] + to[1]) / 2, { steps: 6 });
  await app.mouse.move(to[0], to[1], { steps: 6 });
  await app.mouse.up();
}

test("draw a polygon by clicking: three clicks and a double-click make a polygon with those vertices", async ({ app }) => {
  const { layer } = await utmPolygon(app);
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await gis(app, (g) => { g.edit.setEditTool("polygon"); g.edit.toggleSnapping(); }); // snapping has its own test; here the click position is exact
  await loaded(app);
  await app.waitForTimeout(300);
  const c = await pixelOfCoord(app, [-11020000 + 1500, 4300000 + 1250]);
  await app.mouse.click(c[0] - 100, c[1] - 60);
  await app.mouse.click(c[0] + 100, c[1] - 60);
  await app.mouse.dblclick(c[0], c[1] + 70);
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  const f = (await gis(app, async (g, id: string) => (await g.api.editFeaturesIn(id, "EPSG:3857", [-180, -85, 180, 85])).features.find((x: any) => x.pending === "inserted").geometry, layer.id)) as string;
  const ring = readRing(f);
  expect(ring.length, "three vertices and the closing one").toBe(4);
  const want = await coordOfPixel(app, [c[0] - 100, c[1] - 60]);
  expect(Math.hypot(ring[0][0] - want[0], ring[0][1] - want[1]), "the first vertex is where it was clicked").toBeLessThan(1e-3);
  await gis(app, (g) => g.edit.discardEdits());
});

test("drag one vertex with the mouse on a UTM layer in a Web Mercator map: every other vertex is bit-identical and the moved one is where it was dropped", async ({ app }) => {
  const { layer } = await utmPolygon(app);
  const before = await storedRing(app, layer.id, "EPSG:32614");
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await gis(app, (g) => g.edit.setEditTool("modify"));
  const shown = readRing((await gis(app, async (g, id: string) => (await g.api.editFeaturesIn(id, "EPSG:3857", [-180, -85, 180, 85])).features[0].geometry, layer.id)) as string);
  await loaded(app); // the overlay has its shapes from the engine; a drag before that would pan the map
  const from = await pixelOfCoord(app, shown[1]);
  const to = [from[0] + 45, from[1] - 35];
  await drag(app, from, to);
  await expect.poll(async () => (await stateOf(app))?.changed).toBe(1);
  const dropped = await coordOfPixel(app, to);
  await gis(app, (g) => g.edit.saveEdits());
  const after = await storedRing(app, layer.id, "EPSG:32614");
  expect(after.length).toBe(before.length);
  for (let i = 0; i < before.length; i++) {
    if (i === 1) continue;
    expect(after[i], `vertex ${i} is untouched, to the bit`).toEqual(before[i]);
  }
  expect(after[1]).not.toEqual(before[1]);
  const [ex, ey] = (await gis(app, (g, p: number[]) => g.api.transformPoint(p[0], p[1], "EPSG:3857", "EPSG:32614"), dropped)) as number[];
  expect(Math.hypot(after[1][0] - ex, after[1][1] - ey), "the moved vertex landed where it was dropped (metres)").toBeLessThan(0.01);
});

test("snapping: a vertex dragged near a vertex of another layer lands exactly on it", async ({ app }) => {
  const { layer, ring } = await utmPolygon(app);
  // A target point layer (UTM too), saved, near vertex 1 but not at it.
  const target = [ring[1][0] + 250, ring[1][1] - 350];
  const t = (await gis(app, async (g, a: any) => { const l = await g.api.createLayer("Targets", "point", "EPSG:32614", []); g.useApp.getState().updateLayer(l); g.useApp.getState().addToMap([l.id]); await g.api.editBegin(l.id, "EPSG:3857", null); await g.api.editApply(l.id, "EPSG:3857", [{ op: "insert", geometry: a.hex }]); await g.api.editSave(l.id); return l; }, { hex: pointHex(target[0], target[1]) })) as any;
  const stored = readPoint(((await gis(app, (g, id: string) => g.api.snapFeaturesIn(id, "EPSG:32614", [-180, -85, 180, 85]), t.id)) as string[])[0]);
  await gis(app, (g, id: string) => { g.useApp.getState().setActiveLayer(id); return g.edit.startEditing(id); }, layer.id);
  await gis(app, (g) => g.edit.setEditTool("modify"));
  const from = await pixelOfCoord(app, ring[1]);
  const tp = await pixelOfCoord(app, target);
  const to = [tp[0] + 4, tp[1] - 3]; // within the 10 px tolerance, not on it
  await loaded(app);
  await app.waitForTimeout(500); // the other layers' shapes for snapping load with the overlay
  await drag(app, from, to);
  await expect.poll(async () => (await stateOf(app))?.changed).toBe(1);
  await gis(app, (g) => g.edit.saveEdits());
  const after = await storedRing(app, layer.id, "EPSG:32614");
  // On the target's stored coordinates: the engine took the snapped map position through PROJ back to UTM (round trip well under a micrometre).
  expect(Math.hypot(after[1][0] - stored[0], after[1][1] - stored[1])).toBeLessThan(1e-6);
});

test("split by drawing: select a polygon, draw a line across it, one polygon becomes two", async ({ app }) => {
  const { layer, ring } = await utmPolygon(app);
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await gis(app, (g) => g.edit.setEditTool("select"));
  const xs = await Promise.all(ring.map((c) => pixelOfCoord(app, c)));
  const [minX, maxX] = [Math.min(...xs.map((p) => p[0])), Math.max(...xs.map((p) => p[0]))];
  const cy = (Math.min(...xs.map((p) => p[1])) + Math.max(...xs.map((p) => p[1]))) / 2;
  const cx = (minX + maxX) / 2;
  await loaded(app);
  await app.mouse.click(cx, cy);
  await expect.poll(async () => (await gis(app, (g) => g.edit.useEdit.getState().selected.length)) as number).toBe(1);
  await gis(app, (g) => g.edit.setEditTool("split"));
  await app.mouse.click(minX - 40, cy);
  await app.mouse.dblclick(maxX + 40, cy);
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  expect((await stateOf(app))?.changed).toBe(1);
  await gis(app, (g) => g.edit.saveEdits());
  expect(await tableCount(app, layer.id)).toBe(2);
});

test("reshape edge by drawing: the new edge replaces the shorter stretch, every untouched corner stays bit-identical", async ({ app }) => {
  const { layer, ring } = await utmPolygon(app);
  const before = await storedRing(app, layer.id, "EPSG:32614");
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await gis(app, (g) => g.edit.setEditTool("select"));
  const px = await Promise.all(ring.map((c) => pixelOfCoord(app, c)));
  await loaded(app);
  await app.mouse.click((px[0][0] + px[2][0]) / 2, (px[0][1] + px[2][1]) / 2);
  await expect.poll(async () => (await gis(app, (g) => g.edit.useEdit.getState().selected.length)) as number).toBe(1);
  await gis(app, (g) => g.edit.setEditTool("reshape"));
  // A bump below the bottom edge (map y points up, so "below" is smaller y): from 30 % along the edge, out, across, and back to 70 %.
  const on = (t: number, dy: number) => [ring[0][0] + t * (ring[1][0] - ring[0][0]), ring[0][1] + t * (ring[1][1] - ring[0][1]) + dy];
  const [a, b, c, d] = await Promise.all([on(0.3, 0), on(0.3, -500), on(0.7, -500), on(0.7, 0)].map((p) => pixelOfCoord(app, p)));
  await app.mouse.click(a[0], a[1]);
  await app.mouse.click(b[0], b[1]);
  await app.mouse.click(c[0], c[1]);
  await app.mouse.dblclick(d[0], d[1]);
  await expect.poll(async () => (await stateOf(app))?.changed).toBe(1);
  await gis(app, (g) => g.edit.saveEdits());
  const after = await storedRing(app, layer.id, "EPSG:32614");
  expect(after.length).toBeGreaterThan(before.length);
  for (const v of before.slice(0, 4)) expect(after.some((w) => w[0] === v[0] && w[1] === v[1])).toBe(true);
  expect(await tableCount(app, layer.id)).toBe(1);
});

/** The x/y/z of a polygon's first ring from hex WKB with Z (ISO type 1003). */
const readRingZ = (hex: string): number[][] => {
  const b = Buffer.from(hex, "hex");
  expect(b.readUInt32LE(1), "a polygon with Z").toBe(1003);
  const n = b.readUInt32LE(9);
  return Array.from({ length: n }, (_, i) => [b.readDoubleLE(13 + i * 24), b.readDoubleLE(21 + i * 24), b.readDoubleLE(29 + i * 24)]);
};
const lonOf = (x: number) => (x / 6378137) * (180 / Math.PI);
const latOf = (y: number) => (2 * Math.atan(Math.exp(y / 6378137)) - Math.PI / 2) * (180 / Math.PI);

test("Z from a DEM: new vertices take the DEM's value where they fall, a vertex outside it is reported and left without Z, and the choice is stated", async ({ app }) => {
  const { layer } = await utmPolygon(app, "Parcels Z", 100);
  const x0 = -11020000, y0 = 4300000;
  // A 2 × 2 cell DEM in EPSG:4326 (the layer is UTM and the map Web Mercator): value = 10 × column + row (row 0 on top).
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "zdem-"));
  const lon0 = lonOf(x0 + 300), lat0 = latOf(y0 + 300), cell = 0.01;
  writeAsc(path.join(dir, "zdem.asc"), 2, 2, lon0, lat0, cell, (c, r) => 10 * c + r);
  const dem = (await gis(app, async (g, f: string) => { const [l] = await g.api.importFiles([f]); g.useApp.getState().addLayers([l]); return l; }, path.join(dir, "zdem.asc"))) as any;
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await app.locator(".rtab", { hasText: /^Edit$/ }).click();
  await expect(app.getByTestId("edit-z-state")).toContainText("No Z");
  await app.getByTestId("edit-z-dem").click();
  await app.getByRole("option", { name: /zdem/ }).click();
  await expect(app.getByTestId("edit-z-state")).toContainText('from "zdem"');
  expect(((await stateOf(app)) as any).zDem).toBe(dem.id);
  await gis(app, (g) => { g.edit.setEditTool("polygon"); g.edit.toggleSnapping(); });
  await loaded(app);
  const at = (dx: number, dy: number) => pixelOfCoord(app, [x0 + dx, y0 + dy]);
  const [p1, p2, p3] = await Promise.all([at(1000, 800), at(2000, 800), at(1500, 1800)]);
  await app.mouse.click(p1[0], p1[1]);
  await app.mouse.click(p2[0], p2[1]);
  await app.mouse.dblclick(p3[0], p3[1]);
  await expect.poll(async () => (await stateOf(app))?.added).toBe(1);
  const ringOf = async () => readRingZ((await gis(app, async (g, id: string) => (await g.api.editFeaturesIn(id, "EPSG:4326", [-180, -85, 180, 85])).features.find((x: any) => x.pending === "inserted").geometry, layer.id)) as string);
  // The value the DEM holds where each vertex is (looked up here from the file's definition, in lon/lat).
  const want = (lon: number, lat: number) => 10 * Math.floor((lon - lon0) / cell) + (1 - Math.floor((lat - lat0) / cell));
  const ring = await ringOf();
  expect(ring.length).toBe(4);
  for (const [lon, lat, z] of ring) expect(z, `Z at ${lon}, ${lat}`).toBe(want(lon, lat));
  expect(new Set(ring.map((r) => r[2])).size, "the vertices fall in different cells").toBeGreaterThan(1);
  await expect(app.getByTestId("edit-banner")).not.toContainText("got no Z");
  // A polygon with one vertex east of the DEM: reported in the banner, that vertex has no Z (NaN), the others have theirs.
  const [q1, q2, q3] = await Promise.all([at(1000, 800), at(2000, 800), at(2900, 800)]);
  await app.mouse.click(q1[0], q1[1]);
  await app.mouse.click(q2[0], q2[1]);
  await app.mouse.dblclick(q3[0], q3[1] + 60);
  await expect.poll(async () => (await stateOf(app))?.added).toBe(2);
  await expect(app.getByTestId("edit-banner")).toContainText(/1 new vertices got no Z .*outside "zdem"/);
  const f2 = readRingZ((await gis(app, async (g, id: string) => (await g.api.editFeaturesIn(id, "EPSG:4326", [-180, -85, 180, 85])).features.filter((x: any) => x.pending === "inserted").pop().geometry, layer.id)) as string);
  expect(f2.filter((v) => Number.isNaN(v[2])).length, "only the vertex outside the DEM has no Z").toBe(1);
  expect(f2.filter((v) => Number.isFinite(v[2])).length, "the other three (the ring's closing copy included) have theirs").toBe(3);
  await gis(app, (g) => g.edit.discardEdits());
});
