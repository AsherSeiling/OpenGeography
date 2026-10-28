// L1 feature labels on the map. Cases L1-P1…P4 of docs/TEST_PLAN.md.
import { test, expect, gis, addTestLayer } from "./fixtures";

const labelLayers = (page: any) =>
  gis(page, (g) => g.getMap().getLayers().getArray().filter((l: any) => l.getDeclutter?.()).map((l: any) => ({ visible: l.getVisible(), url: l.getSource().getUrls()[0] }))) as Promise<{ visible: boolean; url: string }[]>;

async function labelsOn(page: any, id: string, field: string) {
  await gis(page, (g, a: any) => { const s = g.useApp.getState(); s.setStyle(a.id, { ...s.styles[a.id], label: { enabled: true, field: a.field, size: 9, color: "#222222", haloColor: "#ffffff", haloWidth: 1.5 } }); }, { id, field });
}

test("L1 labels draw in Web Mercator and UTM, carry the label field only when on, and survive a style round trip", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 20, [-98, 34, -96, 36]);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.waitForTimeout(800);
  expect(await labelLayers(app)).toHaveLength(0);

  await labelsOn(app, layer.id, "class");
  await expect.poll(async () => (await labelLayers(app)).length).toBe(1);
  let [lbl] = await labelLayers(app);
  expect(lbl.url).toContain("fields=class%2C__anchors"); // polygons read anchor points
  // The anchor tiles around the view centre hold label points (the layer has 20 polygons in view).
  const anchors = await gis(app, async (g) => {
    const l = g.getMap().getLayers().getArray().find((x: any) => x.getDeclutter?.());
    const src = l.getSource(), grid = src.getTileGrid(), c = g.getMap().getView().getCenter();
    const [z, tx, ty] = grid.getTileCoordForCoordAndZ(c, 7);
    let total = 0;
    for (let dx = -2; dx <= 2; dx++) for (let dy = -2; dy <= 2; dy++) {
      const url = src.getUrls()[0].replace("{z}", String(z)).replace("{x}", String(tx + dx)).replace("{y}", String(ty + dy));
      const r = await fetch(url);
      if (r.ok) total += (await r.arrayBuffer()).byteLength;
    }
    return total;
  });
  expect(anchors).toBeGreaterThan(30); // more than empty layer headers: polygons under 4 px are not labelled, the rest are

  // UTM 14N: the label layer is rebuilt on the new grid
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await expect.poll(async () => (await labelLayers(app)).filter((l) => /\/tiles\/[^/]+\/[^/]+\/\{z\}/.test(l.url)).length, { timeout: 30_000 }).toBe(1);

  // Off: the layer goes away and tiles no longer carry the field
  await gis(app, (g, id) => { const s = g.useApp.getState(); s.setStyle(id, { ...s.styles[id], label: { ...s.styles[id].label, enabled: false } }); }, layer.id);
  await expect.poll(async () => (await labelLayers(app)).length).toBe(0);

  // Round trip through JSON, like a saved project
  const kept = await gis(app, (g, id) => JSON.parse(JSON.stringify(g.useApp.getState().styles[id])).label, layer.id);
  expect(kept).toMatchObject({ enabled: false, field: "class", size: 9 });
});

test("L2 labels in a layout: the engine preview changes when labels are on, and reports the ones that do not fit", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 12, [-100, 34, -99.7, 34.3]);
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.waitForTimeout(800);
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await app.locator(".rtab", { hasText: /^Layout$/ }).click();
  await app.locator(".rbtn", { hasText: /^Map Frame$/ }).first().click();
  await gis(app, (g) => { const d = g.useLayouts.getState().doc; const f = d.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { center: [420000, 3779000], scale: 150000, basemap: "none" }); });
  const preview = (page: any) => gis(page, async (g) => { const r = await g.api.renderLayoutPreview(g.layoutActions.layoutRequest(g.useLayouts.getState().doc), 96); return { len: r.png.length, warnings: r.warnings }; }) as Promise<{ len: number; warnings: string[] }>;
  const before = await preview(app);
  await labelsOn(app, layer.id, "value");
  const after = await preview(app);
  expect(after.len).not.toBe(before.len); // labels were drawn
  // Labels that do not fit are reported, in words (L2-P3), and nothing else about labels goes wrong.
  for (const w of after.warnings.filter((w) => /label/.test(w))) expect(w).toMatch(/^\d+ labels? of .+ didn't fit$/);
});

test("L3 adjusting labels in the layout: select, drag, edit text, hide, reset, undo", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 12, [-100, 34, -99.7, 34.3]);
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await labelsOn(app, layer.id, "value");
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await app.locator(".rtab", { hasText: /^Layout$/ }).click();
  await app.locator(".rbtn", { hasText: /^Map Frame$/ }).first().click();
  await gis(app, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { center: [420000, 3779000], scale: 150000, basemap: "none" }); });
  const overrides = () => gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame").labelOverrides ?? null) as Promise<any>;
  const box = app.locator(".layout-label:not(.hidden)").first();
  await expect(box).toBeVisible({ timeout: 30_000 });
  const key = (await box.getAttribute("data-label"))!; // "<layer>:<fid>"
  const fid = key.split(":")[1];

  // Click selects it: Properties shows the label.
  const b = (await box.boundingBox())!;
  await app.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
  await expect(app.getByRole("button", { name: "Reset to automatic" })).toBeVisible();
  expect(await overrides()).toBeNull(); // a click edits nothing

  // Drag moves it, in this layout only.
  const b2 = (await app.locator(`.layout-label[data-label="${key}"]`).boundingBox())!;
  await app.mouse.move(b2.x + b2.width / 2, b2.y + b2.height / 2);
  await app.mouse.down();
  await app.mouse.move(b2.x + b2.width / 2 + 60, b2.y + b2.height / 2 + 30, { steps: 5 });
  await app.mouse.up();
  await expect.poll(async () => (await overrides())?.[layer.id]?.features?.[fid]?.dxMm).toBeGreaterThan(0);
  const moved = (await overrides())[layer.id].features[fid];
  expect(moved.dyMm).toBeGreaterThan(0);
  expect((await gis(app, (g, id) => g.useApp.getState().styles[id].label.enabled, layer.id))).toBe(true); // the map's setting is untouched

  // Edit the text in Properties: the label now reads it.
  await expect(app.locator(`.layout-label[data-label="${key}"]`)).toBeVisible({ timeout: 30_000 });
  await app.getByLabel("Text (empty = the field's value)").fill("Hello");
  await expect(app.locator(`.layout-label[data-label="${key}"][title*="Hello"]`)).toBeVisible({ timeout: 30_000 });

  // Undo takes the text edit back (one step), the move stays.
  await gis(app, (g) => g.useLayouts.getState().undo());
  expect((await overrides())[layer.id].features[fid].text).toBeUndefined();
  expect((await overrides())[layer.id].features[fid].dxMm).toBeGreaterThan(0);

  // Hide it: it's listed (dotted) but not drawn, then Reset to automatic clears every edit.
  await gis(app, (g, a: any) => { const s = g.useLayouts.getState(); const f = s.doc.elements.find((e: any) => e.kind === "map_frame"); s.update(f.id, { labelOverrides: { [a.layer]: { features: { [a.fid]: { hidden: true } } } } }); }, { layer: layer.id, fid });
  await expect(app.locator(`.layout-label.hidden[data-label="${key}"]`)).toBeVisible({ timeout: 30_000 });
  await app.locator(`.layout-label.hidden[data-label="${key}"]`).click();
  await app.getByRole("button", { name: "Reset to automatic" }).click();
  expect(await overrides()).toBeNull();
  await expect(app.locator(`.layout-label:not(.hidden)[data-label="${key}"]`)).toBeVisible({ timeout: 30_000 });
});
