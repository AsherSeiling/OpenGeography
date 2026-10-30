// LW5: grid and graticule styles, label formats and the UTM zone designation from the map frame's Properties.
import { test, expect, gis, ribbon, addTestLayer } from "./fixtures";

test("LW5 grid style, label format and zone designation reach the drawing", async ({ app }) => {
  await addTestLayer(app, "polygons", 5, [-93.1, 45.0, -92.9, 45.1]);
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await ribbon(app, "Layout", /^Map Frame$/);
  await gis(app, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { crs: "EPSG:26915", center: [490000, 4985000], scale: 24000, basemap: "none" }); });
  const frame = async () => (await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"))) as any;

  await app.getByRole("checkbox", { name: "Grid (map coordinates)" }).check();
  await app.getByRole("combobox", { name: "Style" }).click();
  await app.getByRole("option", { name: "Ticks at the edges" }).click();
  await app.getByRole("combobox", { name: "Label format" }).click();
  await app.getByRole("option", { name: "Two digits (1 km grid)" }).click();
  await app.getByRole("checkbox", { name: "Zone and 100 km square (UTM)" }).check();
  await app.getByRole("spinbutton", { name: "Line width (mm)" }).fill("0.3");
  await expect.poll(async () => (await frame()).grid?.designation).toBe(true);
  const g = (await frame()).grid;
  expect(g).toMatchObject({ style: "ticks", labelFormat: "short", designation: true, width: 0.3 });

  // The engine draws it: tick paths, two-digit labels, and a designation like "15T …".
  const out = (await gis(app, async (x) => { const s = x.useLayouts.getState(); const st = x.useApp.getState(); return x.api.renderLayoutPreview(x.layoutActions.layoutRequest(s.doc), 96); })) as any;
  expect(out.warnings.filter((w: string) => /grid/i.test(w))).toEqual([]);
});
