// WP10/WP11: any number of named maps per project. Two maps side by side stay independent; the project keeps them; a version 1
// project opens as one map; a layout map frame follows the map it names.
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { addTestLayer, expect, gis, test } from "./fixtures";

const maps = (page: any) => gis(page, (g) => g.useApp.getState().maps.map((m: any) => ({ id: m.id, name: m.name, crs: m.crs?.id ?? null, basemap: m.basemap, order: m.order, visible: m.visible }))) as Promise<any[]>;
const act = (page: any, fn: string, arg?: unknown) =>
  page.evaluate(`(async () => { const m = await import("/src/maps/mapActions.ts"); const p = await import("/src/panes/panes.ts"); return (${fn})(m, p, window.__gis, ${JSON.stringify(arg ?? null)}); })()`);
const dockTab = (page: any, title: string) => page.locator(".dv-tab", { hasText: new RegExp(`^${title}$`) });
const center = (page: any, id: string) => gis(page, (g, i: string) => g.getMap(i).getView().getCenter(), id) as Promise<number[]>;

test.describe("maps", () => {
  test("two maps side by side stay independent: layers, coordinate system, basemap and view", async ({ app }) => {
    const a = await addTestLayer(app, "polygons", 5);
    const first = (await maps(app))[0];
    expect(first.order).toEqual([a.id]);
    await expect(app.getByTestId("active-map-crs")).toContainText(/WGS 84|4326/);

    // New Map: empty, takes the last map's coordinate system, and the status line says so.
    const second = await act(app, "(m,p,g) => m.newMap({ side: true })");
    await expect(app.locator(".map-wrap")).toHaveCount(2);
    await expect(app.locator(".dv-tab", { hasText: second.name })).toHaveCount(1);
    expect(await gis(app, (g) => g.useApp.getState().status)).toMatch(/uses the coordinate system of "Map"/);
    let m = await maps(app);
    expect(m[1].order).toEqual([]);
    expect(m[1].crs).toBe(m[0].crs);
    await expect(app.getByTestId("active-map-name")).toHaveText(second.name);

    // A layer added now goes to the active (second) map only.
    const b = await addTestLayer(app, "points", 5, [-95, 33, -92, 36]);
    m = await maps(app);
    expect(m[0].order).toEqual([a.id]);
    expect(m[1].order).toEqual([b.id]);

    // Each map has its own coordinate system and basemap.
    await gis(app, (g, id: string) => { g.useApp.getState().setMapCrs({ id: "EPSG:3857", name: "WGS 84 / Pseudo-Mercator" }, id); g.useApp.getState().setBasemap("usgsImagery", id); }, second.id);
    await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, second.id)).toBe("EPSG:3857");
    expect(await gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, first.id)).toBe("EPSG:4326");
    m = await maps(app);
    expect([m[0].crs, m[1].crs]).toEqual(["EPSG:4326", "EPSG:3857"]);
    expect([m[0].basemap, m[1].basemap]).toEqual(["none", "usgsImagery"]);

    // Panning one leaves the other where it was.
    const before = await center(app, first.id);
    await gis(app, (g, id: string) => { const v = g.getMap(id).getView(); const c = v.getCenter(); v.setCenter([c[0] + 5_000_000, c[1] + 1_000_000]); }, second.id);
    expect(await center(app, first.id)).toEqual(before);

    // Focus makes a map the active one; Map Layers follows.
    await app.locator(`.map-wrap[data-map-id="${first.id}"]`).click({ position: { x: 40, y: 40 } });
    expect(await gis(app, (g) => g.useApp.getState().activeMapId)).toBe(first.id);
    await expect(app.getByTestId("active-map-name")).toHaveText("Map");
    await app.locator(`.map-wrap[data-map-id="${second.id}"]`).click({ position: { x: 40, y: 40 } });
    expect(await gis(app, (g) => g.useApp.getState().activeMapId)).toBe(second.id);
  });

  test("layers: add to a map from the project, remove from a map, delete from the project, delete a map", async ({ app }) => {
    const a = await addTestLayer(app, "polygons", 3);
    const b = await addTestLayer(app, "points", 3);
    const second = await act(app, "(m) => m.newMap()");
    // + in Map Layers lists the project layers this map doesn't show yet.
    await app.getByRole("button", { name: "Add a project layer to this map" }).click();
    await app.getByRole("menuitem", { name: a.name }).click();
    expect((await maps(app))[1].order).toEqual([a.id]);
    expect((await maps(app))[0].order).toEqual([b.id, a.id]);
    // Removing it from this map keeps it in the project and on the other map.
    await gis(app, (g, id: string) => g.useApp.getState().removeFromMap([id]), a.id);
    expect((await maps(app))[1].order).toEqual([]);
    expect(await gis(app, (g, id: string) => !!g.useApp.getState().layers[id], a.id)).toBe(true);
    // Deleting the layer from the project removes it everywhere.
    await gis(app, (g, id: string) => g.useApp.getState().addToMap([id], undefined), b.id);
    await gis(app, (g, id: string) => g.actions.removeLayer(id), b.id);
    for (const mp of await maps(app)) expect(mp.order).not.toContain(b.id);
    // Deleting a map (Catalog › Project › Maps) never deletes layers.
    app.once("dialog", (d) => void d.accept());
    await app.locator(`[role="treeitem"][data-node="map:${second.id}"]`).hover();
    await app.getByRole("button", { name: `Delete ${second.name}` }).click();
    await expect.poll(async () => (await maps(app)).length).toBe(1);
    expect(await gis(app, (g, id: string) => !!g.useApp.getState().layers[id], a.id)).toBe(true);
    await expect(dockTab(app, second.name)).toHaveCount(0);
  });

  test("the Catalog lists the maps: open, rename, duplicate; Insert has New Map", async ({ app }) => {
    await expect(app.locator('[role="treeitem"][data-node="maps"]')).toBeVisible();
    await app.locator(".rtab", { hasText: /^Insert$/ }).click();
    await app.locator(".rbtn", { hasText: /^New Map$/ }).click();
    await expect.poll(async () => (await maps(app)).length).toBe(2);
    await app.locator(".rbtn", { hasText: /^Duplicate Map$/ }).click();
    await expect.poll(async () => (await maps(app)).length).toBe(3);
    const ms = await maps(app);
    expect(ms[2].name).toBe(`${ms[1].name} copy`);
    await act(app, "(m, p, g, a) => m.renameMap(a.id, 'Overview')", { id: ms[1].id });
    await expect(app.locator('[role="treeitem"][data-node="map:' + ms[1].id + '"]')).toContainText("Overview");
    await expect(dockTab(app, "Overview")).toHaveCount(1);
    // Closing a tab does not delete the map; double-clicking its Catalog row opens it again.
    await dockTab(app, "Overview").hover();
    await app.locator('.dv-tab:has-text("Overview") .dv-default-tab-action').click();
    await expect(dockTab(app, "Overview")).toHaveCount(0);
    expect((await maps(app)).length).toBe(3);
    await app.locator('[role="treeitem"][data-node="map:' + ms[1].id + '"]').dblclick();
    await expect(dockTab(app, "Overview")).toHaveCount(1);
  });

  test("save and reopen: both maps, their CRS, layers, visibility and the active map come back", async ({ app }) => {
    const a = await addTestLayer(app, "polygons", 4);
    const second = await act(app, "(m) => m.newMap({ side: true })");
    await gis(app, (g, id: string) => { g.useApp.getState().setMapCrs({ id: "EPSG:3857", name: "WGS 84 / Pseudo-Mercator" }, id); }, second.id);
    await gis(app, (g, ids: any) => { g.useApp.getState().addToMap([ids.a]); g.useApp.getState().setVisible(ids.a, false); }, { a: a.id });
    const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-maps-")), "maps.gproj");
    await gis(app, (g, d) => g.useApp.setState({ projectDir: d }), dir);
    await gis(app, (g) => g.actions.saveProject());
    await expect.poll(() => gis(app, (g) => g.useApp.getState().dirty)).toBe(false);
    const saved = await maps(app);
    // Wreck the state, then reopen.
    await gis(app, (g) => g.actions.newProject());
    expect((await maps(app)).length).toBe(1);
    await gis(app, (g, d) => g.actions.openProject(d), dir);
    expect(await maps(app)).toEqual(saved);
    expect(await gis(app, (g) => g.useApp.getState().activeMapId)).toBe(second.id);
    await expect(app.locator(".map-wrap")).toHaveCount(2); // the dock layout came back with both map panels
  });

  test("a version 1 project opens as one map named Map, with a null CRS left null", async ({ app }) => {
    const info = await addTestLayer(app, "points", 5);
    const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-v1-")), "v1.gproj");
    await expect.poll(() => gis(app, async (g, a: any) => {
      const ui = { version: 1, order: [a.id], visible: { [a.id]: false }, styles: {}, mapCrs: null, basemap: "none", camera: null, trees: {}, dock: null };
      try { await g.api.saveProject(a.dir, JSON.stringify(ui)); return true; } catch { return false; } // the engine may still be finishing the layer's write
    }, { id: info.id, dir })).toBe(true);
    await app.waitForTimeout(500);
    await gis(app, (g, d) => g.actions.openProject(d), dir);
    const ms = await maps(app);
    expect(ms).toHaveLength(1);
    expect(ms[0]).toMatchObject({ name: "Map", crs: null, basemap: "none", order: [info.id], visible: { [info.id]: false } });
    await expect(app.getByTestId("active-map-crs")).toContainText("No coordinate system yet");
    await expect(dockTab(app, "Map")).toHaveCount(1);
  });

  test("a project from a newer format is refused with a readable message and nothing changes", async ({ app, errors }) => {
    errors.expected = true; // the refusal is shown in the error pop-up (and logged)
    errors.known = [/newer version of GIS Studio/];
    const info = await addTestLayer(app, "points", 3);
    const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-v9-")), "v9.gproj");
    await expect.poll(() => gis(app, async (g, a: any) => { try { await g.api.saveProject(a.dir, JSON.stringify({ version: 9, maps: [] })); return true; } catch { return false; } }, { dir })).toBe(true);
    await gis(app, (g, d) => g.actions.openProject(d), dir);
    expect(await gis(app, (g) => g.useApp.getState().status)).toMatch(/newer version of GIS Studio/);
    expect(await gis(app, (g, id: string) => !!g.useApp.getState().layers[id], info.id)).toBe(true);
    await expect(app.getByRole("alertdialog")).toContainText(/newer version/);
  });

  test.describe("layout map frame", () => {
    test.use({ viewport: { width: 2560, height: 1300 } });
    test("follows the map it names and says so when that map is deleted", async ({ app, errors }) => {
      errors.expected = true; // the engine may report the export-time refusal
      const a = await addTestLayer(app, "polygons", 4);
      const second = await act(app, "(m) => m.newMap()");
      const b = await addTestLayer(app, "points", 4);
      // A layout with one frame that names the second map.
      const out = await gis(app, async (g) => {
        g.layoutActions.newLayout();
        g.layoutActions.insertMapFrame();
        const L = g.useLayouts.getState();
        const f = L.doc.elements.find((e: any) => e.kind === "map_frame");
        return { frame: f.id, layout: L.doc.id };
      });
      await gis(app, (g, a2: any) => g.useLayouts.getState().update(a2.frame, { layers: { map: a2.map } }), { frame: out.frame, map: second.id });
      const req = await gis(app, (g) => { const r = g.layoutActions.layoutRequest(g.useLayouts.getState().doc); return r.layout.elements.find((e: any) => e.kind === "map_frame").layers; });
      expect(req).toEqual([{ id: b.id, visible: true }]); // the second map's layers, not the active one's
      // The engine stores the layout with the map id in it.
      await gis(app, async () => (await import("/src/store/layout.ts")).flushLayoutSaves());
      const stored = await gis(app, async (g, id: string) => (await g.api.listLayouts()).find((l: any) => l.id === id)?.elements.find((e: any) => e.kind === "map_frame")?.layers, out.layout);
      expect(stored).toEqual({ map: second.id });
      // A frame that follows the first map draws that map's layers instead.
      const first = (await maps(app))[0];
      await gis(app, (g, a2: any) => g.useLayouts.getState().update(a2.frame, { layers: { map: a2.map } }), { frame: out.frame, map: first.id });
      expect(await gis(app, (g) => g.layoutActions.layoutRequest(g.useLayouts.getState().doc).layout.elements.find((e: any) => e.kind === "map_frame").layers)).toEqual([{ id: a.id, visible: true }]);
      // The "map" value still follows the active map; a fixed list is left alone.
      await gis(app, (g, a2: any) => g.useLayouts.getState().update(a2.frame, { layers: "map" }), { frame: out.frame });
      expect(await gis(app, (g) => g.layoutActions.layoutRequest(g.useLayouts.getState().doc).layout.elements.find((e: any) => e.kind === "map_frame").layers)).toBe("map");
      await gis(app, (g, a2: any) => g.useLayouts.getState().update(a2.frame, { layers: [{ id: a2.id, visible: true }] }), { frame: out.frame, id: a.id });
      expect(await gis(app, (g) => g.layoutActions.layoutRequest(g.useLayouts.getState().doc).layout.elements.find((e: any) => e.kind === "map_frame").layers)).toEqual([{ id: a.id, visible: true }]);
      // Delete the map the frame names: no layers, and Properties says why.
      await gis(app, (g, a2: any) => g.useLayouts.getState().update(a2.frame, { layers: { map: a2.map } }), { frame: out.frame, map: second.id });
      await act(app, "(m, p, g, a) => m.deleteMap(a.id, false)", { id: second.id });
      expect(await gis(app, (g) => g.layoutActions.layoutRequest(g.useLayouts.getState().doc).layout.elements.find((e: any) => e.kind === "map_frame").layers)).toEqual([]);
      await expect(app.getByRole("alert").filter({ hasText: /map this frame followed was deleted/ })).toBeVisible();
      await gis(app, (g) => g.layoutActions.exportLayout("png", 96));
      expect(await gis(app, (g) => g.useApp.getState().status)).toMatch(/follows a map that was deleted/);
    });
  });
});
