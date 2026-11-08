import { describe, expect, it } from "vitest";
import * as basemapModule from "./basemaps";
import { BASEMAP_DEFS, BASEMAP_IDS, BASEMAP_OPTIONS, basemapDef, basemapNotice, printRefusal, providerGridMaxLevel, readBasemap, type BasemapView } from "./basemaps";
import { printBasemap } from "../store/layout";
import { loadMaps } from "../store/maps";

const merc: BasemapView = { crsKey: "3857", crsName: "Web Mercator", world: false, center: [-98, 39], areaOfUse: [-180, -85, 180, 85], areaKnown: true };
const utm11: BasemapView = { crsKey: "abc", crsName: "NAD83 / UTM zone 11N", world: false, center: [-117, 36], areaOfUse: [-120, 0, -114, 84], areaKnown: true };

describe("basemap list", () => {
  it("is one list the menu, the map and the print rule all read", () => {
    expect(BASEMAP_OPTIONS.map(([id]) => id)).toEqual(BASEMAP_IDS);
    expect(BASEMAP_OPTIONS.at(-1)).toEqual(["none", "None"]);
    expect(new Set(BASEMAP_IDS).size).toBe(BASEMAP_IDS.length);
    for (const id of ["ofmLiberty", "ofmBright", "ofmPositron", "versatiles", "openTopoMap", "naturalEarth", "osm", "usgsTopo"]) expect(basemapDef(id)).not.toBeNull();
    expect(basemapDef("blueMarble")).toBeNull(); // not offered: GIBS doesn't mark the layer's licence
  });
  it("carries each provider's required credit wording", () => {
    const text = (id: string) => basemapDef(id)!.creditText;
    expect(text("ofmLiberty")).toBe("OpenFreeMap © OpenMapTiles Data from OpenStreetMap");
    expect(text("openTopoMap")).toBe("Kartendaten: © OpenStreetMap-Mitwirkende, SRTM | Kartendarstellung: © OpenTopoMap (CC-BY-SA)");
    expect(text("osm")).toBe("© OpenStreetMap contributors");
    expect(text("versatiles")).toContain("© OpenStreetMap contributors");
    // The HTML credit says the same words once tags are removed.
    for (const d of BASEMAP_DEFS) expect(d.credit.replace(/<[^>]+>/g, "").replace(/\s+/g, " ")).toBe(d.creditText.replace(/\s+/g, " "));
  });
  it("prints only USGS and Natural Earth", () => {
    expect(BASEMAP_DEFS.filter((d) => d.print).map((d) => d.id).sort()).toEqual(["naturalEarth", "usgsImagery", "usgsRelief", "usgsTopo"]);
    for (const d of BASEMAP_DEFS) expect(printBasemap(d.id)).toBe(d.print ? d.id : "none");
    expect(printBasemap("none")).toBe("none");
    expect(printRefusal("ofmLiberty")).toMatch(/OpenFreeMap Liberty can't be used in print layouts/);
    expect(printRefusal("naturalEarth")).toBeNull();
    expect(printRefusal("none")).toBeNull();
  });
});

describe("CRS rule", () => {
  it("has no refusal: every basemap draws in every CRS (UR-19), blank where it must be, with a notice", () => {
    expect((basemapModule as Record<string, unknown>).basemapRefusal).toBeUndefined();
    expect(printRefusal("osm")).toMatch(/can't be used in print layouts/); // a provider's terms are not a technical limit: print stays refused
  });
});

describe("notices where a raster basemap is blank or stretched", () => {
  it("says nothing in the middle of the CRS's own area and in Web Mercator", () => {
    for (const id of ["usgsTopo", "osm", "openTopoMap"]) expect(basemapNotice(basemapDef(id), merc)).toBeNull();
    expect(basemapNotice(basemapDef("usgsImagery"), utm11)).toBeNull();
    expect(basemapNotice(basemapDef("usgsImagery"), { ...utm11, center: [-122, 36] })).toBeNull(); // just past the zone edge is fine
  });
  it("says the USGS services only cover the United States, and not for the others", () => {
    const paris = { ...merc, center: [2.35, 48.85] as [number, number] };
    expect(basemapNotice(basemapDef("usgsTopo"), paris)).toMatch(/only has tiles for the United States/);
    expect(basemapNotice(basemapDef("usgsRelief"), { ...merc, center: [-155.5, 19.5] })).toBeNull(); // Hawaii
    expect(basemapNotice(basemapDef("usgsRelief"), { ...merc, center: [-150, 64] })).toBeNull(); // Alaska
    expect(basemapNotice(basemapDef("osm"), paris)).toBeNull();
  });
  it("says Web Mercator ends at 85° and points to Natural Earth", () => {
    const pole = { ...utm11, areaOfUse: [-180, -90, 180, -60] as [number, number, number, number], center: [0, -89] as [number, number] };
    expect(basemapNotice(basemapDef("osm"), pole)).toMatch(/no tiles beyond 85° latitude.*Natural Earth/);
    expect(basemapNotice(basemapDef("naturalEarth"), pole)).toBeNull();
  });
  it("warns, without refusing, far outside the area where the CRS is meant to be used", () => {
    expect(basemapNotice(basemapDef("osm"), { ...utm11, center: [-98.5, 39.5] })).toMatch(/far outside the area where NAD83 \/ UTM zone 11N is meant/);
    expect(basemapNotice(basemapDef("osm"), { ...utm11, center: [-129.9, 36] })).toBeNull();
    expect(basemapNotice(basemapDef("osm"), { ...utm11, areaKnown: false, center: [10, 10] })).toBeNull();
    expect(basemapNotice(basemapDef("osm"), { ...utm11, areaOfUse: [-175, -80, 175, 80], center: [179.9, 0] })).toBeNull();
    const fiji = { ...utm11, areaOfUse: [172, -30, -170, -10] as [number, number, number, number], center: [179, -17] as [number, number] };
    expect(basemapNotice(basemapDef("osm"), fiji)).toBeNull();
    expect(basemapNotice(basemapDef("osm"), { ...fiji, center: [100, -17] })).not.toBeNull();
  });
});

describe("how fine a grid level to ask the engine for", () => {
  it("stops where the grid is more than twice as fine as the provider's own tiles", () => {
    const res = Array.from({ length: 23 }, (_, i) => 156543.03392804097 / 2 ** i); // Web Mercator's own, in metres
    expect(providerGridMaxLevel(res, 1, 16)).toBe(17); // half the native size of level 16
    expect(providerGridMaxLevel(res, 1, 19)).toBe(20);
    // A grid in feet: 0.3048 m per unit, so each unit is smaller than a metre and the same ground resolution is reached at a coarser number.
    const feet = res.map((r) => r / 0.3048);
    expect(providerGridMaxLevel(feet, 0.3048, 16)).toBe(17);
    expect(providerGridMaxLevel([1000], 1, 16)).toBe(0);
  });
});

describe("saved basemaps", () => {
  it("keeps every known value and turns an unknown one into none with a message", () => {
    for (const id of BASEMAP_IDS) expect(readBasemap(id)).toEqual({ id, message: null });
    expect(readBasemap(undefined)).toEqual({ id: "usgsTopo", message: null });
    const r = readBasemap("bingAerial");
    expect(r.id).toBe("none");
    expect(r.message).toContain('"bingAerial"');
    expect(readBasemap(42).id).toBe("none");
  });
  it("loadMaps reads a project naming an unknown basemap without an error", () => {
    const v2 = loadMaps({ version: 2, maps: [{ id: "a", name: "A", crs: null, basemap: "ofmLiberty", order: [], visible: {}, camera: null }, { id: "b", name: "B", crs: null, basemap: "blueMarble2", order: [], visible: {}, camera: null }], activeMapId: "a" } as never, []);
    expect(v2.ok && v2.maps.map((m) => m.basemap)).toEqual(["ofmLiberty", "none"]);
    expect(v2.ok && v2.notes).toHaveLength(1);
    const v1 = loadMaps({ version: 1, basemap: "versatiles" } as never, []);
    expect(v1.ok && v1.maps[0].basemap).toBe("versatiles");
    expect(v1.ok && v1.notes).toEqual([]);
  });
});
