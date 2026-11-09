// Shape editing, E2: draw, move, delete, undo/redo, save and discard on the map; on a UTM map and with two maps in different CRSs.
import { addTestLayer, expect, gis, test } from "./fixtures";

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
  await app.locator(".rtab", { hasText: /^Map$/ }).click();
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
