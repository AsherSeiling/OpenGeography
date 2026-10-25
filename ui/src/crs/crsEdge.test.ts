import { describe, expect, it } from "vitest";
import { loadCatalog, searchCatalog, statePlaneFor, utmFor } from "./crs";

describe("UTM zone choice at the edges", () => {
  it.each([
    [-180, 10, "EPSG:32601"],
    [180, 10, "EPSG:32660"],
    [-0.0001, 10, "EPSG:32630"],
    [0, 10, "EPSG:32631"],
    [-97.5, -35, "EPSG:32714"],
    [-97.5, 0, "EPSG:32614"],
    [-97.5, 35, "EPSG:32614"],
  ])("lon %f lat %f → %s", (lon, lat, code) => expect(utmFor(lon, lat)).toBe(code));

  it("NAD83 gets the NAD83 zones in the US range, and WGS 84 elsewhere", () => {
    expect(utmFor(-97.5, 35, "NAD83")).toBe("EPSG:26914");
    expect(utmFor(-97.5, -35, "NAD83")).toBe("EPSG:32714"); // no southern NAD83 UTM
    expect(utmFor(100, 35, "NAD83")).toBe("EPSG:32647"); // zone 47: beyond NAD83's zones 1–23
  });

  it("out-of-range longitudes clamp rather than produce an invalid zone", () => {
    expect(utmFor(500, 10)).toBe("EPSG:32660");
    expect(utmFor(-500, 10)).toBe("EPSG:32601");
  });
});

describe("CRS search", () => {
  it("returns an empty list for nonsense and respects limit", async () => {
    const list = await loadCatalog();
    expect(searchCatalog(list, "zzzzzzzz-not-a-crs")).toEqual([]);
    expect(searchCatalog(list, "", { limit: 5 })).toHaveLength(5);
    expect(searchCatalog(list, "utm", { limit: 0 })).toEqual([]);
  });

  it("a bare EPSG number finds that code first, with or without the prefix and in any case", async () => {
    const list = await loadCatalog();
    expect(searchCatalog(list, "26914")[0].id).toBe("EPSG:26914");
    expect(searchCatalog(list, "epsg:26914")[0].id).toBe("EPSG:26914");
    expect(searchCatalog(list, "  EPSG:4326  ")[0].id).toBe("EPSG:4326");
  });

  it("the kind filter never mixes geographic and projected", async () => {
    const list = await loadCatalog();
    expect(searchCatalog(list, "nad83", { kind: "geographic", limit: 50 }).every((e) => !e.projected)).toBe(true);
    expect(searchCatalog(list, "nad83", { kind: "projected", limit: 50 }).every((e) => e.projected)).toBe(true);
  });

  it("location filtering keeps only systems covering the point, including across the antimeridian", async () => {
    const list = await loadCatalog();
    const hits = searchCatalog(list, "", { at: [-97.5, 35.4], limit: 400 });
    expect(hits.length).toBeGreaterThan(10);
    for (const h of hits) if (h.bounds) expect(-97.5 >= h.bounds[0] && -97.5 <= h.bounds[2] || h.bounds[0] > h.bounds[2]).toBe(true);
    const fiji = searchCatalog(list, "", { at: [179.9, -17.5], limit: 400 });
    const fijiWest = searchCatalog(list, "", { at: [-179.9, -17.5], limit: 400 });
    expect(fiji.length).toBeGreaterThan(0);
    expect(fijiWest.length).toBeGreaterThan(0);
  });

  it("entries with no area of use (null bounds) are kept, not crashed on, when filtering by location", () => {
    const e = { id: "ESRI:1", name: "Odd", projected: true, area: "", bounds: null };
    expect(searchCatalog([e], "", { at: [10, 10] })).toEqual([e]);
  });

  it("a State Plane lookup for a name that isn't a state is empty", async () => {
    const list = await loadCatalog();
    expect(statePlaneFor(list, "Atlantis")).toEqual([]);
    expect(statePlaneFor(list, "oklahoma").length).toBeGreaterThan(0);
  });
});
