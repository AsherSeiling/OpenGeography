// TC: tools are grouped by category (docs/TEST_PLAN.md TC-*). The Geoprocessing pane is gone: the Shift+A menu's category rail is the
// "browse all" view, and the ribbon's Add Tool opens it.
import { test, expect, addTestLayer } from "./fixtures";

test("TC-2 the Add Tool menu groups tools by category, shows sub-groups, and search crosses the groups", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.locator(".rtab", { hasText: /^Analysis$/ }).click();
  await app.locator(".rbtn", { hasText: /^Add Tool$/ }).click();
  const menu = app.locator(".toolmenu");
  await expect(menu).toBeVisible();
  await menu.locator(".tm-rail button", { hasText: /^Raster/ }).click(); // pick the group
  await expect(menu.locator(".tm-sec", { hasText: "Terrain and Contours" })).toBeVisible();
  await expect(menu.locator(".tm-name", { hasText: /^Hillshade$/ })).toBeVisible();
  await menu.locator(".tm-rail button", { hasText: /^Vector/ }).click(); // another group replaces it
  await expect(menu.locator(".tm-sec", { hasText: "Terrain and Contours" })).toHaveCount(0);
  await menu.getByRole("combobox").fill("hillshade"); // a search looks in every group
  await expect(menu.locator(".tm-name", { hasText: /^Hillshade$/ })).toBeVisible();
  await app.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(app.locator(".tools")).toHaveCount(0); // no Geoprocessing pane any more
});
