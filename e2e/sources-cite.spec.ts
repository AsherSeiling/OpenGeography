import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer, ribbon } from "./fixtures";

test("UR-13 a layer with no citation is marked in Map Layers and Sources; a form or pasted text gives it one", async ({ app }) => {
  // An imported file is cited automatically; a generated layer is not.
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-ur13-"));
  const file = join(dir, "parcels.geojson");
  writeFileSync(file, JSON.stringify({ type: "FeatureCollection", crs: { type: "name", properties: { name: "EPSG:4326" } }, features: [{ type: "Feature", properties: { a: 1 }, geometry: { type: "Point", coordinates: [-97, 35] } }] }));
  await gis(app, (g, f) => g.actions.addData([f]), file);
  const gen = await addTestLayer(app, "points", 5);
  const marks = app.getByTestId("uncited-mark");
  await expect(marks).toHaveCount(1, { timeout: 10_000 });
  await expect(app.locator(".tree-layer", { has: marks })).toContainText(gen.name);

  // The Sources pane says so too (it is not open by default).
  await ribbon(app, "Data", /^Sources$/);
  await expect(app.getByTestId("uncited")).toContainText("1 layer has no citation");

  // Click the marker: the Add citation dialog for that layer opens. The form needs a title or an author.
  await marks.click();
  const dlg = app.getByRole("dialog");
  await expect(dlg).toContainText(`Add a citation — ${gen.name}`);
  await expect(dlg.getByRole("button", { name: "Add citation" })).toBeDisabled();
  await dlg.getByLabel("Title").fill("County parcels");
  await dlg.getByLabel("Author or organisation").fill("Example County GIS");
  await dlg.getByLabel("Year").fill("2025");
  await dlg.getByLabel("Licence or terms of use").fill("CC BY 4.0");
  // A date that isn't one is refused in words and nothing is added.
  await dlg.getByLabel("Accessed date").fill("last Tuesday");
  await dlg.getByRole("button", { name: "Add citation" }).click();
  await expect(dlg).toContainText("year-month-day");
  await dlg.getByLabel("Accessed date").fill("2026-03-04");
  await dlg.getByRole("button", { name: "Add citation" }).click();
  await expect(app.getByRole("dialog")).toHaveCount(0);
  await expect(app.getByTestId("uncited-mark")).toHaveCount(0);
  await expect(app.getByTestId("uncited")).toHaveCount(0);
  await expect(app.locator(".src-entry", { hasText: "County parcels" })).toContainText("Example County GIS");
  await expect(app.locator(".src-entry", { hasText: "County parcels" })).toContainText("CC BY 4.0");

  // A pasted citation is listed exactly as pasted.
  const gen2 = await addTestLayer(app, "polygons", 3);
  await expect(app.getByTestId("uncited")).toContainText("1 layer has no citation", { timeout: 10_000 });
  await app.getByRole("button", { name: `Add citation for ${gen2.name}` }).click();
  await app.getByRole("button", { name: "Paste a citation" }).click();
  const text = "Smith, J. (2020). Soil survey of Example County. U.S. Dept. of Agriculture.";
  await app.getByLabel("Citation text").fill(text);
  await app.getByRole("button", { name: "Add citation" }).last().click();
  await expect(app.locator(".src-entry", { hasText: "Soil survey of Example County" })).toContainText(text);
  await expect(app.getByTestId("uncited-mark")).toHaveCount(0);
});
