// RB2 / RB3: the Analysis tool dropdowns, and the conditional groups on View.
import { test, expect, gis, addTestLayer } from "./fixtures";

test("RB2 an Analysis dropdown lists tools from tool_specs and picking one does what Shift+A does", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.locator(".rtab", { hasText: /^Analysis$/ }).click();
  const vector = app.locator('[data-tool-menu="Vector"]');
  await expect(vector).toBeVisible();
  // The dropdown's tool count comes from the engine's list.
  const expected = await gis(app, (g) => Object.values(g.useRun.getState().specs as Record<string, any>).filter((s: any) => s.role === "tool" && s.category.startsWith("Vector")).length);
  await vector.click();
  await expect(app.getByRole("menuitem")).toHaveCount(expected);
  await app.getByRole("menuitem", { name: /^Buffer$/ }).click();
  await expect.poll(() => gis(app, (g) => g.useModel.getState().nodes.some((n: any) => n.data?.tool === "buffer"))).toBe(true);
});

test("RB3 the Developer group on View is hidden until Settings > Show developer tools is on", async ({ app }) => {
  await app.setViewportSize({ width: 2560, height: 1000 });
  await app.locator(".rtab", { hasText: /^View$/ }).click();
  await expect(app.locator('.rg-slot[data-group="Developer"]')).toHaveCount(0);
  await expect(app.locator(".rbtn", { hasText: /^Frame Rate$/ })).toHaveCount(0);
  // The dev hook turns the setting on without writing the user's settings file.
  await gis(app, (g) => g.useSettings.setState({ settings: { ...g.useSettings.getState().settings, showDeveloperTools: true } }));
  await expect(app.locator('.rg-slot[data-group="Developer"]')).toHaveCount(1);
  await expect(app.locator(".rbtn", { hasText: /^Generate Test Data$/ })).toBeVisible();
});

test("RB3 View > Zoom appears while a layout is open and not on the map", async ({ app }) => {
  await app.setViewportSize({ width: 2560, height: 1000 });
  await app.locator(".rtab", { hasText: /^View$/ }).click();
  await expect(app.locator('.rg-slot[data-group="Zoom"]')).toHaveCount(0);
  await gis(app, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  await app.locator(".rtab", { hasText: /^View$/ }).click();
  await expect(app.locator('.rg-slot[data-group="Zoom"]')).toHaveCount(1);
});
