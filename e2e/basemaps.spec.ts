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

  test("a vector basemap draws in a UTM map (UR-19): drawn off-screen in Web Mercator, warped by the engine, and a known polygon is where it belongs", async ({ app }) => {
    // A synthetic style stands in for the provider (no network): a white background and a black square 0.1° wide around (-98, 38.5).
    const style = {
      version: 8,
      sources: { box: { type: "geojson", data: { type: "Feature", properties: {}, geometry: { type: "Polygon", coordinates: [[[-98.05, 38.45], [-97.95, 38.45], [-97.95, 38.55], [-98.05, 38.55], [-98.05, 38.45]]] } } } },
      layers: [{ id: "bg", type: "background", paint: { "background-color": "#ffffff" } }, { id: "box", type: "fill", source: "box", paint: { "fill-color": "#000000", "fill-opacity": 1 } }],
    };
    await app.route(/styles\/liberty/, (route: any) => route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(style) }));
    const m = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
    await gis(app, (g, a: any) => { g.useApp.getState().setMapCrs(a.crs, a.id); g.useApp.getState().setBasemap("ofmLiberty", a.id); }, { crs: UTM14, id: m.id });
    await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, m.id)).toBe("EPSG:26914");
    // No refusal any more; the CRS stays; the provider's credit is on the map.
    await expect(mapEl(app, m.id).getByText(/Web Mercator only/)).toHaveCount(0);
    expect(await gis(app, (g, id: string) => g.useApp.getState().maps.find((x: any) => x.id === id).crs.id, m.id)).toBe("EPSG:26914");
    // Centre the view on the square's centre at 100 m per pixel: the square is about 90 px wide, so its centre is dark and 150 px away is white.
    await gis(app, async (g, id: string) => {
      const [x, y] = await g.api.transformPoint(-98, 38.5, "EPSG:4326", "EPSG:26914");
      const v = g.getMap(id).getView();
      v.setCenter([x, y]);
      v.setResolution(100);
    }, m.id);
    const px = (dx: number) => gis(app, (g, a: any) => {
      const map = g.getMap(a.id);
      const size = map.getSize();
      const k = (map.getViewport().querySelector("canvas") as HTMLCanvasElement).width / size[0];
      // The basemap image layer is one of the viewport's canvases; read the composite of them all at the point.
      const out = document.createElement("canvas");
      out.width = Math.round(size[0] * k); out.height = Math.round(size[1] * k);
      const ctx = out.getContext("2d")!;
      map.getViewport().querySelectorAll("canvas").forEach((c: HTMLCanvasElement) => { if (c.width) ctx.drawImage(c, 0, 0, out.width, out.height); });
      const d = ctx.getImageData(Math.round((size[0] / 2 + a.dx) * k), Math.round((size[1] / 2) * k), 1, 1).data;
      return [d[0], d[1], d[2], d[3]];
    }, { id: m.id, dx });
    // (Poll for all three at once: the first picture may be an earlier view's until the one for this view is warped.)
    await expect.poll(async () => [(await px(0))[0], (await px(70))[0] > 240, (await px(150))[0] > 240], { timeout: 40_000 }).toEqual([0, true, true]);
    expect((await px(0))[3]).toBe(255);
    // The edge is where 0.05° of longitude at 38.5° N is: about 41 px at 100 m/px. 30 px inside is dark, 55 px out is white.
    expect((await px(30))[0]).toBe(0);
    expect((await px(55))[0]).toBeGreaterThan(240);
    expect((await px(-30))[0]).toBe(0);
    expect((await px(-55))[0]).toBeGreaterThan(240);
    expect(await credit(app, m.id).textContent()).toContain("OpenFreeMap");
  });

  test("a raster basemap in a UTM map is no longer refused far outside the zone (MM-3, UR-19): the engine warps it, the map says it is far outside", async ({ app }) => {
    // The engine would fetch the provider's tiles; here the tile route answers with a transparent PNG and records what was asked.
    const asked: string[] = [];
    const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==", "base64");
    await app.route(/\/tiles\/__bm_usgsImagery\//, (route: any) => { asked.push(route.request().url()); return route.fulfill({ status: 200, contentType: "image/png", body: png }); });
    const m = (await gis(app, (g) => g.useApp.getState().maps[0])) as { id: string };
    await gis(app, (g, a: any) => { g.useApp.getState().setMapCrs(a.crs, a.id); g.useApp.getState().setBasemap("usgsImagery", a.id); }, { crs: UTM11, id: m.id });
    await expect.poll(() => gis(app, (g, id: string) => g.mapController.getActiveCrs(id)?.id, m.id)).toBe("EPSG:26911");
    // The default view is centred on the middle of the US, far east of UTM zone 11: said in a sentence, not refused.
    await expect(mapEl(app, m.id).getByTestId("map-notices")).toContainText("far outside the area where NAD83 / UTM zone 11N is meant to be used");
    await expect(mapEl(app, m.id).getByTestId("map-notices")).not.toContainText("isn't drawn here");
    await expect(credit(app, m.id)).toContainText("USGS The National Map");
    // The tiles come from the engine, on this map's own grid (the CRS key is in the path), not from the provider's server.
    await expect.poll(() => asked.length, { timeout: 15_000 }).toBeGreaterThan(0);
    expect(asked[0]).toMatch(/\/tiles\/__bm_usgsImagery\/[^/]+\/\d+\/\d+\/\d+\.png/);
    expect(asked.every((u) => !u.includes("nationalmap.gov"))).toBe(true);
    // Inside the zone the notice goes and the tiles keep coming.
    await gis(app, async (g, id: string) => {
      const [x, y] = await g.api.transformPoint(-117, 36, "EPSG:4326", "EPSG:26911");
      g.getMap(id).getView().setCenter([x, y]);
    }, m.id);
    await expect(mapEl(app, m.id).getByTestId("map-notices")).toHaveCount(0);
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
    expect(names).toEqual(["USGS Topo", "USGS Imagery", "USGS Shaded Relief", "OpenStreetMap", "OpenTopoMap", "OpenFreeMap Liberty", "OpenFreeMap Bright", "OpenFreeMap Positron", "VersaTiles Colorful", "Natural Earth", "None"]);
  });
});
