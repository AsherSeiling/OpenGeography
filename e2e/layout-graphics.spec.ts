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

test("CG4 overview map outlines the main frame's extent", async ({ app }) => {
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await ribbon(app, "Layout", /^Map Frame$/);
  await gis(app, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { center: [500000, 4649776], scale: 24000, basemap: "none" }); });
  await ribbon(app, "Layout", /^Overview Map$/);
  const fr = await gis(app, (g) => g.useLayouts.getState().doc.elements.filter((e: any) => e.kind === "map_frame")) as any[];
  expect(fr).toHaveLength(2);
  expect(fr[1].indicator).toEqual({ frame: fr[0].id });
  expect(fr[1].scale).toBeGreaterThan(fr[0].scale * 5);
  // Properties offers the choice and the engine draws it without complaint.
  await expect(app.getByText(/Overview map \(show another frame/)).toBeVisible();
  const out = await gis(app, async (g) => { const s = g.useLayouts.getState(); return g.api.renderLayoutPreview({ layout: s.doc, styles: {}, order: [], visible: {}, project: "p", date: "2026-01-01" }, 96); }) as any;
  expect(out.warnings.filter((w: string) => /overview rectangle/.test(w))).toEqual([]);
});

test("CG5 table and chart elements from a layer's attributes", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 6, [-93.1, 45.0, -92.9, 45.1]);
  await gis(app, (g, a: any) => g.useApp.setState({ activeLayer: a.id }), { id: layer.id });
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await ribbon(app, "Layout", /^Table$/);
  const state = () => gis(app, (g) => g.useLayouts.getState().doc.elements) as Promise<any[]>;
  expect((await state()).find((e) => e.kind === "table").layer).toBe(layer.id);
  await app.getByRole("checkbox", { name: "Striped rows" }).uncheck();
  expect((await state()).find((e) => e.kind === "table").zebra).toBe(false);
  await ribbon(app, "Layout", /^Chart$/);
  const field = layer.fields.find((f: any) => /INT|VARCHAR/i.test(f.type) && !f.name.startsWith("__"))?.name ?? layer.fields[0].name;
  await app.getByRole("combobox", { name: "Category field" }).click();
  await app.getByRole("option", { name: field, exact: true }).click();
  expect((await state()).find((e) => e.kind === "chart").category).toBe(field);
  const out = await gis(app, async (g) => { const s = g.useLayouts.getState(); return g.api.renderLayoutPreview({ layout: { ...s.doc, author: "Me" }, styles: {}, order: [], visible: {}, project: "p", date: "2026-01-01" }, 96); }) as any;
  expect(out.warnings.filter((w: string) => /Table|Chart/.test(w))).toEqual([]);
});

test("CG6 page setup, guides, snapping, spread, group and the element list", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  const doc = () => gis(app, (g) => g.useLayouts.getState().doc) as Promise<any>;

  // Page Setup: bleed, crop marks, author and a vertical guide.
  await app.locator(".rbtn", { hasText: /Page Setup/ }).click();
  await app.getByLabel("Bleed (mm)").fill("3");
  await app.getByRole("checkbox", { name: /Crop marks/ }).check();
  await app.getByLabel(/Author/).fill("A. Mapper");
  await app.getByRole("button", { name: "Add vertical" }).click();
  await app.getByLabel("Guide 1 position").fill("100");
  await app.getByRole("button", { name: "OK" }).click();
  let d = await doc();
  expect([d.page.bleed, d.page.cropMarks, d.author, d.guides]).toEqual([3, true, "A. Mapper", [{ axis: "x", pos: 100 }]]);
  await expect(app.locator('[data-guide="x:100"]')).toBeVisible();

  // Snapping: dragging a rectangle so its left edge ends 0.6 mm from the guide lands on it.
  await ribbon(app, "Layout", /^Rectangle$/);
  const id = d.elements.length ? d.elements[0].id : (await doc()).elements[0].id;
  const el = app.locator(`[data-el="${id}"]`);
  const page = await app.locator(".layout-page").boundingBox();
  const z = page!.width / d.page.width;
  const b = (await el.boundingBox())!;
  const startX = (await doc()).elements[0].x;
  const wantDx = (100.6 - startX) * z;
  await app.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await app.mouse.down();
  await app.mouse.move(b.x + b.width / 2 + wantDx, b.y + b.height / 2, { steps: 6 });
  await app.mouse.up();
  expect((await doc()).elements[0].x).toBe(100);

  // Three shapes: spread them, group them, and the element list shows the group.
  await ribbon(app, "Layout", /^Ellipse$/);
  await ribbon(app, "Layout", /^Triangle$/);
  d = await doc();
  const ids: string[] = d.elements.map((e: any) => e.id);
  await gis(app, (g, a: any) => { const s = g.useLayouts.getState(); s.update(a.ids[0], { x: 10, y: 10, w: 20, h: 10 }); s.update(a.ids[1], { x: 40, y: 10, w: 20, h: 10 }); s.update(a.ids[2], { x: 150, y: 10, w: 20, h: 10 }); s.select(a.ids); s.distribute("horizontal"); }, { ids });
  d = await doc();
  expect(d.elements.map((e: any) => e.x)).toEqual([10, 80, 150]);
  await gis(app, (g) => g.useLayouts.getState().group());
  d = await doc();
  expect(new Set(d.elements.map((e: any) => e.group)).size).toBe(1);
  await gis(app, (g, a: any) => g.useLayouts.getState().select([a.id]), { id: ids[0] });
  expect((await gis(app, (g) => g.useLayouts.getState().selected)) as string[]).toHaveLength(3);
  await expect(app.locator(".layout-outline .row")).toHaveCount(3);
  await app.locator(`.layout-outline [data-element="${ids[1]}"] input[type=checkbox]`).uncheck();
  expect((await doc()).elements.find((e: any) => e.id === ids[1]).visible).toBe(false);
});

test("CG7 frame shape from Properties and a layer shadow in the layout", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 4, [-93.1, 45.0, -92.9, 45.1]);
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await ribbon(app, "Layout", /^Map Frame$/);
  await gis(app, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { crs: "EPSG:26915", center: [490000, 4985000], scale: 100000, basemap: "none" }); });
  await app.getByRole("combobox", { name: "Frame shape" }).click();
  await app.getByRole("option", { name: "Outline of a polygon layer" }).click();
  const f = await gis(app, (g) => g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame")) as any;
  expect(f.clip).toEqual({ shape: "layer", layer: layer.id });
  await gis(app, (g, a: any) => g.useApp.getState().setStyle(a.id, { ...g.useApp.getState().styles[a.id], effect: { dx: 1, dy: 1, color: "#000000", opacity: 0.3 } }), { id: layer.id });
  const out = await gis(app, async (g) => { const s = g.useLayouts.getState(); const st = g.useApp.getState(); return g.api.renderLayoutPreview({ layout: s.doc, styles: st.styles, order: st.order, visible: st.visible, project: "p", date: "2026-01-01" }, 96); }) as any;
  expect(out.warnings.filter((w: string) => /clip shape|polygon layer/.test(w))).toEqual([]);
});
