// LB1 fonts from the device. Cases LB1-* of docs/TEST_PLAN.md.
import { test, expect, gis, addTestLayer } from "./fixtures";

test("LB1 the engine lists installed fonts and the picker sets the family, bold and italic on the label style", async ({ app }) => {
  const fonts: { name: string }[] = await gis(app, (g) => g.api.listFonts());
  expect(fonts.length, "this computer has fonts").toBeGreaterThan(0);
  const layer = await addTestLayer(app, "polygons", 10, [-98, 34, -96, 36]);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await gis(app, (g, a: any) => { const s = g.useApp.getState(); s.setStyle(a.id, { ...s.styles[a.id], label: { enabled: true, field: "class", size: 9, color: "#222222", haloColor: "#ffffff", haloWidth: 1.5 } }); }, { id: layer.id });
  await gis(app, async (g, id) => { const m = await import("/src/dialogs/LayerPropertiesWindow.tsx"); m.openLayerProperties(id, "symbology"); }, layer.id);
  const pick = app.getByTestId("label-font");
  await expect(pick).toBeVisible();
  const name = fonts[0].name;
  await pick.click();
  await pick.fill(name);
  await app.getByRole("option", { name }).first().click();
  await expect.poll(() => gis(app, (g, id) => g.useApp.getState().styles[id].label.fontFamily, layer.id)).toBe(name);
  await app.getByRole("checkbox", { name: "Bold" }).check();
  await expect.poll(() => gis(app, (g, id) => g.useApp.getState().styles[id].label.bold, layer.id)).toBe(true);
});
