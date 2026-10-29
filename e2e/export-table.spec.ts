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
