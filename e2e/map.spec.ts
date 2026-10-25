import { test, expect, gis, addTestLayer, ribbon } from "./fixtures";

const status = (page: any) => page.locator(".statusbar");

test("M-4 map CRS: UTM 14N, Web Mercator, polar 3031 with dropped note", async ({ app }) => {
  await addTestLayer(app, "polygons", 60, [-170, -60, 170, 70]); // a world-wide layer
  await ribbon(app, "Map", "Coordinate System");
  const dlg = app.getByRole("dialog");
  await dlg.getByRole("combobox").or(dlg.getByRole("searchbox")).or(dlg.locator("input").first()).first().fill("26914");
  await dlg.getByText(/NAD83 \/ UTM zone 14N$/).first().click();
  await dlg.getByRole("button", { name: "OK" }).click();
  await expect(status(app).locator(".sb-crs")).toContainText(/UTM zone 14N/);
  expect((await gis(app, (g) => g.useApp.getState().mapCrs)).id).toBe("EPSG:26914");
  await expect(app.locator(".map canvas").first()).toBeVisible();

  await ribbon(app, "Map", "Coordinate System");
  await app.getByRole("dialog").getByRole("button", { name: /Web Mercator/ }).click();
  await app.getByRole("dialog").getByRole("button", { name: "OK" }).click();
  await expect(status(app).locator(".sb-crs")).toContainText(/Pseudo-Mercator|Web Mercator/);

  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:3031", name: "WGS 84 / Antarctic Polar Stereographic" }));
  await expect(app.locator(".map-notice.warn")).toContainText(/outside the area of use/, { timeout: 20_000 });
  await expect(app.locator(".map canvas").first()).toBeVisible(); // not blank: map still rendered
});

const view = (page: any) => gis(page, (g) => { const v = g.getMap().getView(); return { center: v.getCenter(), res: v.getResolution() }; });
const mapBox = async (page: any) => (await page.locator(".map-wrap").boundingBox())!;

test("M-5 navigation: wheel zoom, drag pan, double-click zoom, Full Extent, Zoom To Layer", async ({ app }) => {
  const info = await addTestLayer(app, "points", 300);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), info.id);
  await app.waitForTimeout(800);
  const b = await mapBox(app);
  const cx = b.x + b.width / 2, cy = b.y + b.height / 2;
  const v0 = await view(app);

  await app.mouse.move(cx, cy);
  await app.mouse.wheel(0, -400);
  await expect.poll(async () => (await view(app)).res).toBeLessThan(v0.res);
  const v1 = await view(app);

  await app.mouse.move(cx, cy);
  await app.mouse.down();
  await app.mouse.move(cx + 150, cy + 80, { steps: 8 });
  await app.mouse.up();
  await expect.poll(async () => (await view(app)).center[0]).toBeLessThan(v1.center[0]); // dragged right → centre moves west

  const v2 = await view(app);
  await app.mouse.dblclick(cx + 100, cy + 50);
  await expect.poll(async () => (await view(app)).res).toBeLessThan(v2.res);

  await ribbon(app, "Map", "Full Extent");
  await expect.poll(async () => (await view(app)).res).toBeGreaterThan(v2.res);

  // Zoom To Layer from the Contents context menu
  await gis(app, (g) => g.mapController.zoomToExtent([0, 0, 1, 1]));
  await app.locator(".tree-layer", { hasText: "Test points" }).click({ button: "right" });
  await app.getByRole("menuitem", { name: /Zoom To Layer/ }).click();
  await expect.poll(async () => { const c = (await view(app)).center; return Math.abs(c[0] + 95) < 6 && Math.abs(c[1] - 35) < 6; }).toBe(true);
});

test("M-7 coordinate readout follows the pointer and ends on the last position", async ({ app }) => {
  await addTestLayer(app, "points", 50);
  await app.waitForTimeout(800);
  const b = await mapBox(app);
  const pts = Array.from({ length: 12 }, (_, i) => [b.x + 200 + i * 30, b.y + 150 + i * 12]);
  for (const [x, y] of pts) await app.mouse.move(x, y);
  const [lx, ly] = pts[pts.length - 1];
  const expected = await gis(app, (g, p: any) => {
    const m = g.getMap(); const r = m.getTargetElement().getBoundingClientRect();
    const c = m.getCoordinateFromPixel([p[0] - r.left, p[1] - r.top]);
    return g.mapController.getActiveCrs().id === "EPSG:4326" ? c : null;
  }, [lx, ly]);
  await expect(app.locator(".sb-coords")).toContainText("°");
  await expect.poll(async () => { const p = await gis(app, (g) => g.useMapView.getState().pointer); return p ? Math.abs(p.lon - expected[0]) + Math.abs(p.lat - expected[1]) : 99; }).toBeLessThan(0.01);
  await expect(app.locator(".sb-coords")).toContainText(/\d+°\d{2}′/); // DMS
  // projected CRS: shows E/N
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:32614", name: "WGS 84 / UTM zone 14N" }));
  await app.waitForTimeout(500);
  await app.mouse.move(b.x + 300, b.y + 200);
  await app.mouse.move(b.x + 320, b.y + 210);
  await expect(app.locator(".sb-coords")).toContainText(/ E .* N/);
});

test("M-8 panes toggle and reset; dark theme", async ({ app }) => {
  const tabOf = (name: string) => app.locator(".dv-default-tab, .dv-tab").filter({ hasText: new RegExp(`^${name}$`) });
  await app.locator(".rtab", { hasText: /^View$/ }).click();
  for (const [label, tab] of [["Contents", "Contents"], ["Attribute Table", "Attribute Table"], ["Properties", "Properties"], ["Geoprocessing", "Geoprocessing"], ["Catalog", "Catalog"]] as const) {
    const btn = app.locator(".rbtn", { hasText: new RegExp(`^${label}$`) }).first();
    await expect(tabOf(tab).first()).toBeVisible();
    await btn.click();
    await expect(tabOf(tab)).toHaveCount(0);
    await btn.click();
    await expect(tabOf(tab).first()).toBeVisible();
  }
  await app.locator(".rbtn", { hasText: "Contents" }).first().click();
  await ribbon(app, null, "Reset Panes");
  await expect(tabOf("Contents").first()).toBeVisible();
  const was = await gis(app, (g) => g.useApp.getState().dark);
  await ribbon(app, null, "Dark Theme");
  expect(await gis(app, (g) => g.useApp.getState().dark)).toBe(!was);
  await ribbon(app, null, "Dark Theme");
  expect(await gis(app, (g) => g.useApp.getState().dark)).toBe(was);
});
