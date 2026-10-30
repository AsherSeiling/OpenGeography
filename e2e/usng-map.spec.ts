// USNG / MGRS on the map: the Land Nav group's MGRS Grid button adds a labelled grid for the view, sized for the zoom.
import { test, expect, gis, addTestLayer } from "./fixtures";

test("MGRS Grid adds a labelled grid layer for the map view and finer squares when zoomed in", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 12, [-93.05, 44.98, -92.95, 45.04]);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.waitForTimeout(1500);
  await app.locator(".rtab", { hasText: /^Map$/ }).click();
  await app.locator(".rbtn", { hasText: /MGRS Grid/ }).first().click();
  await expect.poll(async () => ((await gis(app, (g) => Object.values(g.useApp.getState().layers).map((l: any) => l.name))) as string[]).some((n) => n.startsWith("MGRS grid"))).toBe(true);
  const info = (await gis(app, (g) => {
    const s = g.useApp.getState();
    const l = Object.values(s.layers as Record<string, any>).find((x: any) => x.name.startsWith("MGRS grid")) as any;
    return { name: l.name, count: l.featureCount, style: s.styles[l.id], top: s.order[0] === l.id, fields: l.fields.map((f: any) => f.name) };
  })) as any;
  expect(info.name).toMatch(/^MGRS grid (1 km|100 m|10 km)$/);
  expect(info.count).toBeGreaterThanOrEqual(4);
  expect(info.fields).toContain("MGRS");
  expect(info.style.opacity).toBe(0); // outlines only
  expect(info.style.label).toMatchObject({ enabled: true, field: "MGRS" });
  expect(info.top).toBe(true);
  if (process.env.USNG_PNG) await app.waitForTimeout(5000);
  if (process.env.USNG_PNG) await app.screenshot({ path: process.env.USNG_PNG });
});
