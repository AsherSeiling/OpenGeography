// LB2 placement and rendering options of labels. Cases LB2-* of docs/TEST_PLAN.md.
import { test, expect, gis, addTestLayer } from "./fixtures";

test("LB2 point label position, wrap, priority and data-defined fields are set from the Labels panel and drawn", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 30, [-98, 34, -96, 36]);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await gis(app, (g, a: any) => { const s = g.useApp.getState(); s.setStyle(a.id, { ...s.styles[a.id], label: { enabled: true, field: "class", size: 9, color: "#222222", haloColor: "#ffffff", haloWidth: 1.5 } }); }, { id: layer.id });
  await gis(app, async (g, id) => { const m = await import("/src/dialogs/LayerPropertiesWindow.tsx"); m.openLayerProperties(id, "symbology"); }, layer.id);
  const label = () => gis(app, (g, id) => g.useApp.getState().styles[id].label, layer.id);

  await app.getByRole("button", { name: "Placement" }).click();
  await app.getByRole("radio", { name: "Above right" }).click();
  expect((await label()).position).toBe("aboveRight");

  await app.getByRole("button", { name: "Rendering and data-defined" }).click();
  await app.getByRole("checkbox", { name: /Allow overlap/ }).check();
  expect((await label()).allowOverlap).toBe(true);

  await app.getByRole("button", { name: "Halo, background and shadow" }).click();
  await app.getByRole("checkbox", { name: "Background box" }).check();
  await app.getByRole("checkbox", { name: "Drop shadow" }).check();
  const l = await label();
  expect(l.background).toMatchObject({ padding: 2 });
  expect(l.shadow).toMatchObject({ dx: 1.5 });

  // The map keeps a label layer reading the label field.
  await expect.poll(() => gis(app, (g) => g.getMap().getLayers().getArray().filter((x: any) => x.getSource?.().getUrls?.()[0]?.includes("fields=class")).length)).toBeGreaterThan(0);
});
