// TC: tools are grouped in dropdowns (docs/TEST_PLAN.md TC-*).
import { test, expect, gis } from "./fixtures";

test("TC-2 Geoprocessing pane groups tools in dropdowns that open, close and open on search", async ({ app }) => {
  await app.getByRole("tab", { name: "Geoprocessing" }).click();
  const raster = app.getByTestId("tool-group-Raster");
  await expect(raster).toBeVisible();
  await expect(raster.getByText("Terrain and Contours")).toBeVisible();
  await raster.getByRole("button", { name: /^Raster/ }).first().click(); // close the group
  await expect(raster.getByText("Terrain and Contours")).toHaveCount(0);
  await app.getByPlaceholder("Find Tools").fill("hillshade"); // a search opens what matches
  await expect(app.getByTestId("tool-group-Raster").getByText("Hillshade")).toBeVisible();
});
