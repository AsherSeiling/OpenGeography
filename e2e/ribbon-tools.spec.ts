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
