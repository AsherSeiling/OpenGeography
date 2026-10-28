// C3.1 layout templates: new layout from a bundled template, save the open layout as a template, use it, delete it.
import { test, expect, gis, addTestLayer } from "./fixtures";

const doc = (page: any) => gis(page, (g) => { const d = g.useLayouts.getState().doc; return d && { name: d.name, w: d.page.width, h: d.page.height, els: d.elements.map((e: any) => ({ kind: e.kind, crs: e.crs, center: e.center, scale: e.scale, ratio: e.ratio })) }; }) as Promise<any>;

async function newFromTemplate(page: any, name: RegExp) {
  await page.locator(".rtab", { hasText: /^Insert$/ }).click();
  await page.locator(".rbtn", { hasText: /New Layout/ }).click();
  await page.getByRole("menuitem", { name: "From template" }).click();
  await page.getByRole("menuitem", { name }).click();
}

test("C3.1 a layout from a bundled template: six elements, the frame takes the map's CRS and view", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 20, [-100, 34, -99.5, 34.5]);
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.waitForTimeout(800);
  await newFromTemplate(app, /Map — A4 portrait/);
  await expect(app.locator(".layout-page")).toBeVisible();
  await expect.poll(async () => (await doc(app))?.els.find((e: any) => e.kind === "map_frame")?.center[0]).toBeGreaterThan(100000); // placed on the map view, not at 0,0
  const d = await doc(app);
  expect(d.els.map((e: any) => e.kind)).toEqual(["text", "map_frame", "legend", "north_arrow", "scale_bar", "sources"]);
  expect(d.name).toMatch(/^Layout \d+$/);
  expect([Math.round(d.w), Math.round(d.h)]).toEqual([210, 297]);
  const f = d.els.find((e: any) => e.kind === "map_frame");
  expect(f.crs).toBe("EPSG:26914");
  expect(d.els.find((e: any) => e.kind === "scale_bar").ratio).toBe(true);
  await expect(app.locator(".layout-img")).toBeVisible({ timeout: 30_000 }); // it renders
});

test("C3.1 without a map CRS the template is refused with words, no layout is made", async ({ app }) => {
  await newFromTemplate(app, /Map — Letter landscape/);
  await expect(app.locator(".statusbar")).toContainText(/Set a coordinate system for the map/);
  expect(await gis(app, (g) => g.useLayouts.getState().list().length)).toBe(0);
});

test("C3.1 save the open layout as a template, make a layout from it, then delete it", async ({ app }) => {
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await newFromTemplate(app, /Map — Letter landscape/);
  await expect(app.locator(".layout-page")).toBeVisible();
  // A change worth keeping: the title text.
  await gis(app, (g) => { const t = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "text"); g.useLayouts.getState().update(t.id, { text: "My county series" }); });

  app.once("dialog", (d) => void d.accept("E2E county template"));
  await app.getByRole("button", { name: "Save as Template" }).click();
  await expect(app.locator(".statusbar")).toContainText(/Saved the template "E2E county template"/);

  await newFromTemplate(app, /E2E county template/);
  await expect.poll(async () => (await gis(app, (g) => g.useLayouts.getState().list().length))).toBe(2);
  const texts = await gis(app, (g) => g.useLayouts.getState().doc.elements.filter((e: any) => e.kind === "text").map((e: any) => e.text));
  expect(texts).toEqual(["My county series"]);

  // Delete it: asked first, then gone from the gallery.
  app.once("dialog", (d) => void d.accept());
  await app.getByRole("button", { name: /Delete Template/ }).click();
  await app.getByRole("menuitem", { name: "E2E county template" }).click();
  await expect(app.locator(".statusbar")).toContainText(/Deleted the template "E2E county template"/);
  const list = await gis(app, (g) => g.api.listLayoutTemplates());
  expect(list.templates.some((t: any) => t.name === "E2E county template")).toBe(false);
  expect(list.templates.filter((t: any) => t.builtin)).toHaveLength(10);
});
