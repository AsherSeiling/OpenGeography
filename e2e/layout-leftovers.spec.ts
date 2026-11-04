// WP9 leftovers: the bottom panel closes under a layout and comes back; Items has a column of its own; a new map frames on its first layer;
// pickers read project layers; the Run button is outlined; the start gallery's topo tiles pre-select their preset.
import { test, expect, gis, addTestLayer } from "./fixtures";

async function openLayout(app: any) {
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await gis(app, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  await expect(app.locator(".layout-page")).toBeVisible();
}
const panelIds = (app: any) => app.evaluate(async () => (await import("/src/panes/panes.ts")).getDock()!.panels.map((p: any) => p.id)) as Promise<string[]>;
const backToMap = (app: any) => app.locator(".rtab-back").click();

test("layout problem 1: the Node Editor and Attribute Table close under a layout and return as they were", async ({ app }) => {
  const before = await app.evaluate(async () => {
    const d = (await import("/src/panes/panes.ts")).getDock()!;
    return { ids: d.getPanel("model")!.group.panels.map((p: any) => p.id), active: d.getPanel("model")!.group.activePanel?.id, h: Math.round(d.getPanel("model")!.group.api.height) };
  });
  await openLayout(app);
  await expect(app.locator(".model-canvas")).toHaveCount(0);
  const ids = await panelIds(app);
  expect(ids).not.toContain("model");
  expect(ids).not.toContain("table");
  await backToMap(app);
  await expect(app.locator(".model-canvas")).toBeVisible();
  const after = await app.evaluate(async () => {
    const d = (await import("/src/panes/panes.ts")).getDock()!;
    return { ids: d.getPanel("model")!.group.panels.map((p: any) => p.id), active: d.getPanel("model")!.group.activePanel?.id, h: Math.round(d.getPanel("model")!.group.api.height) };
  });
  expect(after.ids).toEqual(before.ids);
  expect(after.active).toBe(before.active);
  expect(Math.abs(after.h - before.h)).toBeLessThan(8);
});

test("layout problem 1: with no bottom panel, Shift+A and right-click on the page still work", async ({ app }) => {
  await app.evaluate(async () => { const d = (await import("/src/panes/panes.ts")).getDock()!; d.getPanel("model")!.api.close(); d.getPanel("table")!.api.close(); });
  await openLayout(app);
  expect(await panelIds(app)).not.toContain("model");
  const box = (await app.locator(".layout-page").boundingBox())!;
  await app.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await app.locator(".layout-canvas").focus();
  await app.keyboard.press("Shift+A");
  await expect(app.getByRole("dialog", { name: "Insert an element" })).toBeVisible();
  await app.keyboard.press("Escape");
  await app.mouse.click(box.x + 20, box.y + 20, { button: "right" });
  await expect(app.getByRole("menuitem")).toHaveText(["PasteCtrl+V", "Insert…Shift+A", "Page Setup…"]);
  await app.keyboard.press("Escape");
  // Leaving with no bottom panel open opens none.
  await backToMap(app);
  expect(await panelIds(app)).not.toContain("model");
});

test("layout problem 2 (option B): Items is the left column on its own, wide enough to read; Map Layers comes back above Catalog on Back to Map", async ({ app }) => {
  const before = await app.evaluate(async () => {
    const d = (await import("/src/panes/panes.ts")).getDock()!;
    return { w: Math.round(d.getPanel("contents")!.group.api.width), h: Math.round(d.getPanel("contents")!.group.api.height) };
  });
  await openLayout(app);
  const w = await app.evaluate(async () => {
    const d = (await import("/src/panes/panes.ts")).getDock()!;
    const items = d.getPanel("items")!;
    return { alone: items.group.panels.length, width: Math.round(items.group.api.width), contents: !!d.getPanel("contents"), catalog: !!d.getPanel("catalog") };
  });
  expect(w.alone).toBe(1);
  expect(w.contents).toBe(false);
  expect(w.catalog).toBe(true); // stacked under Items, not tabbed with it
  expect(w.width).toBeGreaterThanOrEqual(200);
  const tab = app.locator(".dv-tab", { hasText: /^Items$/ });
  await expect(tab).toBeVisible();
  expect(await tab.evaluate((e: HTMLElement) => e.scrollWidth <= e.clientWidth + 1)).toBe(true);
  await backToMap(app);
  const after = await app.evaluate(async () => {
    const d = (await import("/src/panes/panes.ts")).getDock()!;
    return { ids: d.panels.map((p: any) => p.id), w: Math.round(d.getPanel("contents")!.group.api.width), h: Math.round(d.getPanel("contents")!.group.api.height), same: d.getPanel("contents")!.group !== d.getPanel("catalog")!.group };
  });
  expect(after.ids).not.toContain("items");
  expect(after.ids).toContain("catalog");
  expect(after.same).toBe(true);
  expect(Math.abs(after.w - before.w)).toBeLessThan(8);
  expect(Math.abs(after.h - before.h)).toBeLessThan(8);
});

test("layout problem 2: tab names and item rows stay readable at 1024 px", async ({ app }) => {
  await app.setViewportSize({ width: 1024, height: 800 });
  await addTestLayer(app, "points", 5);
  await openLayout(app);
  await app.locator(".rail-btn", { hasText: /^Text/ }).first().click().catch(() => undefined);
  const clipped = await app.evaluate(() => [...document.querySelectorAll<HTMLElement>(".dv-tab, .items-pane .row .grow")].filter((e) => e.offsetParent && e.scrollWidth > e.clientWidth + 1).map((e) => e.textContent));
  expect(clipped).toEqual([]);
  const items = await app.locator(".dv-tab", { hasText: /^Items$/ }).boundingBox();
  expect(items!.width).toBeGreaterThan(40);
});

test("MM-1: a new map frames on the first layer added to it; a map with layers keeps its view", async ({ app }) => {
  const a = await addTestLayer(app, "points", 5, [-100, 30, -90, 40]);
  const second = await app.evaluate(async () => { const m = await import("/src/maps/mapActions.ts"); return m.newMap({ side: true }); }) as any;
  await expect(app.locator(".map-wrap")).toHaveCount(2);
  const centre = (id: string) => gis(app, (g, i: string) => g.getMap(i).getView().getCenter(), id) as Promise<number[]>;
  const b = await addTestLayer(app, "points", 5, [10, 40, 12, 42]);
  const crs = await gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, second.id);
  expect(crs).toBe("EPSG:4326");
  await expect.poll(async () => { const c = await centre(second.id); return c ? Math.abs(c[0] - 11) + Math.abs(c[1] - 41) : 999; }, { timeout: 10000 }).toBeLessThan(2);
  // A second layer elsewhere does not move the view again.
  const framed = await centre(second.id);
  await addTestLayer(app, "points", 5, [-60, -10, -58, -8]);
  await app.waitForTimeout(1200);
  const now = await centre(second.id);
  expect(Math.abs(now[0] - framed[0]) + Math.abs(now[1] - framed[1])).toBeLessThan(1);
  expect(a.id).not.toBe(b.id);
});

test("MM-2: the Series coverage picker and the Attribute Table list read project layers, not the active map's order", async ({ app }) => {
  const a = await addTestLayer(app, "polygons", 5);
  await app.evaluate(async () => { const m = await import("/src/maps/mapActions.ts"); m.newMap({ side: true }); });
  await expect.poll(() => gis(app, (g) => g.useApp.getState().maps.length)).toBe(2);
  expect(await gis(app, (g) => g.useApp.getState().order)).toEqual([]); // the active map shows nothing
  // Attribute Table lists the layer anyway.
  await gis(app, (g, id: string) => g.useApp.getState().setActiveLayer(id), a.id);
  await app.locator(".dv-tab", { hasText: /^Attribute Table$/ }).click();
  await app.locator(".attr-bar [role=combobox]").first().click();
  await expect(app.getByRole("option", { name: a.name })).toBeVisible();
  await app.keyboard.press("Escape");
  // Series dialog offers it.
  await openLayout(app);
  await app.locator(".rtab", { hasText: /^Page$/ }).click();
  await app.getByRole("button", { name: /Set Up Map Series/ }).click();
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByRole("button", { name: "Create series" })).toBeEnabled();
});

test("the Node Editor's Run button is outlined, not solid", async ({ app }) => {
  const run = app.locator(".model-run");
  await expect(run).toBeVisible();
  const css = await run.evaluate((e: HTMLElement) => { const c = getComputedStyle(e); const s = getComputedStyle(document.body); return { bg: c.backgroundColor, border: c.borderTopWidth, text: c.color, surface2: getComputedStyle(document.documentElement).getPropertyValue("--surface-2").trim(), weight: c.fontWeight, body: s.backgroundColor }; });
  expect(css.border).toBe("1px");
  expect(Number(css.weight)).toBeGreaterThanOrEqual(600);
  expect(css.text).not.toBe("rgb(255, 255, 255)");
});

test("start gallery: Land navigation and USGS quadrangle each pre-select their own Topo Map preset", async ({ app }) => {
  for (const [tile, label] of [["landnav", /Land Nav 1:24,000/], ["quad", /USGS quadrangle 1:24,000 \(classic/]] as const) {
    await openLayout(app);
    await app.getByTestId("start-gallery").locator(`[data-start="${tile}"]`).click();
    const dlg = app.getByRole("dialog");
    await expect(dlg.getByRole("combobox", { name: "Preset" })).toHaveText(label);
    await dlg.getByRole("button", { name: /Cancel|Close/ }).first().click();
    await expect(dlg).toHaveCount(0);
    await backToMap(app);
    await app.evaluate(async () => { const m = await import("/src/layout/actions.ts"); const l = (await import("/src/store/layout.ts")).useLayouts.getState().list(); for (const x of l) m.closeLayoutPanel(x.id); });
  }
});
