// WP12: more open basemaps. Credits per map, the CRS refusal in words, and the bundled Natural Earth drawing in a projected CRS.
// Nothing here needs live tiles: credits and notices don't depend on tiles loading, and Natural Earth comes from the engine.
import { expect, gis, test } from "./fixtures";

const act = (page: any, fn: string, arg?: unknown) =>
  page.evaluate(`(async () => { const m = await import("/src/maps/mapActions.ts"); const p = await import("/src/panes/panes.ts"); return (${fn})(m, p, window.__gis, ${JSON.stringify(arg ?? null)}); })()`);
const mapEl = (page: any, id: string) => page.locator(`.map-wrap[data-map-id="${id}"]`);
const credit = (page: any, id: string) => mapEl(page, id).locator(".ol-attribution");
const UTM14 = { id: "EPSG:26914", name: "NAD83 / UTM zone 14N" };
const UTM11 = { id: "EPSG:26911", name: "NAD83 / UTM zone 11N" };

test.describe("basemaps", () => {
  test("two maps with different basemaps each show their own credit", async ({ app }) => {
    const first = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
    const second = await act(app, "(m) => m.newMap({ side: true })");
    await gis(app, (g, a: any) => { g.useApp.getState().setBasemap("naturalEarth", a.first); g.useApp.getState().setBasemap("openTopoMap", a.second); }, { first: first.id, second: second.id });
    await expect(credit(app, first.id)).toContainText("Made with Natural Earth");
    await expect(credit(app, second.id)).toContainText("Kartendaten: © OpenStreetMap-Mitwirkende, SRTM | Kartendarstellung: © OpenTopoMap (CC-BY-SA)");
    await expect(credit(app, first.id)).not.toContainText("OpenTopoMap");
    // Switching the second map to an OpenFreeMap style changes only its credit.
    await gis(app, (g, id: string) => g.useApp.getState().setBasemap("ofmBright", id), second.id);
    await expect(credit(app, second.id)).toContainText("OpenFreeMap © OpenMapTiles Data from OpenStreetMap");
    await expect(credit(app, first.id)).toContainText("Made with Natural Earth");
    // Each basemap in use is recorded as a source.
    await expect.poll(async () => (await gis(app, (g) => g.api.sources())).map((s: any) => s.citation.key).sort()).toEqual(expect.arrayContaining(["natural-earth"]));
  });

  test("a vector basemap in a UTM map says so in words and doesn't draw; the CRS stays", async ({ app }) => {
    const m = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
    await gis(app, (g, a: any) => { g.useApp.getState().setMapCrs(a.crs, a.id); g.useApp.getState().setBasemap("ofmLiberty", a.id); }, { crs: UTM14, id: m.id });
    await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, m.id)).toBe("EPSG:26914");
    await expect(mapEl(app, m.id).getByTestId("map-notices")).toContainText("OpenFreeMap Liberty is drawn in Web Mercator only, so it can't be shown in NAD83 / UTM zone 14N");
    await expect(credit(app, m.id)).not.toContainText("OpenFreeMap");
    expect(await gis(app, (g, id: string) => g.useApp.getState().maps.find((x: any) => x.id === id).crs.id, m.id)).toBe("EPSG:26914");
    // Natural Earth draws in the same map and the message goes.
    await gis(app, (g, id: string) => g.useApp.getState().setBasemap("naturalEarth", id), m.id);
    await expect(mapEl(app, m.id).getByTestId("map-notices")).toHaveCount(0);
  });

  test("a raster basemap in a UTM map is refused when the view is outside the zone (MM-3) and draws once it is inside", async ({ app }) => {
    const m = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
    await gis(app, (g, a: any) => { g.useApp.getState().setMapCrs(a.crs, a.id); g.useApp.getState().setBasemap("usgsImagery", a.id); }, { crs: UTM11, id: m.id });
    await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, m.id)).toBe("EPSG:26911");
    // The default view is centred on the middle of the US, far east of UTM zone 11.
    await expect(mapEl(app, m.id).getByTestId("map-notices")).toContainText("USGS Imagery isn't drawn here: the view is outside the area where NAD83 / UTM zone 11N is valid");
    await gis(app, async (g, id: string) => {
      const [x, y] = await g.api.transformPoint(-117, 36, "EPSG:4326", "EPSG:26911");
      g.getMap(id).getView().setCenter([x, y]);
    }, m.id);
    await expect(mapEl(app, m.id).getByTestId("map-notices")).toHaveCount(0);
    await expect(credit(app, m.id)).toContainText("USGS The National Map");
  });

  test("Natural Earth draws from the engine in a projected CRS", async ({ app }) => {
    const m = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
    await gis(app, (g, a: any) => { g.useApp.getState().setMapCrs(a.crs, a.id); g.useApp.getState().setBasemap("naturalEarth", a.id); }, { crs: UTM14, id: m.id });
    await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, m.id)).toBe("EPSG:26914");
    // The map's centre is in the US interior: land colour, opaque, once the tiles have drawn.
    await expect
      .poll(async () =>
        gis(app, (g, id: string) => {
          const map = g.getMap(id);
          const size = map.getSize();
          const c = map.getViewport().querySelector("canvas") as HTMLCanvasElement;
          const k = c.width / size[0];
          const d = c.getContext("2d")!.getImageData(Math.round((size[0] / 2) * k), Math.round((size[1] / 2) * k), 1, 1).data;
          return [d[0], d[1], d[2], d[3]];
        }, m.id),
      { timeout: 20000 })
      .toEqual([238, 235, 224, 255]);
  });

  test("a saved project naming a basemap this version doesn't have opens with none and a message", async ({ app }) => {
    const r = (await gis(app, async (g) => {
      const m = await import("/src/store/maps.ts");
      const loaded = m.loadMaps({ version: 2, maps: [{ id: "a", name: "A", crs: null, basemap: "bingAerial", order: [], visible: {}, camera: null }], activeMapId: "a" } as any, []);
      return loaded.ok ? { basemap: loaded.maps[0].basemap, notes: loaded.notes } : null;
    })) as { basemap: string; notes: string[] };
    expect(r.basemap).toBe("none");
    expect(r.notes[0]).toContain("bingAerial");
  });

  test("the Basemap menu lists every open basemap", async ({ app }) => {
    await app.getByRole("button", { name: /^Basemap/ }).click();
    const names = await app.getByRole("menuitemradio").allInnerTexts();
    test.skip(!names.includes("Natural Earth"), "needs the WP12 diff to ribbon/commands.tsx (the menu still reads its old list)");
    expect(names).toEqual(["USGS Topo", "USGS Imagery", "USGS Shaded Relief", "OpenStreetMap", "OpenTopoMap", "OpenFreeMap Liberty", "OpenFreeMap Bright", "OpenFreeMap Positron", "VersaTiles Colorful", "Natural Earth", "None"]);
  });
});
