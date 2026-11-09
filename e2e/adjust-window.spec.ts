// Adjust Shapes (GR3): the "To points are in" coordinate system dialog opens over the window and closes back to it.
// The fixture fails this test on any console error, which is how the mis-nested dialog first showed up.
import { test, expect, ribbon, addTestLayer } from "./fixtures";

test("GR3 Adjust Shapes opens its coordinate system dialog on top and Cancel returns to the window", async ({ app }) => {
  await addTestLayer(app, "polygons", 4);
  await ribbon(app, "Map", "Adjust Shapes");
  const main = app.getByRole("dialog", { name: "Spatial Adjustment" });
  await expect(main).toBeVisible();

  await main.getByTestId("adj-crs").click();
  const crs = app.getByRole("dialog", { name: "Coordinate system of the To points" });
  await expect(crs).toBeVisible();
  await crs.getByRole("button", { name: "Cancel" }).click();
  await expect(crs).toHaveCount(0);

  await expect(main).toBeVisible();
  await main.getByRole("button", { name: "Close", exact: true }).click();
  await expect(main).toHaveCount(0);
});
