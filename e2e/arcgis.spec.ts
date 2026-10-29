import { test, expect, ribbon } from "./fixtures";

test("D1 ArcGIS Services dialog lists the catalog with its citation and explains a bad address", async ({ app, errors }) => {
  await ribbon(app, "Data", /ArcGIS Services/);
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByRole("radio", { name: /Protected areas and land managers \(PAD-US\)/ })).toBeVisible({ timeout: 10_000 });
  for (const label of [/Flood hazard zones/, /Wetlands \(National Wetlands Inventory\)/, /Current wildfire perimeters/]) await expect(dlg.getByRole("radio", { name: label })).toBeVisible();
  // The source and licence are shown before anything is downloaded.
  await dlg.getByRole("radio", { name: /Flood hazard zones/ }).check();
  await expect(dlg).toContainText(/Federal Emergency Management Agency/);
  await expect(dlg).toContainText(/Public domain/);

  await dlg.getByRole("radio", { name: /A service address I paste/ }).check();
  await expect(dlg.getByRole("button", { name: "Download" })).toBeDisabled();
  await dlg.getByPlaceholder(/FeatureServer/).fill("https://example.com/data.json");
  await dlg.getByRole("button", { name: "Find layers" }).click();
  await expect(dlg).toContainText(/MapServer or FeatureServer/, { timeout: 10_000 });
  await dlg.getByRole("button", { name: "Cancel" }).click();
  void errors;
});
