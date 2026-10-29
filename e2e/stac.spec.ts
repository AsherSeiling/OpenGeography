import { test, expect, ribbon } from "./fixtures";

test("D3 Imagery & DEM dialog lists the catalog and explains a bad date before searching", async ({ app }) => {
  await ribbon(app, "Data", /Imagery & DEM/);
  const dlg = app.getByRole("dialog");
  await expect(dlg).toContainText("Imagery and elevation (STAC)");
  await expect(dlg.getByRole("combobox").nth(1)).toContainText(/Sentinel-2 surface reflectance/, { timeout: 10_000 });
  await expect(dlg.getByRole("button", { name: "Download" })).toBeDisabled();
  await dlg.getByPlaceholder("from/to").fill("yesterday");
  await dlg.getByRole("button", { name: "Search" }).click();
  await expect(dlg).toContainText(/Dates look like 2024-06-01\/2024-09-30/, { timeout: 10_000 });
  await dlg.getByRole("button", { name: "Cancel" }).click();
});
