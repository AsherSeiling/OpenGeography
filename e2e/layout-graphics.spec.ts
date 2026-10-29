import { test, expect, gis, ribbon, addTestLayer } from "./fixtures";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

const state = (page: any) => gis(page, (g) => { const s = g.useLayouts.getState(); return { selected: s.selected, els: s.doc?.elements ?? [] }; }) as Promise<any>;

test("CG1 shapes, pictures and background/frame/shadow in a layout", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();

  for (const [label, shape] of [["Rectangle", "rectangle"], ["Ellipse", "ellipse"], ["Triangle", "triangle"], ["Line", "line"], ["Arrow", "arrow"]] as const) {
    await ribbon(app, "Layout", new RegExp(`^${label}$`));
    const s = await state(app);
    expect(s.els.find((e: any) => e.id === s.selected[0]).shape).toBe(shape);
  }

  // A picture from a file on the machine running the dev server (the browser build prompts for the path).
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cg1-"));
  const file = path.join(dir, "logo.svg");
  fs.writeFileSync(file, '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#2060c0"/></svg>');
  app.once("dialog", (d) => d.accept(file));
  await ribbon(app, "Layout", /^Picture$/);
  await expect.poll(async () => (await state(app)).els.find((e: any) => e.kind === "picture")?.source).toMatch(/^data:image\/svg\+xml;base64,/);

  // Decor on the selected picture: background, frame and shadow from Properties.
  await app.getByText("Background, frame and shadow").click();
  await app.getByRole("checkbox", { name: "Background" }).check();
  await app.getByRole("checkbox", { name: "Drop shadow" }).check();
  const pic = (await state(app)).els.find((e: any) => e.kind === "picture");
  expect(pic.decor.background).toBe("#ffffff");
  expect(pic.decor.shadow).toBe(true);

  // The engine accepts the whole layout and draws every element.
  const out = await gis(app, async (g) => { const s = g.useLayouts.getState(); return g.api.renderLayoutPreview({ layout: s.doc, styles: {}, order: [], visible: {}, project: "p", date: "2026-01-01" }, 96); }) as any;
  expect(out.warnings).toEqual([]);
});

test("CG2 north arrow styles and scale bar styles from Properties", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  await ribbon(app, "Layout", /^Map Frame$/);
  await gis(app, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { crs: "EPSG:26915", center: [500000, 4649776], scale: 24000, basemap: "none" }); });
  await ribbon(app, "Layout", /^North Arrow$/);
  await app.getByRole("combobox", { name: "Style" }).click();
  await app.getByRole("option", { name: "Compass rose (N E S W)" }).click();
  await app.getByRole("combobox", { name: "Points to" }).click();
  await app.getByRole("option", { name: /Grid north/ }).click();
  const na = await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "north_arrow")) as any;
  expect(na.style).toBe("rose");
  expect(na.north).toBe("grid");
  await ribbon(app, "Layout", /^Scale Bar$/);
  await app.getByRole("combobox", { name: "Style" }).click();
  await app.getByRole("option", { name: "Double (two units)" }).click();
  await app.getByRole("checkbox", { name: /1 cm on the map/ }).check();
  const sb = await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "scale_bar")) as any;
  expect([sb.style, sb.verbal]).toEqual(["double", true]);
  const out = await gis(app, async (g) => { const s = g.useLayouts.getState(); return g.api.renderLayoutPreview({ layout: s.doc, styles: {}, order: [], visible: {}, project: "p", date: "2026-01-01" }, 96); }) as any;
  expect(out.warnings.filter((w: string) => /North arrow|Scale bar/.test(w) && !/too short|does not fit/.test(w))).toEqual([]);
});

test("CG3 legend entries: rename, hide, add an extra entry", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 5, [-93.1, 45.0, -92.9, 45.1]);
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await ribbon(app, "Layout", /^Map Frame$/);
  await gis(app, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { crs: "EPSG:26915", center: [490000, 4985000], scale: 100000, basemap: "none" }); });
  await ribbon(app, "Layout", /^Legend$/);
  const name = app.getByRole("textbox", { name: `Name of ${layer.name}` });
  await expect(name).toBeVisible({ timeout: 30_000 });
  await name.fill("Parcels");
  await app.getByText(/^Extra entries/).click();
  await app.getByRole("button", { name: "Add entry" }).click();
  const lg = await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "legend")) as any;
  expect(lg.overrides[layer.id].label).toBe("Parcels");
  expect(lg.custom).toEqual([{ label: "New entry", color: "#888888" }]);
  await app.locator(".props").getByRole("checkbox", { name: `Show ${layer.name}` }).uncheck();
  const lg2 = await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "legend")) as any;
  expect(lg2.overrides[layer.id]).toEqual({ label: "Parcels", hidden: true });
});
