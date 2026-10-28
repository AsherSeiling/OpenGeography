// B6 Split by Attribute: one layer or one file per value. Cases B6-* of docs/TEST_PLAN.md.
import { existsSync, mkdtempSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

async function open(page: any, id: string) {
  await gis(page, (g, i) => g.actions.splitByAttribute(i), id);
  const dlg = page.getByRole("dialog");
  await expect(dlg.getByText(/Split by Attribute —/)).toBeVisible();
  await dlg.getByRole("combobox").first().click();
  await page.getByRole("option", { name: "class" }).click();
  await expect(dlg.getByTestId("split-groups")).toContainText(/values?, 30 rows/);
  return dlg;
}

test("B6 split into layers: one per value, none lost, the input untouched", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 30, [-100, 30, -95, 35]);
  const distinct = (await gis(app, (g, id) => g.api.uniqueValues(id, "class", 100), layer.id)).length;
  const dlg = await open(app, layer.id);
  await dlg.getByRole("button", { name: "Split", exact: true }).click();
  await expect.poll(async () => (await gis(app, (g) => g.useApp.getState().order)).length).toBe(1 + distinct);
  const counts = await gis(app, (g, id) => g.useApp.getState().order.filter((x: string) => x !== id).map((x: string) => g.useApp.getState().layers[x].featureCount), layer.id);
  expect(counts.reduce((a: number, b: number) => a + b, 0)).toBe(30);
  expect(await gis(app, (g, id) => g.useApp.getState().layers[id].featureCount, layer.id)).toBe(30);
});

test("B6 split into files: needs a folder, writes one file per value, never overwrites", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 30, [-100, 30, -95, 35]);
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-split-"));
  const distinct = (await gis(app, (g, id) => g.api.uniqueValues(id, "class", 100), layer.id)).length;
  const dlg = await open(app, layer.id);
  await dlg.getByRole("radio", { name: /One file per value/ }).check();
  await expect(dlg.getByRole("button", { name: "Split", exact: true })).toBeDisabled(); // no folder yet
  await dlg.getByPlaceholder("Folder to write into").fill(dir);
  await dlg.getByRole("button", { name: "Split", exact: true }).click();
  await expect.poll(() => readdirSync(dir).filter((f) => f.endsWith(".gpkg")).length).toBe(distinct);
  expect(existsSync(join(dir, "0.gpkg"))).toBe(true);
  // Again into the same folder: the files are kept, new ones get a number.
  const dlg2 = await open(app, layer.id);
  await dlg2.getByRole("radio", { name: /One file per value/ }).check();
  await dlg2.getByPlaceholder("Folder to write into").fill(dir);
  await dlg2.getByRole("button", { name: "Split", exact: true }).click();
  await expect.poll(() => readdirSync(dir).filter((f) => f.endsWith(".gpkg")).length).toBe(2 * distinct);
  expect(existsSync(join(dir, "0_2.gpkg"))).toBe(true);
});
