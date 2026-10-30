// Messages drawn over the map can be cleared: one by one, or all at once; they come back only if the message changes.
import { test, expect, gis } from "./fixtures";

const put = (app: any, notices: Record<string, string>) => gis(app, (g, n: any) => g.useMapView.setState({ notices: n }), notices);

test("map messages are clearable and return only when their text changes", async ({ app }) => {
  await put(app, { one: "3 features of Roads aren't drawn", two: "2 raster layers aren't drawn yet" });
  const box = app.getByTestId("map-notices");
  await expect(box.locator(".map-notice")).toHaveCount(2);

  // Clear one.
  await box.getByRole("button", { name: /Clear: 3 features of Roads/ }).click();
  await expect(box.locator(".map-notice")).toHaveCount(1);
  await expect(box.getByText("2 raster layers aren't drawn yet")).toBeVisible();

  // The same message set again stays cleared (redraws re-send it); a different one shows.
  await put(app, { one: "3 features of Roads aren't drawn", two: "2 raster layers aren't drawn yet" });
  await expect(box.locator(".map-notice")).toHaveCount(1);
  await put(app, { one: "5 features of Roads aren't drawn", two: "2 raster layers aren't drawn yet" });
  await expect(box.locator(".map-notice")).toHaveCount(2);

  // Clear all.
  await box.getByRole("button", { name: "Clear all messages" }).click();
  await expect(app.getByTestId("map-notices")).toHaveCount(0);
});
