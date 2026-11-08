import { test, expect, gis, ribbon } from "./fixtures";

test("UR-16 Settings opens from the View tab and the project menu, saves per user, and survives a reload", async ({ app }) => {
  // The View tab.
  await ribbon(app, "View", /^Settings$/);
  let dlg = app.getByRole("dialog");
  await expect(dlg).toContainText("Settings");
  await dlg.getByRole("combobox", { name: "Distance unit" }).click();
  await app.getByRole("option", { name: "Kilometers" }).click();
  await dlg.getByRole("combobox", { name: "Coordinate system of a new map" }).click();
  await app.getByRole("option", { name: /Web Mercator/ }).click();
  await expect.poll(() => gis(app, (g) => g.api.getSettings().then((s: any) => `${s.lengthUnit}|${s.defaultCrs?.id}`))).toBe("km|EPSG:3857");
  await dlg.getByRole("button", { name: "Close" }).click();

  // After a reload the choices are applied: a new map starts in Web Mercator.
  await app.reload();
  await expect(app.locator(".rtab").first()).toBeVisible({ timeout: 20_000 });
  await gis(app, async (g) => { const { newMap } = await import("/src/maps/mapActions.ts"); newMap(); });
  await expect.poll(() => gis(app, (g) => g.useApp.getState().maps.at(-1)?.crs?.id)).toBe("EPSG:3857");

  // The project menu.
  await app.locator(".proj-btn").click();
  await app.getByRole("menuitem", { name: /^Settings/ }).click();
  dlg = app.getByRole("dialog");
  await expect(dlg).toContainText("Settings");
  // A coordinate system that is not one is refused in words and nothing is saved.
  await dlg.getByLabel("Another coordinate system").fill("EPSG:99999999");
  await dlg.getByRole("button", { name: "Use" }).click();
  await expect(dlg).toContainText(/isn't a coordinate system/);
  await expect.poll(() => gis(app, (g) => g.api.getSettings().then((s: any) => s.defaultCrs?.id))).toBe("EPSG:3857");
  await dlg.getByRole("button", { name: "Close" }).click();

  // Put the machine's settings back: this run shares one settings file.
  await gis(app, (g) => g.api.setSettings({}));
});
