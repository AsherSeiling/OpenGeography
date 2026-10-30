// Export Table: attributes plus coordinates in a chosen CRS. Cases ET-* of docs/TEST_PLAN.md.
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

test("ET-1 the CRS must be chosen; CSV has lon/lat and a sidecar naming the system", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 12, [-100, 30, -95, 35]);
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-et-"));
  const file = join(dir, "pts.csv");
  await gis(app, (g, i) => g.actions.exportTable(i), layer.id);
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByText(/Export Table —/)).toBeVisible();
  await dlg.getByTestId("export-table-path").fill(file);
  await expect(dlg.getByRole("button", { name: "Export", exact: true })).toBeDisabled(); // no CRS chosen: nothing is assumed
  await dlg.getByRole("button", { name: "WGS 84 lon/lat" }).click();
  await dlg.getByRole("button", { name: "Export", exact: true }).click();
  await expect.poll(() => existsSync(file)).toBe(true);
  const lines = readFileSync(file, "utf8").trim().split("\n");
  expect(lines.length).toBe(13);
  expect(lines[0]).toMatch(/lon,lat$/);
  const last = lines[1].split(",");
  expect(Number(last.at(-2))).toBeGreaterThanOrEqual(-100);
  expect(Number(last.at(-2))).toBeLessThanOrEqual(-95);
  expect(readFileSync(file + ".crs.txt", "utf8")).toMatch(/Units: degree/);
});

test("ET-USNG the export can write a USNG or MGRS reference with no coordinate system chosen", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 12, [-100, 30, -95, 35]);
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-et-"));
  const usng = join(dir, "usng.csv");
  await gis(app, (g, i) => g.actions.exportTable(i), layer.id);
  const dlg = app.getByRole("dialog");
  await dlg.getByTestId("export-table-path").fill(usng);
  await dlg.getByRole("combobox", { name: "Coordinates to write" }).click();
  await app.getByRole("option", { name: "USNG reference only" }).click();
  await expect(dlg.getByTestId("export-table-grid-note")).toContainText("chosen automatically");
  await dlg.getByRole("combobox", { name: "Grid reference precision" }).click();
  await app.getByRole("option", { name: "1 km (2 digits)" }).click();
  await expect(dlg.getByText("Coordinate system of the X/Y columns")).toHaveCount(0); // nothing to pick
  await dlg.getByRole("button", { name: "Export", exact: true }).click();
  await expect.poll(() => (existsSync(usng) && existsSync(usng + ".crs.txt") ? readFileSync(usng, "utf8").trim().split("\n").length : 0)).toBe(13);
  const lines = readFileSync(usng, "utf8").trim().split("\n");
  expect(lines[0]).toMatch(/USNG$/);
  expect(lines[0]).not.toMatch(/lon|lat/);
  for (const l of lines.slice(1)) expect(l.split(",").at(-1)).toMatch(/^\d{1,2}[C-X] [A-Z]{2} \d\d \d\d$/);
  expect(readFileSync(usng + ".crs.txt", "utf8")).toMatch(/USNG reference.*1000 m wide/s);

  // MGRS beside lon/lat: the CRS picker returns and both are written.
  const both = join(dir, "both.csv");
  await gis(app, (g, i) => g.actions.exportTable(i), layer.id);
  const d2 = app.getByRole("dialog");
  await d2.getByTestId("export-table-path").fill(both);
  await d2.getByRole("combobox", { name: "Coordinates to write" }).click();
  await app.getByRole("option", { name: "X/Y or lon/lat and an MGRS reference" }).click();
  await expect(d2.getByRole("button", { name: "Export", exact: true })).toBeDisabled(); // the X/Y columns still need a CRS
  await d2.getByRole("button", { name: "WGS 84 lon/lat" }).click();
  await d2.getByRole("button", { name: "Export", exact: true }).click();
  await expect.poll(() => (existsSync(both) && existsSync(both + ".crs.txt") ? readFileSync(both, "utf8").trim().split("\n").length : 0)).toBe(13);
  const lines2 = readFileSync(both, "utf8").trim().split("\n");
  expect(lines2[0]).toMatch(/lon,lat,MGRS$/);
  expect(lines2[1].split(",").at(-1)).toMatch(/^\d{1,2}[C-X][A-Z]{2}\d{10}$/);
});

test("ET-USNG-CRS choosing USNG or MGRS in the coordinate system dialog picks the UTM zone of the view", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 12, [-100, 30, -95, 35]);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.waitForTimeout(1200);
  await app.locator(".rbtn", { hasText: /Coordinate System/ }).first().click();
  const dlg = app.getByRole("dialog");
  await dlg.getByRole("button", { name: /^USNG/ }).click();
  await expect(dlg.getByTestId("grid-crs-note")).toContainText("UTM zone 14N"); // -97.5° is zone 14
  await expect(dlg.getByText(/NAD83 \/ UTM zone 14N/).first()).toBeVisible();
  await dlg.getByRole("button", { name: /^MGRS/ }).click();
  await expect(dlg.getByTestId("grid-crs-note")).toContainText("WGS 84 / UTM zone 14N");
});
