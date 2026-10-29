// The Symbology and Labels node restyles the layer its tree belongs to. Cases SY-* of docs/TEST_PLAN.md.
import { test, expect, gis, addTestLayer, addNode, frameAll, connect, nodeOf, hoverCanvas } from "./fixtures";

test("SY-1 Symbology and Labels node: after a run the layer takes its colour and labels", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 20);
  const sym = await addNode(app, "symbology", "symbology");
  await frameAll(app);
  const inId = (await nodeOf(app, "input_layer")).id, outId = (await nodeOf(app, "layer_output")).id;
  await connect(app, inId, sym);
  await connect(app, sym, outId);
  await gis(app, (g, a: any) => { const m = g.useModel.getState(); m.setParam(a.sym, "fill", "#336699"); m.setParam(a.sym, "label", true); m.setParam(a.sym, "label_field", "class"); m.setParam(a.sym, "label_size", 11); }, { sym });
  await hoverCanvas(app);
  await app.keyboard.press("ControlOrMeta+Enter");
  await expect.poll(() => gis(app, (g, id) => g.useApp.getState().styles[id].fill, layer.id), { timeout: 30_000 }).toBe("#336699");
  const label = await gis(app, (g, id) => g.useApp.getState().styles[id].label, layer.id);
  expect(label).toMatchObject({ enabled: true, field: "class", size: 11 });
});
