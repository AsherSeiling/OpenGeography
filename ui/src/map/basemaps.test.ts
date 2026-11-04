import { describe, expect, it } from "vitest";
import { BASEMAP_DEFS, BASEMAP_IDS, BASEMAP_OPTIONS, basemapDef, basemapRefusal, printRefusal, readBasemap, type BasemapView } from "./basemaps";
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

describe("CRS refusal", () => {
  it("lets every kind draw in Web Mercator", () => {
    for (const d of BASEMAP_DEFS) expect(basemapRefusal(d, merc)).toBeNull();
  });
  it("refuses vector tiles outside Web Mercator, in words", () => {
    for (const id of ["ofmLiberty", "ofmBright", "ofmPositron", "versatiles"]) {
      expect(basemapRefusal(basemapDef(id), utm11)).toMatch(/Web Mercator only.*NAD83 \/ UTM zone 11N/);
    }
  });
  it("draws raster tiles inside the CRS's area of use and refuses far outside it (MM-3)", () => {
    expect(basemapRefusal(basemapDef("usgsImagery"), utm11)).toBeNull();
    const away = basemapRefusal(basemapDef("usgsImagery"), { ...utm11, center: [-98.5, 39.5] });
    expect(away).toMatch(/outside the area where NAD83 \/ UTM zone 11N is valid/);
    expect(basemapRefusal(basemapDef("openTopoMap"), { ...utm11, center: [-98.5, 39.5] })).toBe(away!.replace("USGS Imagery", "OpenTopoMap"));
    // Antimeridian-crossing areas of use work.
    const fiji = { ...utm11, areaOfUse: [172, -30, -170, -10] as [number, number, number, number], center: [179, -17] as [number, number] };
    expect(basemapRefusal(basemapDef("osm"), fiji)).toBeNull();
    expect(basemapRefusal(basemapDef("osm"), { ...fiji, center: [-175, -17] })).toBeNull();
    expect(basemapRefusal(basemapDef("osm"), { ...fiji, center: [100, -17] })).not.toBeNull();
  });
  it("refuses Web Mercator tiles at the poles and never guesses when the area is unknown", () => {
    expect(basemapRefusal(basemapDef("usgsTopo"), { ...utm11, areaOfUse: [-180, -90, 180, -60], center: [0, -89] })).toMatch(/doesn't cover the poles/);
    expect(basemapRefusal(basemapDef("usgsTopo"), { ...utm11, areaKnown: false, center: [10, 10] })).toBeNull();
    expect(basemapRefusal(basemapDef("usgsTopo"), { ...utm11, center: null })).toBeNull();
  });
  it("lets Natural Earth draw in every CRS, even at the pole and outside the zone", () => {
    expect(basemapRefusal(basemapDef("naturalEarth"), { ...utm11, center: [-98.5, 39.5] })).toBeNull();
    expect(basemapRefusal(basemapDef("naturalEarth"), { ...utm11, center: [0, -90] })).toBeNull();
    expect(basemapRefusal(basemapDef("none"), utm11)).toBeNull();
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
