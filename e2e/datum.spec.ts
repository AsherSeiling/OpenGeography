// CRS accuracy plan A2/A3: the Properties pane says how accurate the datum shift to the map is, and Strict mode can be switched on.
import { test, expect, gis, ribbon } from "./fixtures";

test("A2/A3 the layer's Properties show the datum shift to the map CRS, and the Strict checkbox is remembered by the engine", async ({ app }) => {
  const layer = await gis(app, async (g) => {
    const info = await g.api.createFeatures("NAD27 points", [{ wkt: "POINT(-97.5 35.4)" }, { wkt: "POINT(-97.4 35.5)" }]);
    const l = await g.api.defineProjection(info.id, "EPSG:4267", "NAD27");
    g.useApp.getState().addLayers([l]);
    g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" });
    return l;
  }) as any;
  await gis(app, (g, id) => { const s = g.useApp.getState(); s.setActiveLayer(id); s.setProperties({ kind: "layer", id }); }, layer.id);
  await app.getByRole("tab", { name: "Coordinate System" }).click();
  const pane = app.locator(".kv", { hasText: "Datum shift" });
  await expect(pane).toContainText(/Datum shift/, { timeout: 10_000 });
  await expect(pane).toContainText(/error up to about|not installed|No accurate/);

  const strict = app.getByLabel(/Refuse approximate datum shifts/);
  await expect(strict).not.toBeChecked();
  await strict.check();
  await expect.poll(() => gis(app, (g) => g.api.datumPolicy())).toBe(true);
  await strict.uncheck();
  await expect.poll(() => gis(app, (g) => g.api.datumPolicy())).toBe(false);
});

test("A5 the datum coverage table lists every checked region with a plain-words level", async ({ app }) => {
  const rows = (await gis(app, (g) => g.api.datumCoverage())) as [string, { level: string; text: string }][];
  expect(rows.length).toBeGreaterThanOrEqual(9);
  expect(rows.map((r) => r[0]).join(" ")).toMatch(/United States.*Canada.*Mexico.*Europe/);
  for (const [, r] of rows) expect(["exact", "precise", "approximate", "ballpark"]).toContain(r.level);
});

test("B2 the Datum Grids menu lists the catalog by region, filters it, and offers downloads", async ({ app }) => {
  await ribbon(app, "Map", "Datum Grids");
  const dlg = app.getByRole("dialog");
  await expect(dlg).toContainText("Datum Grids");
  const list = dlg.getByTestId("grid-list");
  await expect(list.locator("[data-grid]").first()).toBeVisible({ timeout: 10_000 });
  const total = await list.locator("[data-grid]").count();
  expect(total).toBeGreaterThan(100);
  // Regions come first, countries inside them: North America before Europe.
  const text = await list.innerText();
  expect(text.indexOf("North America")).toBeGreaterThanOrEqual(0);
  expect(text.indexOf("North America")).toBeLessThan(text.indexOf("Europe"));
  // Search narrows the list; the Canadian NAD27 grid is there with a way to get it or a note that it is installed.
  await dlg.getByLabel("Search grids").fill("ntv2_0");
  await expect(list.locator("[data-grid]")).toHaveCount(1);
  await expect(list.locator('[data-grid="ca_nrc_ntv2_0.tif"]')).toContainText(/Download|Installed|Included with the app/);
  await dlg.getByLabel("Search grids").fill("zzzz-no-such-grid");
  await expect(list).toContainText("No grids match.");
  await dlg.getByLabel("Search grids").fill("");
  // Purpose filter.
  await dlg.getByRole("combobox", { name: "Purpose" }).click();
  await app.getByRole("option", { name: /Heights/ }).click();
  const kinds = await list.locator("[data-grid]").count();
  expect(kinds).toBeGreaterThan(20);
  expect(kinds).toBeLessThan(total);
  await dlg.getByRole("button", { name: "Close" }).click();
});

test("D3/C4 the responsible-use statement opens from the ribbon, and a US-foot layer says which foot it uses", async ({ app }) => {
  await ribbon(app, "Map", "Limits & Responsible Use");
  const dlg = app.getByRole("dialog");
  await expect(dlg).toContainText("Limitations and responsible use");
  await expect(dlg.getByTestId("responsible-use")).toContainText(/not a survey instrument/i);
  await expect(dlg.getByTestId("responsible-use")).toContainText(/Define Vertical Datum/);
  await dlg.getByRole("button", { name: "Close" }).click();

  const layer = await gis(app, async (g) => {
    const info = await g.api.createFeatures("Oklahoma North ftUS", [{ wkt: "POINT(2000000 500000)" }]);
    const l = await g.api.defineProjection(info.id, "EPSG:2267", "NAD83 / Oklahoma North (ftUS)");
    g.useApp.getState().addLayers([l]);
    return l;
  }) as any;
  await gis(app, (g, id) => { const s = g.useApp.getState(); s.setActiveLayer(id); s.setProperties({ kind: "layer", id }); }, layer.id);
  await app.getByRole("tab", { name: "Coordinate System" }).click();
  await expect(app.locator(".kv", { hasText: "US survey foot" })).toContainText(/2 parts per million/, { timeout: 10_000 });
});
