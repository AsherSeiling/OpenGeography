// CRS accuracy plan A2/A3: the Properties pane says how accurate the datum shift to the map is, and Strict mode can be switched on.
import { test, expect, gis } from "./fixtures";

test("A2/A3 the layer's Properties show the datum shift to the map CRS, and the Strict checkbox is remembered by the engine", async ({ app }) => {
  const layer = await gis(app, async (g) => {
    const info = await g.api.createFeatures("NAD27 points", [{ wkt: "POINT(-97.5 35.4)" }, { wkt: "POINT(-97.4 35.5)" }]);
    const l = await g.api.defineProjection(info.id, "EPSG:4267", "NAD27");
    g.useApp.getState().addLayers([l]);
    g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" });
    return l;
  }) as any;
  await gis(app, (g, id) => { const s = g.useApp.getState(); s.setActiveLayer(id); s.setProperties({ kind: "layer", id }); }, layer.id);
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
