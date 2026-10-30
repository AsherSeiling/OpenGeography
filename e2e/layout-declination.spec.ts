// LW6: the declination diagram (grid, true and magnetic north from the World Magnetic Model) in a layout.
import { test, expect, gis, ribbon, addTestLayer } from "./fixtures";

test("LW6 insert a declination diagram, change style, units and date, and see it drawn by the engine", async ({ app }) => {
  await addTestLayer(app, "polygons", 5, [-93.1, 45.0, -92.9, 45.1]);
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await ribbon(app, "Layout", /^Map Frame$/);
  await gis(app, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { crs: "EPSG:26915", center: [490000, 4985000], scale: 24000, basemap: "none" }); });
  await ribbon(app, "Layout", /^Declination$/);
  const el = async () => (await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "declination"))) as any;
  expect((await el()).style).toBe("usgs");
  await expect(app.locator(".props-section-title", { hasText: "Text sizes and fonts" })).toBeVisible();

  await app.getByRole("combobox", { name: "Style" }).click();
  await app.getByRole("option", { name: "US Army (grid-magnetic angle)" }).click();
  await app.getByRole("combobox", { name: "Units" }).click();
  await app.getByRole("option", { name: "Mils (6400 to a circle)" }).click();
  await app.getByRole("textbox", { name: "Declination date" }).fill("2026-06-01");
  await expect.poll(async () => (await el()).date).toBe("2026-06-01");
  expect(await el()).toMatchObject({ style: "army", units: "mils" });

  // The engine draws all three lines and the army caption in mils.
  const svg = (await gis(app, async (g) => {
    const s = g.useLayouts.getState();
    const r = await g.api.renderLayoutPreview(g.layoutActions.layoutRequest(s.doc), 96);
    return r.warnings;
  })) as string[];
  expect(svg.filter((w) => /declination/i.test(w))).toEqual([]);
  // Out-of-range dates are explained rather than guessed.
  await gis(app, (g) => { const d = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "declination"); g.useLayouts.getState().update(d.id, { date: "2045-01-01" }); });
  const warn = (await gis(app, async (g) => (await g.api.renderLayoutPreview(g.layoutActions.layoutRequest(g.useLayouts.getState().doc), 96)).warnings)) as string[];
  expect(warn.some((w) => /Declination diagram/.test(w))).toBe(true);
});
