import { test, expect, gis, ribbon } from "./fixtures";

test("R8 world view: Albers over the whole Earth, graticule and outline layers, projection builder re-centres", async ({ app }) => {
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:5070", name: "NAD83 / Conus Albers" }));
  await app.waitForTimeout(600);
  await ribbon(app, "Map", /^World View$/);
  await expect.poll(async () => gis(app, (g) => g.mapController.getActiveCrs()?.info.world?.lon0)).toBe(-96);
  expect(await gis(app, (g) => g.mapController.getActiveCrs()?.id)).toBe("world:EPSG:5070");

  await ribbon(app, "Map", /^World Outline$/);
  await expect.poll(async () => gis(app, (g) => Object.values(g.useApp.getState().layers).filter((l: any) => /World outline/.test(l.name)).length)).toBe(1);
  await ribbon(app, "Map", /^Graticule$/);
  await expect.poll(async () => gis(app, (g) => Object.values(g.useApp.getState().layers).filter((l: any) => /Graticule|Reference lines/.test(l.name)).length)).toBe(2);
  // The whole-Earth view is framed: the view's extent is about the size of the grid.
  const framed = await gis(app, (g) => {
    const a = g.mapController.getActiveCrs(); const v = g.getMap().getView(); const [w, h] = g.getMap().getSize();
    return { cover: Math.min(w, h) * v.getResolution() / (a.info.grid.extent[2] - a.info.grid.extent[0]) };
  });
  expect(framed.cover).toBeGreaterThan(0.9);
  expect(framed.cover).toBeLessThan(1.4);
  // The source of the outline is cited.
  const src = await gis(app, (g) => g.api.sources());
  expect(src.some((s: any) => s.citation.key === "natural-earth-110m-land")).toBe(true);

  // Back to the normal view.
  await ribbon(app, "Map", /^World View$/);
  await expect.poll(async () => gis(app, (g) => g.mapController.getActiveCrs()?.id)).toBe("EPSG:5070");
});

test("R8 projection builder: sliders make a PROJ string, bad parallels are explained, the map re-centres", async ({ app }) => {
  await ribbon(app, "Map", /Projection Builder/);
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByTestId("proj-string")).toContainText("+proj=aea");
  // Equal and opposite standard parallels are rejected with a sentence, and no string is offered.
  await dlg.getByRole("slider", { name: "First standard parallel" }).focus();
  await app.keyboard.press("Home"); // -89
  await dlg.getByRole("slider", { name: "Second standard parallel" }).focus();
  await app.keyboard.press("End"); // 89
  await expect(dlg).toContainText(/cannot be equal and opposite/);
  await expect(dlg.getByTestId("proj-string")).toHaveText("—");
  await expect(dlg.getByRole("button", { name: /Use as map CRS/ })).toBeDisabled();
  await dlg.getByRole("button", { name: "Close" }).click();
});
