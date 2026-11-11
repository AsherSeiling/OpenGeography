import { cpSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer, ribbon } from "./fixtures";

const status = (page: any) => page.locator(".statusbar");

test("M-4 map CRS: UTM 14N, Web Mercator, polar 3031 with dropped note", async ({ app }) => {
  await addTestLayer(app, "polygons", 60, [-170, -60, 170, 70]); // a world-wide layer
  await ribbon(app, "Coordinates", "Coordinate System");
  const dlg = app.getByRole("dialog");
  await dlg.getByRole("combobox").or(dlg.getByRole("searchbox")).or(dlg.locator("input").first()).first().fill("26914");
  await dlg.getByText(/NAD83 \/ UTM zone 14N$/).first().click();
  await dlg.getByRole("button", { name: "OK" }).click();
  await expect(status(app).locator(".sb-crs")).toContainText(/UTM zone 14N/);
  expect((await gis(app, (g) => g.useApp.getState().mapCrs)).id).toBe("EPSG:26914");
  await expect(app.locator(".map canvas").first()).toBeVisible();

  await ribbon(app, "Coordinates", "Coordinate System");
  await app.getByRole("dialog").getByRole("button", { name: /Web Mercator/ }).click();
  await app.getByRole("dialog").getByRole("button", { name: "OK" }).click();
  await expect(status(app).locator(".sb-crs")).toContainText(/Pseudo-Mercator|Web Mercator/);

  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:3031", name: "WGS 84 / Antarctic Polar Stereographic" }));
  await expect(app.locator(".map-notice.warn")).toContainText(/outside the area of use/, { timeout: 20_000 });
  await expect(app.locator(".map canvas").first()).toBeVisible(); // not blank: map still rendered
});

const view = (page: any) => gis(page, (g) => { const v = g.getMap().getView(); return { center: v.getCenter(), res: v.getResolution() }; });
const mapBox = async (page: any) => (await page.locator(".map-wrap").boundingBox())!;

test("M-5 navigation: wheel zoom, drag pan, double-click zoom, Full Extent, Zoom To Layer", async ({ app }) => {
  const info = await addTestLayer(app, "points", 300);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), info.id);
  await app.waitForTimeout(800);
  const b = await mapBox(app);
  const cx = b.x + b.width / 2, cy = b.y + b.height / 2;
  const v0 = await view(app);

  await app.mouse.move(cx, cy);
  await app.mouse.wheel(0, -400);
  await expect.poll(async () => (await view(app)).res).toBeLessThan(v0.res);
  const v1 = await view(app);

  await app.mouse.move(cx, cy);
  await app.mouse.down();
  await app.mouse.move(cx + 150, cy + 80, { steps: 8 });
  await app.mouse.up();
  await expect.poll(async () => (await view(app)).center[0]).toBeLessThan(v1.center[0]); // dragged right → centre moves west

  const v2 = await view(app);
  await app.mouse.dblclick(cx + 100, cy + 50);
  await expect.poll(async () => (await view(app)).res).toBeLessThan(v2.res);

  await ribbon(app, "Map", "Full Extent");
  await expect.poll(async () => (await view(app)).res).toBeGreaterThan(v2.res);

  // Zoom To Layer from the Contents context menu
  await gis(app, (g) => g.mapController.zoomToExtent([0, 0, 1, 1]));
  await app.locator(".tree-layer", { hasText: "Test points" }).click({ button: "right" });
  await app.getByRole("menuitem", { name: /Zoom To Layer/ }).click();
  await expect.poll(async () => { const c = (await view(app)).center; return Math.abs(c[0] + 95) < 6 && Math.abs(c[1] - 35) < 6; }).toBe(true);
});

test("M-7 coordinate readout follows the pointer and ends on the last position", async ({ app }) => {
  await addTestLayer(app, "points", 50);
  await app.waitForTimeout(800);
  const b = await mapBox(app);
  const pts = Array.from({ length: 12 }, (_, i) => [b.x + 200 + i * 30, b.y + 150 + i * 12]);
  for (const [x, y] of pts) await app.mouse.move(x, y);
  const [lx, ly] = pts[pts.length - 1];
  const expected = await gis(app, (g, p: any) => {
    const m = g.getMap(); const r = m.getTargetElement().getBoundingClientRect();
    const c = m.getCoordinateFromPixel([p[0] - r.left, p[1] - r.top]);
    return g.mapController.getActiveCrs().id === "EPSG:4326" ? c : null;
  }, [lx, ly]);
  await expect(app.locator(".sb-coords")).toContainText("°");
  await expect.poll(async () => { const p = await gis(app, (g) => g.useMapView.getState().pointer); return p ? Math.abs(p.lon - expected[0]) + Math.abs(p.lat - expected[1]) : 99; }).toBeLessThan(0.01);
  // One format at a time (UX14): choose degrees minutes seconds.
  await app.locator(".sb-format").selectOption("dms");
  await app.mouse.move(lx + 1, ly + 1);
  await expect(app.locator(".sb-coords")).toContainText(/\d+°\d{2}′/); // DMS
  // projected CRS with the map coordinate format: shows E/N
  await app.locator(".sb-format").selectOption("projected");
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:32614", name: "WGS 84 / UTM zone 14N" }));
  await app.waitForTimeout(500);
  await app.mouse.move(b.x + 300, b.y + 200);
  await app.mouse.move(b.x + 320, b.y + 210);
  await expect(app.locator(".sb-coords")).toContainText(/ E .* N/);
});

test("M-8 panes toggle and reset; dark theme", async ({ app }) => {
  const tabOf = (name: string) => app.locator(".dv-default-tab, .dv-tab").filter({ hasText: new RegExp(`^${name}$`) });
  await expect(tabOf("Sources")).toHaveCount(0); // not in the default layout (UR-3); Data › Sources opens it
  await ribbon(app, "Data", /^Sources$/);
  await expect(tabOf("Sources").first()).toBeVisible();
  await app.locator(".rbtn", { hasText: /^Sources$/ }).first().click(); // it is a toggle
  await expect(tabOf("Sources")).toHaveCount(0);
  await app.locator(".rbtn", { hasText: /^Sources$/ }).first().click();
  await app.locator(".rtab", { hasText: /^View$/ }).click();
  for (const [label, tab] of [["Map Layers", "Map Layers"], ["Attribute Table", "Attribute Table"], ["Properties", "Properties"], ["Processing nodes", "Processing nodes"], ["Catalog", "Catalog"]] as const) {
    const btn = app.locator(".rbtn", { hasText: new RegExp(`^${label}$`) }).first();
    await expect(tabOf(tab).first()).toBeVisible();
    await btn.click();
    await expect(tabOf(tab)).toHaveCount(0);
    await btn.click();
    await expect(tabOf(tab).first()).toBeVisible();
  }
  await app.locator(".rbtn", { hasText: /^Map Layers$/ }).first().click();
  await ribbon(app, null, "Reset Panes");
  await expect(tabOf("Map Layers").first()).toBeVisible();
  const was = await gis(app, (g) => g.useApp.getState().dark);
  await ribbon(app, null, "Dark Theme");
  expect(await gis(app, (g) => g.useApp.getState().dark)).toBe(!was);
  await ribbon(app, null, "Dark Theme");
  expect(await gis(app, (g) => g.useApp.getState().dark)).toBe(was);
});

// ---- Shell (WP 6): default layout, the bottom panel that exists only when opened, and saved layouts ----
const dockTab = (page: any, name: string) => page.locator(".dv-tab").filter({ hasText: new RegExp(`^${name}$`) });
const box = async (loc: any) => (await loc.first().boundingBox())!;
const groupOf = (page: any, tab: string) => page.locator(".dv-groupview", { has: dockTab(page, tab) });

test.describe("shell with the bottom panel closed at start-up", () => {
  test.use({ bottom: false });

  test("WP6 default layout: Map Layers over Catalog, Properties on the right, no Sources, no bottom panel (UR-3)", async ({ app }) => {
    for (const t of ["Map Layers", "Catalog", "Map", "Properties"]) await expect(dockTab(app, t)).toHaveCount(1);
    for (const t of ["Sources", "Processing nodes", "Attribute Table", "Geoprocessing"]) await expect(dockTab(app, t)).toHaveCount(0);
    await expect(app.locator(".model-canvas")).toHaveCount(0);
    const dock = await box(app.locator(".dock"));
    const [layers, catalog, map, props] = await Promise.all(["Map Layers", "Catalog", "Map", "Properties"].map((t) => box(groupOf(app, t))));
    // Left column about 15 % and right about 21 % of the window; the map takes the rest and the whole height.
    expect(layers.width / dock.width).toBeGreaterThan(0.11);
    expect(layers.width / dock.width).toBeLessThan(0.19);
    expect(props.width / dock.width).toBeGreaterThan(0.17);
    expect(props.width / dock.width).toBeLessThan(0.25);
    expect(map.height).toBeGreaterThan(dock.height - 3);
    // Stacked, not tabbed: Catalog sits under Map Layers, Sources under Properties, in the same column.
    expect(catalog.y).toBeGreaterThan(layers.y + layers.height - 3);
    expect(Math.abs(catalog.x - layers.x)).toBeLessThan(3);
    expect(layers.height / (layers.height + catalog.height)).toBeLessThan(0.4);
    // Properties takes the whole right column. Sources is still in the View menu, and opens under Properties.
    expect(props.height).toBeGreaterThan(dock.height - 3);
    await ribbon(app, "Data", /^Sources$/);
    await expect(dockTab(app, "Sources")).toHaveCount(1);
    const sources = await box(groupOf(app, "Sources"));
    expect(sources.y).toBeGreaterThan((await box(groupOf(app, "Properties"))).y);
  });

  test("WP6 the bottom panel appears when Node Editor or Attribute Table is opened and goes when the last one closes", async ({ app }) => {
    const full = (await box(groupOf(app, "Map"))).height;
    await ribbon(app, "View", /^Processing nodes$/);
    await expect(dockTab(app, "Processing nodes")).toHaveCount(1);
    await expect(app.locator(".model-canvas")).toBeVisible();
    const withPanel = (await box(groupOf(app, "Map"))).height;
    expect(withPanel).toBeLessThan(full * 0.7); // the panel takes about 47 %
    await ribbon(app, "View", /^Attribute Table$/);
    await expect(dockTab(app, "Attribute Table")).toHaveCount(1);
    // Both are tabs of the one panel under the map.
    expect(await groupOf(app, "Processing nodes").locator(".dv-tab").filter({ hasText: /^Attribute Table$/ }).count()).toBe(1);
    // Close one: the panel stays. Close the other: it is gone and the map fills the centre.
    await dockTab(app, "Attribute Table").locator(".dv-default-tab-action").click();
    await expect(dockTab(app, "Attribute Table")).toHaveCount(0);
    await expect(app.locator(".model-canvas")).toBeVisible();
    await dockTab(app, "Processing nodes").locator(".dv-default-tab-action").click();
    await expect(dockTab(app, "Processing nodes")).toHaveCount(0);
    await expect(app.locator(".model-canvas")).toHaveCount(0);
    await expect.poll(async () => (await box(groupOf(app, "Map"))).height).toBeGreaterThan(full - 3);
  });

  test("UR-2 a project saved with the old tab title 'Node Editor' opens with 'Processing nodes'", async ({ app }) => {
    await ribbon(app, "View", /^Processing nodes$/);
    await expect(dockTab(app, "Processing nodes")).toHaveCount(1);
    await app.evaluate(() => {
      const g = (window as any).__gis.panes;
      const saved = JSON.parse(JSON.stringify(g.serializeDock()));
      saved.layout.panels.model.title = "Node Editor"; // what a project saved before the rename holds
      g.restoreDock(saved);
    });
    await expect(dockTab(app, "Processing nodes")).toHaveCount(1);
    await expect(dockTab(app, "Node Editor")).toHaveCount(0);
  });

  test("WP6 picking a tool from the Shift+A menu opens the Node Editor; Run and Add Tool are on the Analysis tab", async ({ app }) => {
    await addTestLayer(app, "points", 20);
    await expect(dockTab(app, "Processing nodes")).toHaveCount(0);
    await app.locator(".rtab", { hasText: /^Analysis$/ }).click();
    await expect(app.locator(".rbtn", { hasText: /^Run$/ })).toBeVisible();
    await app.locator(".rbtn", { hasText: /^Add Tool$/ }).click();
    await expect(app.locator(".toolmenu")).toBeVisible();
    await app.keyboard.type("buffer");
    await app.keyboard.press("Enter");
    await expect(dockTab(app, "Processing nodes")).toHaveCount(1);
    await expect(app.locator(".model-canvas")).toBeVisible();
    await expect.poll(() => gis(app, (g) => g.useModel.getState().nodes.some((n: any) => n.data.tool === "buffer"))).toBe(true);
    // The favourites shelf lists what was just used.
    await expect(app.getByTestId("tool-shelf")).toContainText("Buffer");
  });

  test("WP6 search finds a command, a tool and a layer; picking a tool places it like the menu", async ({ app }) => {
    await addTestLayer(app, "points", 20);
    const box = app.getByPlaceholder("Search commands and tools");
    await app.keyboard.press("Alt+q");
    await expect(box).toBeFocused();
    await box.fill("coord");
    await expect(app.getByRole("option", { name: /Coordinate System/ }).first()).toBeVisible();
    await box.fill("test poi");
    await expect(app.getByRole("option", { name: /Test points.*Layer/ })).toBeVisible();
    await box.fill("cut"); // a plain word: Clip
    await expect(app.getByRole("option", { name: /^Clip\s*Tool/ })).toBeVisible();
    await app.getByRole("option", { name: /^Clip\s*Tool/ }).click();
    await expect(dockTab(app, "Processing nodes")).toHaveCount(1);
    await expect.poll(() => gis(app, (g) => g.useModel.getState().nodes.some((n: any) => n.data.tool === "clip"))).toBe(true);
    await box.fill("zzzzqq");
    await expect(app.getByText(/Nothing matches/)).toBeVisible();
  });

  test("WP6 Public Data opens the Catalog if it is closed and puts the focus on the Public Data row", async ({ app }) => {
    await gis(app, async () => { const m = await import("/src/panes/panes.ts"); m.togglePane("catalog"); });
    await expect(dockTab(app, "Catalog")).toHaveCount(0);
    await ribbon(app, "Data", /^Public Data/);
    await expect(dockTab(app, "Catalog")).toHaveCount(1);
    await expect(app.locator('[role="treeitem"][data-node="public"]')).toBeFocused();
    await expect(app.locator('[role="treeitem"][data-node^="public:"]')).toHaveCount(8);
  });

  test("WP6 a saved layout of this version comes back; an old one falls back to the default and the project data is untouched", async ({ app }) => {
    const info = await addTestLayer(app, "points", 20);
    await ribbon(app, "View", /^Processing nodes$/);
    await expect(dockTab(app, "Processing nodes")).toHaveCount(1);
    const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-dock-")), "dock.gproj");
    await gis(app, (g, d) => g.useApp.setState({ projectDir: d }), dir);
    await gis(app, (g) => g.actions.saveProject());
    await expect.poll(() => gis(app, (g) => g.useApp.getState().dirty)).toBe(false);
    await ribbon(app, "View", /^Reset Panes$/);
    await expect(dockTab(app, "Processing nodes")).toHaveCount(0);
    await gis(app, (g, d) => g.actions.openProject(d), dir);
    await expect(dockTab(app, "Processing nodes")).toHaveCount(1); // this version's layout is restored
    // Now a project saved by the old shell: raw dockview JSON, naming the removed Geoprocessing pane.
    const old = join(mkdtempSync(join(tmpdir(), "gis-e2e-olddock-")), "old.gproj");
    await gis(app, async (g, a: any) => {
      const ui = { version: 1, order: [a.id], visible: { [a.id]: true }, styles: {}, mapCrs: null, basemap: "none", camera: null, trees: {}, dock: { grid: {}, panels: { map: { id: "map" }, tools: { id: "tools" }, model: { id: "model" } } } };
      await g.api.saveProject(a.dir, JSON.stringify(ui));
    }, { id: info.id, dir: old });
    await gis(app, (g, d) => g.actions.openProject(d), old);
    for (const t of ["Map Layers", "Catalog", "Map", "Properties"]) await expect(dockTab(app, t)).toHaveCount(1);
    for (const t of ["Sources", "Processing nodes", "Attribute Table", "Geoprocessing"]) await expect(dockTab(app, t)).toHaveCount(0);
    expect(await gis(app, (g) => Object.keys(g.useApp.getState().layers).length)).toBe(1); // the data came with it
  });
});

test("U-6 (guard) a layer with the id and version of one already drawn in UTM, in a project with no UTM table, still draws in UTM", async ({ app }) => {
  const UTM = { id: "EPSG:26914", name: "NAD83 / UTM zone 14N" };
  const draws = async (id: string) =>
    // A tile of the layer's own source answers with data (the engine 404s until the display table is built).
    gis(app, async (g, lid: string) => {
      const m = g.getMap();
      const lay = m.getLayers().getArray().find((l: any) => l.getSource?.()?.getTileUrlFunction && l.getSource().getUrls?.()?.[0]?.includes(`/${lid}/`));
      if (!lay) return "no layer";
      const src = lay.getSource();
      const coord = src.getTileGrid().getTileCoordForCoordAndZ(m.getView().getCenter(), 8);
      const r = await fetch(src.getTileUrlFunction()(coord, 1, src.getProjection()));
      return r.ok ? "ok" : `HTTP ${r.status}`;
    }, id) as Promise<string>;
  // 1. A layer in Web Mercator; the project is saved before any UTM display table exists.
  const layer = await addTestLayer(app, "polygons", 12, [-100, 34, -99.7, 34.3]);
  const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-u6-")), "u6.gproj");
  await gis(app, (g, d) => g.useApp.setState({ projectDir: d }), dir);
  await gis(app, (g) => g.actions.saveProject());
  await expect.poll(() => gis(app, (g) => g.useApp.getState().dirty)).toBe(false);
  // A second copy of the saved project, taken now: it has the same layer id and version and no UTM table.
  const dir2 = join(mkdtempSync(join(tmpdir(), "gis-e2e-u6b-")), "u6.gproj");
  cpSync(dir, dir2, { recursive: true });
  // 2. UTM: the engine builds the table and the map remembers it.
  await gis(app, (g, u: any) => g.useApp.getState().setMapCrs(u), UTM);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await expect.poll(() => draws(layer.id), { timeout: 30_000 }).toBe("ok");
  // 3. The copy: the engine loads a project whose layer has the same id and version but no UTM table.
  await gis(app, (g, d) => g.actions.openProject(d), dir2);
  expect(await gis(app, (g, id) => { const l = g.useApp.getState().layers[id]; return l ? `${l.id}@${l.version}` : null; }, layer.id)).toBe(`${layer.id}@${layer.version}`);
  // 4. UTM again.
  await gis(app, (g, u: any) => g.useApp.getState().setMapCrs(u), UTM);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await expect.poll(() => draws(layer.id), { timeout: 30_000 }).toBe("ok");
});

test("SAVE-1 Save right after adding a layer succeeds while the map is still preparing it", async ({ app }) => {
  // The map starts building a display table for the layer's CRS as soon as it is added; Save used to fail with "Cannot CHECKPOINT" when it overlapped.
  await addTestLayer(app, "polygons", 12, [-100, 34, -99.7, 34.3]);
  const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-save1-")), "s.gproj");
  await gis(app, (g, d) => g.useApp.setState({ projectDir: d }), dir);
  await gis(app, (g) => g.actions.saveProject());
  await expect.poll(() => gis(app, (g) => g.useApp.getState().dirty)).toBe(false);
});
