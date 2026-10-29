import { test, expect, addTestLayer, ribbon } from "./fixtures";

test("D6 Earthquakes & Soils dialog: soils refuse a large area before any request, bad dates are explained", async ({ app }) => {
  await addTestLayer(app, "polygons", 20, [-100, 30, -90, 40]);
  await ribbon(app, "Data", /Earthquakes & Soils/);
  const dlg = app.getByRole("dialog");
  await dlg.getByRole("combobox").first().click();
  await app.getByRole("option", { name: /Extent of/ }).click();
  await dlg.getByRole("radio", { name: /Soil map units/ }).check();
  await expect(dlg).toContainText(/too large for soil map units/);
  await expect(dlg.getByRole("button", { name: "Download" })).toBeDisabled();
  await dlg.getByRole("radio", { name: /Earthquakes/ }).check();
  await expect(dlg.getByRole("button", { name: "Download" })).toBeEnabled();
  await dlg.getByLabel("From").fill("June 2024");
  await dlg.getByRole("button", { name: "Download" }).click();
  await expect(dlg).toContainText(/Dates look like 2024-06-01/, { timeout: 10_000 });
  await dlg.getByRole("button", { name: "Cancel" }).click();
});
