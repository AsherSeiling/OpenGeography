import { test, expect, addTestLayer, ribbon } from "./fixtures";

test("D0 Elevation dialog shows the engine's plan, warns about coarsening and needs a confirmation for a large download", async ({ app }) => {
  await addTestLayer(app, "polygons", 20, [-100, 36, -99.9, 36.1]); // about 9 km: one request, no confirmation
  await ribbon(app, "Data", /Elevation/);
  const dlg = app.getByRole("dialog");
  await dlg.getByRole("combobox").first().click();
  await app.getByRole("option", { name: /Extent of/ }).click();
  await expect(dlg).toContainText(/cells .*MB · 1 request · EPSG:326/, { timeout: 10_000 });
  await expect(dlg.getByRole("button", { name: "Download" })).toBeEnabled();
  await expect(dlg.getByRole("checkbox")).toHaveCount(0);
  await dlg.getByRole("button", { name: "Cancel" }).click();

  // A continental box: coarsened (and it says so), geographic, and the Download button waits for the confirmation.
  await addTestLayer(app, "polygons", 21, [-125, 25, -66, 49]);
  await ribbon(app, "Data", /Elevation/);
  const d2 = app.getByRole("dialog");
  await d2.getByRole("combobox").first().click();
  await app.getByRole("option", { name: /Extent of Test polygons \(21\)/ }).click();
  await expect(d2).toContainText(/too large for 10 m cells/, { timeout: 10_000 });
  await expect(d2).toContainText(/EPSG:4326/);
  const go = d2.getByRole("button", { name: "Download" });
  await expect(go).toBeDisabled();
  await d2.getByRole("checkbox").check();
  await expect(go).toBeEnabled();
  await d2.getByRole("button", { name: "Cancel" }).click();
});
