// C3.0 layout polish: rulers, Arrange, export dpi box, scale bar ratio. Cases C30-* of docs/TEST_PLAN.md.
import { test, expect, gis, ribbon } from "./fixtures";

async function newLayout(app: any) {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
}

test("C3.0 rulers show millimetres along the page and follow zoom", async ({ app }) => {
  await newLayout(app);
  const h = app.getByTestId("ruler-h");
  await expect(h.locator("text").first()).toBeVisible();
  const labels = async () => (await h.locator("text").allTextContents()).map(Number);
  const before = await labels();
  expect(before).toContain(0);
  expect(before.every((n) => n % 10 === 0 || n % 5 === 0 || n % 2 === 0 || n % 1 === 0)).toBe(true);
  await app.getByRole("button", { name: "+", exact: true }).click(); // zoom in
  await expect.poll(async () => (await labels()).length).not.toBe(before.length);
  await expect(app.getByTestId("ruler-v").locator("text").first()).toBeVisible();
});

test("C3.0 Arrange: order and align the selected elements, undo restores", async ({ app }) => {
  await newLayout(app);
  await ribbon(app, "Layout", /^Text$/);
  await ribbon(app, "Layout", /^Text$/);
  const els = () => gis(app, (g) => g.useLayouts.getState().doc.elements.map((e: any) => ({ id: e.id, x: e.x }))) as Promise<{ id: string; x: number }[]>;
  const [first, second] = await els();
  await gis(app, (g, a: any) => { g.useLayouts.getState().update(a.second, { x: 120 }); g.useLayouts.getState().select([a.first]); }, { first: first.id, second: second.id });
  await app.locator(".rtab.ctx", { hasText: /^Format$/ }).click();
  await app.getByRole("button", { name: /Order/ }).click();
  await app.getByRole("menuitem", { name: "Bring to Front" }).click();
  expect((await els()).map((e) => e.id)).toEqual([second.id, first.id]);
  await gis(app, (g, a: any) => g.useLayouts.getState().select([a.first, a.second]), { first: first.id, second: second.id });
  await app.getByRole("button", { name: /Align/ }).click();
  await app.getByRole("menuitem", { name: "Align Left" }).click();
  const x = (await els()).map((e) => e.x);
  expect(x[0]).toBe(x[1]);
  await gis(app, (g) => g.useLayouts.getState().undo());
  expect(new Set((await els()).map((e) => e.x)).size).toBe(2);
});

test("C3.0 the dpi box takes 72–600 only, is remembered, and the scale bar can print its ratio", async ({ app }) => {
  await newLayout(app);
  await app.locator(".rtab.ctx", { hasText: /^Export$/ }).click();
  const dpi = app.getByLabel("Export resolution in dots per inch");
  await expect(dpi).toHaveValue("300");
  await dpi.fill("150");
  await expect.poll(() => gis(app, () => localStorage.getItem("gis-studio.exportDpi"))).toBe("150");
  await dpi.fill("30"); // too low: ignored
  expect(await gis(app, () => localStorage.getItem("gis-studio.exportDpi"))).toBe("150");
  await dpi.blur();
  await expect(dpi).toHaveValue("150"); // shows the last good value again

  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await ribbon(app, "Layout", /^Map Frame$/);
  await ribbon(app, "Layout", /^Scale Bar$/);
  await app.getByLabel("Also print the ratio (1:N) under the bar").check();
  expect(await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "scale_bar").ratio)).toBe(true);
});
