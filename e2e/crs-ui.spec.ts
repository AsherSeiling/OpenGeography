import { test, expect, gis, addTestLayer, ribbon } from "./fixtures";

test("R6 status bar shows k and convergence in a projected map CRS, and the picker previews a custom definition", async ({ app }) => {
  await addTestLayer(app, "points", 50);
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await app.waitForTimeout(800);
  const b = (await app.locator(".map-wrap").boundingBox())!;
  await app.mouse.move(b.x + 300, b.y + 200);
  await app.mouse.move(b.x + 320, b.y + 210);
  await expect(app.locator(".sb-coords")).toContainText(/k \d\.\d{5} · γ -?\d+\.\d{3}°/, { timeout: 10_000 });

  // The picker explains a typed definition before it is used, and says why a bad one fails.
  await ribbon(app, "Map", "Coordinate System");
  const dlg = app.getByRole("dialog");
  const box = dlg.getByPlaceholder(/\+proj=utm/);
  await box.fill("+proj=utm +zone=14 +datum=NAD83 +units=us-ft");
  await expect(dlg).toContainText(/projected · .*foot · North American Datum 1983/, { timeout: 10_000 });
  await box.fill("not a crs");
  await expect(dlg).toContainText(/Unknown or invalid coordinate system/, { timeout: 10_000 });
  await dlg.getByRole("button", { name: /Cancel/ }).click();
});

test("R6b Tissot circles are added for the map CRS, and the picker remembers what was picked", async ({ app }) => {
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:5070", name: "NAD83 / Conus Albers" }));
  await ribbon(app, "Map", /Tissot/);
  await expect.poll(async () => gis(app, (g) => Object.values(g.useApp.getState().layers).filter((l: any) => /Tissot/.test(l.name)).length)).toBe(1);
  await expect(app.locator(".statusbar")).toContainText(/Tissot circles for/);

  await ribbon(app, "Map", "Coordinate System");
  const dlg = app.getByRole("dialog");
  await dlg.getByRole("button", { name: /Web Mercator|Pseudo-Mercator/ }).first().click();
  await dlg.getByRole("button", { name: "OK" }).click();
  await ribbon(app, "Map", "Coordinate System");
  await expect(app.getByRole("dialog")).toContainText("Recent:");
  await app.getByRole("dialog").getByRole("button", { name: /Cancel/ }).click();
});
