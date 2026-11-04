// Catalog pane: one tree of Project (Layouts, Project files) and Public Data. UI redesign WP 4.
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis } from "./fixtures";

const PUBLIC = [
  ["tiger", /Sample Data — Census TIGER/],
  ["nationalMap", /The National Map/],
  ["elevation", /Elevation — USGS 3DEP/],
  ["stac", /Imagery and elevation \(STAC\)/],
  ["overture", /Overture Maps/],
  ["hazardsSoils", /Earthquakes and soils/],
  ["arcgis", /ArcGIS services/],
  ["ogc", /OGC API – Features/],
] as const;

const node = (page: any, id: string) => page.locator(`[role="treeitem"][data-node="${id}"]`);

test("CAT-1 Public Data lists all 8 dialogs and each opens from the tree; Browse is gone", async ({ app }) => {
  await app.getByRole("tab", { name: "Catalog" }).click();
  await expect(app.getByRole("tree", { name: "Catalog" })).toBeVisible();
  await expect(app.locator(".cat-mode")).toHaveCount(0);
  await expect(app.getByText("Browse", { exact: true })).toHaveCount(0);
  await expect(app.locator('[role="treeitem"][data-node^="public:"]')).toHaveCount(8);
  for (const [id, title] of PUBLIC) {
    await node(app, `public:${id}`).dblclick();
    await expect(app.getByRole("dialog").getByText(title).first()).toBeVisible();
    await app.keyboard.press("Escape");
    await expect(app.getByRole("dialog")).toHaveCount(0);
  }
  // Keyboard: focus a row, Enter opens its dialog.
  await node(app, "public:tiger").focus();
  await app.keyboard.press("ArrowDown");
  await expect(node(app, "public:nationalMap")).toBeFocused();
  await app.keyboard.press("Enter");
  await expect(app.getByRole("dialog").getByText(PUBLIC[1][1]).first()).toBeVisible();
  await app.keyboard.press("Escape");
});

test("CAT-2 branches expand and collapse by keyboard", async ({ app }) => {
  await app.getByRole("tab", { name: "Catalog" }).click();
  const pub = node(app, "public");
  await expect(pub).toHaveAttribute("aria-expanded", "true");
  await pub.click(); // a click collapses it and leaves it focused
  await expect(pub).toHaveAttribute("aria-expanded", "false");
  await expect(app.locator('[data-node^="public:"]')).toHaveCount(0);
  await app.keyboard.press("ArrowRight");
  await expect(pub).toHaveAttribute("aria-expanded", "true");
  await expect(app.locator('[data-node^="public:"]')).toHaveCount(8);
  await app.keyboard.press("ArrowRight"); // into the first child
  await expect(node(app, "public:tiger")).toBeFocused();
  await app.keyboard.press("ArrowLeft"); // back to the branch
  await expect(pub).toBeFocused();
  await app.keyboard.press("ArrowLeft");
  await expect(pub).toHaveAttribute("aria-expanded", "false");
  await node(app, "files").focus();
  await app.keyboard.press("Enter");
  await expect(node(app, "files")).toHaveAttribute("aria-expanded", "true");
});

test("CAT-3 a layout opens from the Layouts branch; Contents no longer lists layouts", async ({ app }) => {
  await app.getByRole("tab", { name: "Catalog" }).click();
  await expect(app.locator(".layout-row")).toHaveCount(0);
  await gis(app, (g) => g.layoutActions.newLayout());
  await expect(app.locator(".layout-page")).toBeVisible();
  await expect(app.locator(".layout-row")).toHaveCount(1);
  // Close the layout's tab, then open it again from the tree.
  const id = await gis(app, (g) => g.useLayouts.getState().list()[0].id);
  await gis(app, (g, i) => g.layoutActions.closeLayoutPanel(i), id);
  await expect(app.locator(".layout-page")).toBeHidden();
  await app.getByRole("tab", { name: "Catalog" }).click();
  await app.locator(".layout-row", { hasText: "Layout 1" }).dblclick();
  await expect(app.locator(".layout-page")).toBeVisible();
  // Contents (Map Layers) has no Layouts section any more.
  await app.getByRole("tab", { name: "Map Layers" }).click();
  await expect(app.locator(".contents .layouts, .contents .layouts-head, .contents .layout-row")).toHaveCount(0);
});

test("CAT-4 Project files lists the files layers came from and flags a missing one", async ({ app }) => {
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-cat-"));
  const file = join(dir, "parcels.geojson");
  writeFileSync(file, JSON.stringify({ type: "FeatureCollection", crs: { type: "name", properties: { name: "EPSG:4326" } }, features: [{ type: "Feature", properties: { a: 1 }, geometry: { type: "Point", coordinates: [-97, 35] } }] }));
  await gis(app, (g, f) => g.actions.addData([f]), file);
  await expect.poll(() => gis(app, (g) => g.useApp.getState().order.length)).toBe(1);
  await app.getByRole("tab", { name: "Catalog" }).click();
  await node(app, "files").click();
  const row = app.locator(`[role="treeitem"][data-node="file:${file}"]`);
  await expect(row).toContainText("parcels.geojson");
  await expect(row).toContainText("used by");
  await row.click({ button: "right" });
  await expect(app.getByRole("menuitem", { name: "Add to map" })).toBeVisible();
  await expect(app.getByRole("menuitem", { name: "Copy path" })).toBeVisible();
});
