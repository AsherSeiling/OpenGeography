// C3.2 map series: set it up in the dialog, follow it with a frame, step through the pages, export every page. Cases P21/P22 in the UI.
import { existsSync, mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, ribbon } from "./fixtures";

const NAMES = ["Yuma", "Adams", "Polk", "Cass", "Wake"];

async function setup(app: any) {
  const features = NAMES.map((n, i) => ({ wkt: `POLYGON((${-100 + i * 0.3} 34, ${-99.8 + i * 0.3} 34, ${-99.8 + i * 0.3} 34.2, ${-100 + i * 0.3} 34.2, ${-100 + i * 0.3} 34))`, props: { NAME: n, pop: (i * 7) % 11 } }));
  const layer = await gis(app, async (g, f: any) => { const l = await g.api.createFeatures("Series index", f); g.useApp.getState().addLayers([l]); return l; }, features);
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await ribbon(app, "Layout", /^Map Frame$/);
  await ribbon(app, "Layout", /^Text$/);
  await gis(app, (g) => { const t = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "text"); g.useLayouts.getState().update(t.id, { text: "{page_name} ({page_number} of {page_count})" }); });
  return layer;
}

test("C3.2 set up a series, follow it, step through the pages", async ({ app }) => {
  const layer = await setup(app);
  // The dialog: index layer, name field.
  await app.locator(".rtab.ctx", { hasText: /^Layout$/ }).click();
  await app.getByRole("button", { name: /Set Up Map Series/ }).click();
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByText("Map Series", { exact: true })).toBeVisible();
  await expect(dlg.getByRole("button", { name: "Create series" })).toBeEnabled(); // the first vector layer is already chosen
  await dlg.getByRole("button", { name: "Create series" }).click();
  const series = await gis(app, (g) => g.useLayouts.getState().doc.series);
  expect(series).toMatchObject({ indexLayer: layer.id, nameField: "NAME", extent: "fit", marginPct: 10 });

  // No frame follows yet: stepping says so in the engine's words, nothing crashes.
  // Tick "Follows the map series" on the frame.
  const frameId = await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame").id);
  await gis(app, (g, id) => g.useLayouts.getState().select([id]), frameId);
  await app.getByLabel("Follows the map series").check();
  expect(await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame").followsSeries)).toBe(true);

  // Pages in name order; the caption tracks the page.
  const caption = app.getByTestId("series-caption");
  await expect(caption).toHaveText("5 pages");
  await app.getByRole("button", { name: "Next series page" }).click();
  await expect(caption).toHaveText("Page 1 of 5 — Adams");
  await app.getByRole("button", { name: "Next series page" }).click();
  await expect(caption).toHaveText("Page 2 of 5 — Cass");
  await expect(app.locator(".layout-img")).toBeVisible({ timeout: 30_000 });
  // The engine drew that page: the request carries it, and the preview comes back without warnings about the series.
  const r = await gis(app, async (g) => { const q = g.layoutActions.layoutRequest(g.useLayouts.getState().doc); const p = await g.api.renderLayoutPreview(q, 72); return { page: q.seriesPage, warnings: p.warnings }; });
  expect(r.page).toBe(1);
  expect(r.warnings.filter((w: string) => /series/i.test(w))).toEqual([]);
  await app.getByRole("button", { name: "As saved" }).click();
  await expect(caption).toHaveText("5 pages");
});

test("C3.2 exporting the series writes one PDF with every page, and one PNG per page", async ({ app }) => {
  await setup(app);
  await gis(app, (g) => { const s = g.useLayouts.getState(); const f = s.doc.elements.find((e: any) => e.kind === "map_frame"); const layer = Object.keys(g.useApp.getState().layers)[0]; s.setSeries({ indexLayer: layer, nameField: "NAME", extent: "fit", marginPct: 10 }); s.update(f.id, { followsSeries: true }); });
  await app.locator(".rtab.ctx", { hasText: /^Layout$/ }).click();
  await expect(app.getByTestId("series-caption")).toHaveText("5 pages");
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-series-"));
  const pdf = join(dir, "series.pdf");
  const res = await gis(app, async (g, p: string) => g.api.exportLayoutSeries(g.layoutActions.layoutRequest(g.useLayouts.getState().doc), "pdf", 72, p), pdf);
  expect(res.pages).toBe(5);
  const bytes = readFileSync(pdf).toString("latin1");
  expect(bytes.startsWith("%PDF-")).toBe(true);
  expect(bytes).toMatch(/\/Count 5\b/);
  const pngs = join(dir, "pngs");
  const r2 = await gis(app, async (g, p: string) => g.api.exportLayoutSeries(g.layoutActions.layoutRequest(g.useLayouts.getState().doc), "png", 72, p), pngs);
  expect(r2.files).toHaveLength(5);
  expect(readdirSync(pngs).sort()).toEqual(r2.files.map((f: string) => f.split("/").pop()).sort());
  expect(readdirSync(pngs).every((f) => f.endsWith(".png") && !f.startsWith("."))).toBe(true);
  expect(existsSync(join(pngs, "Layout 1_001_Adams.png"))).toBe(true);
});

test("C3.2 a series with no following frame explains what to do, and an empty index layer is refused in words", async ({ app }) => {
  await setup(app);
  const layer = Object.keys(await gis(app, (g) => g.useApp.getState().layers))[0];
  await gis(app, (g, id) => g.useLayouts.getState().setSeries({ indexLayer: id, nameField: "NAME", extent: "fit", marginPct: 10 }), layer);
  await app.locator(".rtab.ctx", { hasText: /^Layout$/ }).click();
  await expect(app.getByTestId("series-caption")).toHaveText("5 pages");
  const err = await gis(app, async (g) => { try { await g.api.renderLayoutPreview({ ...g.layoutActions.layoutRequest(g.useLayouts.getState().doc), seriesPage: 0 }, 72); return null; } catch (e) { return String(e); } });
  expect(err).toContain("Follows the map series");
  await gis(app, (g) => g.useLayouts.getState().setSeries({ indexLayer: "nope", nameField: "NAME", extent: "fit", marginPct: 10 }));
  await expect(app.getByTestId("series-caption")).toHaveText("Series can't be shown");
});
