// Maps of a project (WP10): the v1 → v2 project migration, the newer-version refusal, CRS never guessed, and layer membership.
import { beforeEach, describe, expect, it } from "vitest";
import type { LayerInfo } from "../ipc/api";
import { basemapsInUse, emptyMap, loadMaps, newMapFrom, PROJECT_VERSION, serializeMaps } from "./maps";
import { useApp } from "./store";

const info = (id: string, over: Partial<LayerInfo> = {}): LayerInfo =>
  ({ id, name: id, geomType: "Polygon", crs: "EPSG:26914", crsName: "NAD83 / UTM 14N", version: 1, fields: [], ...over }) as never;
const UTM = { id: "EPSG:26914", name: "NAD83 / UTM 14N" };

function reset() {
  useApp.setState({ layers: {}, results: {}, showOriginal: {}, styles: {}, selection: {}, activeLayer: null, dirty: false, maps: [emptyMap()], activeMapId: "map" });
}
beforeEach(reset);

describe("project format", () => {
  it("opens a version 1 file as one map named Map with order, visibility, CRS, basemap and view unchanged", () => {
    const camera = { center: [-98, 39] as [number, number], scale: 250000 };
    const r = loadMaps({ version: 1, order: ["b", "a"], visible: { a: false, b: true }, mapCrs: UTM, basemap: "usgsImagery", camera }, ["a", "b"]);
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.maps).toHaveLength(1);
    expect(r.maps[0]).toMatchObject({ id: "map", name: "Map", crs: UTM, basemap: "usgsImagery", order: ["b", "a"], visible: { a: false, b: true }, camera });
    expect(r.activeMapId).toBe("map");
  });

  it("keeps a version 1 mapCrs of null as null (no CRS is guessed), even with layers", () => {
    const r = loadMaps({ version: 1, order: ["a"], mapCrs: null }, ["a"]);
    expect(r.ok && r.maps[0].crs).toBeNull();
  });

  it("a file with no version, or an empty one, is read as version 1", () => {
    for (const ui of [{}, { order: ["a"] }]) {
      const r = loadMaps(ui, ["a"]);
      expect(r.ok && r.maps).toHaveLength(1);
      expect(r.ok && r.maps[0].crs).toBeNull();
      expect(r.ok && r.maps[0].basemap).toBe("usgsTopo");
    }
  });

  it("version 1: layers missing from the saved order go on top and ids of removed layers are dropped", () => {
    const r = loadMaps({ version: 1, order: ["gone", "a"] }, ["a", "b"]);
    expect(r.ok && r.maps[0].order).toEqual(["b", "a"]);
    expect(r.ok && r.maps[0].visible).toEqual({ a: true, b: true });
  });

  it("round-trips a version 2 file exactly, including two maps, a layer shown on one only and a null CRS", () => {
    const saved = {
      version: PROJECT_VERSION,
      activeMapId: "map-2",
      maps: [
        { id: "map", name: "Map", crs: UTM, basemap: "usgsTopo", order: ["b", "a"], visible: { a: true, b: false }, camera: { center: [1, 2], scale: 24000 } },
        { id: "map-2", name: "Detail", crs: null, basemap: "none", order: ["a"], visible: { a: true }, camera: null },
      ],
    };
    const r = loadMaps(saved, ["a", "b"]);
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(serializeMaps(r.maps)).toEqual(saved.maps);
    expect(r.activeMapId).toBe("map-2");
    expect(r.maps[1].order).toEqual(["a"]); // b is not forced onto the second map
  });

  it("version 2: ids of layers no longer in the project are dropped, an empty maps list gets one map, a bad active id falls back", () => {
    const r = loadMaps({ version: 2, activeMapId: "nope", maps: [{ id: "x", name: "X", crs: null, basemap: "osm", order: ["a", "gone"], visible: { gone: true } }] }, ["a"]);
    expect(r.ok && r.maps[0].order).toEqual(["a"]);
    expect(r.ok && r.activeMapId).toBe("x");
    const e = loadMaps({ version: 2, maps: [] }, ["a"]);
    expect(e.ok && e.maps).toHaveLength(1);
  });

  it("refuses a file from a newer format with a readable message instead of reading it", () => {
    const r = loadMaps({ version: PROJECT_VERSION + 1, maps: [] }, []);
    expect(r.ok).toBe(false);
    expect(!r.ok && r.error).toMatch(/newer version/);
    expect(!r.ok && r.error).toMatch(/Nothing was changed/);
  });

  it("refuses an unreadable version", () => {
    for (const version of ["2", 0, -1, 1.5, null]) expect(loadMaps({ version }, []).ok).toBe(false);
  });

  it("does not save the session-only world view", () => {
    const m = emptyMap({ worldView: true });
    expect(serializeMaps([m])[0]).not.toHaveProperty("worldView");
  });
});

describe("new maps", () => {
  it("takes the last map's CRS and basemap and names where from", () => {
    const a = emptyMap({ crs: UTM, basemap: "osm" });
    const { map, inheritedFrom } = newMapFrom([a]);
    expect(map.crs).toEqual(UTM);
    expect(map.crs).not.toBe(a.crs);
    expect(map.basemap).toBe("osm");
    expect(map.order).toEqual([]);
    expect(map.id).not.toBe(a.id);
    expect(inheritedFrom?.id).toBe(a.id);
    expect(map.name).toBe("Map 2");
  });
  it("stays without a CRS when the last map has none", () => {
    const { map, inheritedFrom } = newMapFrom([emptyMap()]);
    expect(map.crs).toBeNull();
    expect(inheritedFrom).toBeNull();
  });
  it("lists each basemap once", () => {
    expect(basemapsInUse([emptyMap({ basemap: "osm" }), emptyMap({ id: "b", basemap: "osm" }), emptyMap({ id: "c", basemap: "none" })])).toEqual(["osm", "none"]);
  });
});

describe("maps in the store", () => {
  it("keeps the old single-value fields equal to the active map's, whichever side is written", () => {
    useApp.getState().addLayers([info("a")]);
    expect(useApp.getState().mapCrs).toEqual(UTM);
    useApp.setState({ mapCrs: { id: "EPSG:4326", name: "WGS 84" } }); // an old-style write
    expect(useApp.getState().maps[0].crs?.id).toBe("EPSG:4326");
    useApp.getState().setBasemap("none");
    expect(useApp.getState().maps[0].basemap).toBe("none");
    expect(useApp.getState().basemap).toBe("none");
  });

  it("a layer added later goes to the active map only and leaves the others alone", () => {
    useApp.getState().addLayers([info("a")]);
    const { map } = useApp.getState().addMap(); // active is now the new map; it copied the CRS
    expect(useApp.getState().activeMapId).toBe(map.id);
    useApp.getState().addLayers([info("b", { crs: "EPSG:4326", crsName: "WGS 84" })]);
    const [first, second] = useApp.getState().maps;
    expect(first.order).toEqual(["a"]);
    expect(first.crs).toEqual(UTM);
    expect(second.order).toEqual(["b"]);
    expect(second.crs).toEqual(UTM); // already set: a later layer never changes it
    expect(useApp.getState().order).toEqual(["b"]);
  });

  it("a new map with no layers and no CRS stays CRS-less until a real source sets it", () => {
    const { map } = useApp.getState().addMap();
    expect(useApp.getState().mapCrs).toBeNull();
    useApp.getState().addLayers([info("t", { geomType: "Table", crs: null })]);
    expect(useApp.getState().maps.find((m) => m.id === map.id)?.crs).toBeNull();
    useApp.getState().addLayers([info("p", { crs: "EPSG:3857", crsName: "Web Mercator" })]);
    expect(useApp.getState().mapCrs?.id).toBe("EPSG:3857");
  });

  it("visibility and order are per map", () => {
    useApp.getState().addLayers([info("a"), info("b")]);
    useApp.getState().addMap();
    useApp.getState().addToMap(["a", "b"]);
    useApp.getState().setVisible("a", false);
    useApp.getState().moveLayer("b", 5);
    const [first, second] = useApp.getState().maps;
    expect(first.visible).toEqual({ a: true, b: true });
    expect(first.order).toEqual(["b", "a"]);
    expect(second.visible.a).toBe(false);
    expect(second.order).toEqual(["a", "b"]);
  });

  it("removing a layer from a map keeps it in the project; deleting it removes it from every map", () => {
    useApp.getState().addLayers([info("a"), info("b")]);
    useApp.getState().addMap();
    useApp.getState().addToMap(["a"]);
    useApp.getState().removeFromMap(["a"]);
    expect(useApp.getState().layers.a).toBeDefined();
    expect(useApp.getState().maps[1].order).toEqual([]);
    expect(useApp.getState().maps[0].order).toEqual(["b", "a"]);
    useApp.getState().removeLayer("a");
    for (const m of useApp.getState().maps) {
      expect(m.order).not.toContain("a");
      expect(m.visible).not.toHaveProperty("a");
    }
  });

  it("adding a project layer to a map with no CRS gives it that layer's CRS; one that has a CRS keeps it", () => {
    useApp.getState().addLayers([info("a", { crs: "EPSG:4326", crsName: "WGS 84" })]);
    const { map } = useApp.getState().addMap();
    useApp.getState().setMapCrs(null, map.id);
    useApp.getState().addToMap(["a"]);
    expect(useApp.getState().mapCrs?.id).toBe("EPSG:4326");
    useApp.getState().addLayers([info("b")]);
    useApp.getState().addToMap(["b"], "map");
    expect(useApp.getState().maps[0].crs?.id).toBe("EPSG:4326");
  });

  it("deleting or closing a map never deletes layers, and the last map cannot be deleted", () => {
    useApp.getState().addLayers([info("a")]);
    const { map } = useApp.getState().addMap();
    expect(useApp.getState().deleteMap(map.id)).toBe(true);
    expect(useApp.getState().layers.a).toBeDefined();
    expect(useApp.getState().activeMapId).toBe("map");
    expect(useApp.getState().deleteMap("map")).toBe(false);
    expect(useApp.getState().maps).toHaveLength(1);
  });

  it("duplicates a map with its own copies of order, visibility and CRS", () => {
    useApp.getState().addLayers([info("a")]);
    const copy = useApp.getState().duplicateMap("map")!;
    useApp.getState().setVisible("a", false);
    expect(copy.name).toBe("Map copy");
    expect(useApp.getState().maps[0].visible.a).toBe(true);
    expect(useApp.getState().maps[1].crs).toEqual(UTM);
  });

  it("renames to a free name and ignores a blank one", () => {
    const { map } = useApp.getState().addMap();
    useApp.getState().renameMap(map.id, "  ");
    expect(useApp.getState().maps[1].name).toBe("Map 2");
    useApp.getState().renameMap(map.id, "Map");
    expect(useApp.getState().maps[1].name).toBe("Map 2");
    useApp.getState().renameMap(map.id, "Overview");
    expect(useApp.getState().maps[1].name).toBe("Overview");
  });
});
