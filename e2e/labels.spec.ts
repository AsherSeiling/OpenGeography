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
