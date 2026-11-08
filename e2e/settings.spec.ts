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

test("UR-17 the current panes can be saved as the startup layout: new projects open with them, a project with its own layout keeps it, Reset goes back", async ({ app }) => {
  const dockTab = (name: string) => app.locator(".dv-tab").filter({ hasText: new RegExp(`^${name}$`) });
  await expect(dockTab("Catalog")).toHaveCount(1);
  const builtin = await app.evaluate(async () => (await import("/src/panes/panes.ts")).serializeDock());
  // Arrange: no Catalog, Sources open. Save it from the View tab.
  await dockTab("Catalog").locator(".dv-default-tab-action").click();
  await expect(dockTab("Catalog")).toHaveCount(0);
  await ribbon(app, "View", /^Sources$/);
  await expect(dockTab("Sources")).toHaveCount(1);
  await ribbon(app, "View", /^Save Startup Layout$/);
  await expect.poll(() => gis(app, (g) => g.api.getSettings().then((s: any) => !!s.startupLayout))).toBe(true);

  // A new project opens with it.
  await gis(app, (g) => g.actions.newProject());
  await expect(dockTab("Sources")).toHaveCount(1);
  await expect(dockTab("Catalog")).toHaveCount(0);
  await expect(dockTab("Map Layers")).toHaveCount(1);

  // A project saved with its own layout keeps it (here: the built-in one, restored the way opening a project does).
  await app.evaluate(async (d) => (await import("/src/panes/panes.ts")).restoreDock(d), builtin);
  await expect(dockTab("Catalog")).toHaveCount(1);
  await expect(dockTab("Sources")).toHaveCount(0);

  // Reset: a new project is built-in again.
  await ribbon(app, "View", /^Reset Startup Layout$/);
  await expect.poll(() => gis(app, (g) => g.api.getSettings().then((s: any) => !!s.startupLayout))).toBe(false);
  await gis(app, (g) => g.actions.newProject());
  await expect(dockTab("Catalog")).toHaveCount(1);
  await expect(dockTab("Sources")).toHaveCount(0);
  await gis(app, (g) => g.api.setSettings({}));
});
