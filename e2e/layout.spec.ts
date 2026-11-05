import { existsSync, mkdtempSync, readFileSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, ribbon, addTestLayer } from "./fixtures";

const MOD = "ControlOrMeta";
const doc = (page: any) => gis(page, (g) => { const d = g.useLayouts.getState().doc; return d && { name: d.name, els: d.elements.map((e: any) => ({ id: e.id, kind: e.kind, x: e.x, y: e.y, w: e.w, h: e.h, scale: e.scale, crs: e.crs, text: e.text, center: e.center })) }; }) as Promise<{ name: string; els: any[] } | null>;
const frame = async (page: any) => (await doc(page))!.els.find((e) => e.kind === "map_frame");

test("M-19 layout: elements, drag/resize, undo, Set from Map View, Frame CRS, scale, export PDF/PNG/SVG, rename/duplicate/delete", async ({ app }) => {
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-layout-"));
  const layer = await addTestLayer(app, "polygons", 30, [-100, 30, -95, 35]);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.waitForTimeout(800);

  // Insert → New Layout ▾ → Letter landscape
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  expect((await doc(app))!.name).toBe("Layout 1");

  // Map Frame, Legend, Scale Bar, North Arrow, Text, Sources
  await ribbon(app, "Layout", /^Map Frame$/);
  await expect.poll(async () => (await doc(app))!.els.some((e) => e.kind === "map_frame")).toBe(true);
  for (const label of ["Legend", "Scale Bar", "North Arrow", "Text", "Sources"]) await ribbon(app, "Layout", new RegExp(`^${label}$`));
  await expect.poll(async () => (await doc(app))!.els.map((e) => e.kind).sort().join()).toBe(["legend", "map_frame", "north_arrow", "scale_bar", "sources", "sources", "text"].join()); // a Map Frame brings its own sources line
  await expect(app.locator(".layout-el")).toHaveCount(7);
  await expect(app.locator(".layout-img")).toBeVisible({ timeout: 20_000 }); // the engine rendered a preview

  // {scale} in a text box is replaced on export (checked in the SVG below)
  const textId = (await doc(app))!.els.find((e) => e.kind === "text").id;
  await gis(app, (g, id) => g.useLayouts.getState().update(id, { text: "Scale is {scale}" }), textId);

  // Drag the map frame, then Ctrl+Z puts it back
  const f0 = await frame(app);
  const el = app.locator(`.layout-el[data-el="${f0.id}"]`);
  await el.click();
  const eb = (await el.boundingBox())!;
  await app.mouse.move(eb.x + eb.width / 2, eb.y + eb.height / 2);
  await app.mouse.down();
  await app.mouse.move(eb.x + eb.width / 2 + 60, eb.y + eb.height / 2 + 30, { steps: 8 });
  await app.mouse.up();
  await expect.poll(async () => (await frame(app)).x).toBeGreaterThan(f0.x + 1);
  // resize by the south-east handle
  const w1 = (await frame(app)).w;
  const hb = (await app.locator(`.layout-el[data-el="${f0.id}"] .layout-handle.se`).boundingBox())!;
  await app.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2);
  await app.mouse.down();
  await app.mouse.move(hb.x + hb.width / 2 - 40, hb.y + hb.height / 2 - 20, { steps: 6 });
  await app.mouse.up();
  await expect.poll(async () => (await frame(app)).w).toBeLessThan(w1);
  await app.locator(".layout-canvas").focus();
  await app.keyboard.press(`${MOD}+z`);
  await expect.poll(async () => (await frame(app)).w).toBe(w1);
  await app.keyboard.press(`${MOD}+z`);
  await expect.poll(async () => (await frame(app)).x).toBe(f0.x);

  // Set from Map View, Frame CRS, scale box
  await ribbon(app, "Layout", /Set from Map View/);
  await expect.poll(async () => (await frame(app)).scale).toBeGreaterThan(0);
  await ribbon(app, "Layout", /^Frame CRS$/);
  await app.getByRole("dialog").getByRole("button", { name: /Web Mercator/ }).click();
  await app.getByRole("dialog").getByRole("button", { name: "OK" }).click();
  await expect.poll(async () => (await frame(app)).crs).toBe("EPSG:3857");
  await app.getByLabel("Map frame scale denominator").fill("24000");
  await expect.poll(async () => (await frame(app)).scale).toBe(24000);

  // Export PDF / PNG / SVG through the browser build's path prompt
  const paths: Record<string, string> = { PDF: join(dir, "out.pdf"), PNG: join(dir, "out.png"), SVG: join(dir, "out.svg") };
  app.on("dialog", async (d) => {
    const fmt = /Export (PDF|PNG|SVG)/.exec(d.message())?.[1];
    if (d.type() === "prompt" && fmt) await d.accept(paths[fmt]);
    else await d.accept();
  });
  for (const fmt of ["PDF", "PNG", "SVG"]) {
    await ribbon(app, "Layout", new RegExp(`^${fmt}$`));
    await expect.poll(() => existsSync(paths[fmt]) && statSync(paths[fmt]).size > 500, { timeout: 60_000 }).toBe(true);
  }
  expect(readFileSync(paths.PDF).subarray(0, 4).toString()).toBe("%PDF");
  expect([...readFileSync(paths.PNG).subarray(0, 4)]).toEqual([0x89, 0x50, 0x4e, 0x47]);
  const svg = readFileSync(paths.SVG, "utf8");
  expect(svg).toContain("<svg");
  expect(svg).not.toContain("{scale}");
  expect(svg).toMatch(/Scale is 1:\s?24,?000/);

  // Rename, duplicate, delete from the Layouts branch of the Catalog tree
  app.removeAllListeners("dialog");
  app.on("dialog", (d) => (d.type() === "prompt" ? d.accept("Site map") : d.accept()));
  await app.getByRole("tab", { name: "Catalog" }).click();
  await app.locator(".layout-row", { hasText: "Layout 1" }).hover(); // row buttons show on hover
  await app.getByRole("button", { name: "Rename Layout 1" }).click();
  await expect(app.locator(".layout-row", { hasText: "Site map" })).toBeVisible();
  await app.locator(".layout-row", { hasText: "Site map" }).hover();
  await app.getByRole("button", { name: "Duplicate Site map" }).click();
  await expect(app.locator(".layout-row")).toHaveCount(2);
  await app.locator(".layout-row", { hasText: "Site map" }).first().hover();
  await app.getByRole("button", { name: /^Delete Site map/ }).first().click();
  await expect(app.locator(".layout-row")).toHaveCount(1);
});

test("UR-5 a 20 m page: Page Setup accepts it, the preview is drawn coarser, PDF exports, a PNG at 300 dpi is refused with a way out", async ({ app, errors }) => {
  errors.expected = true; // the refused PNG raises the error pop-up and logs it
  errors.known.push(/Export of Layout 1\.png failed: A PNG of this 20000/);
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-bigpage-"));
  const layer = await addTestLayer(app, "polygons", 30, [-100, 30, -95, 35]);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await ribbon(app, "Layout", /^Map Frame$/);
  await expect(app.locator(".layout-img")).toBeVisible({ timeout: 20_000 });

  // Through the dialog, so its limits are the ones tested.
  await app.locator(".rtab", { hasText: /^Page$/ }).click();
  await app.locator(".rbtn", { hasText: /Page Setup/ }).click();
  const dlg = app.getByRole("dialog");
  await dlg.getByLabel("Width (mm)").fill("20001");
  await expect(dlg).toContainText("20,000 mm");
  await expect(dlg.getByRole("button", { name: "OK" })).toBeDisabled();
  await dlg.getByLabel("Width (mm)").fill("20000");
  await dlg.getByLabel("Height (mm)").fill("14000");
  await expect(dlg).toContainText("Acrobat");
  await dlg.getByRole("button", { name: "OK" }).click();
  await expect.poll(() => gis(app, (g) => g.useLayouts.getState().doc.page.width)).toBe(20000);

  // The preview still draws, at a lower resolution, and says so.
  await expect(app.locator(".layout-stat")).toContainText("lowered", { timeout: 60_000 });
  await expect(app.locator(".layout-img")).toBeVisible();

  const exportAs = async (fmt: string, dpi: number, file: string) => {
    const path = join(dir, file);
    await gis(app, async (g, a: any) => { window.prompt = () => a.path; await g.layoutActions.exportLayout(a.fmt, a.dpi); }, { path, fmt, dpi });
    return { path, status: (await gis(app, (g) => g.useApp.getState().status)) as string };
  };
  const pdf = await exportAs("pdf", 300, "big.pdf");
  expect(statSync(pdf.path).size).toBeGreaterThan(500);
  expect(readFileSync(pdf.path).subarray(0, 5).toString()).toBe("%PDF-");
  const png = await exportAs("png", 300, "big.png");
  expect(png.status).toMatch(/million pixels|million/);
  expect(png.status).toContain("or less for this page");
  expect(existsSync(png.path)).toBe(false);
  // The dpi it suggests (37 for this page) works.
  const ok = await exportAs("png", 30, "ok.png");
  expect(ok.status).toContain("Exported");
  expect(statSync(ok.path).size).toBeGreaterThan(500);
});
