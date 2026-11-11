// UX11: fields and coordinate systems in the ribbon search.
import { describe, expect, it } from "vitest";
import type { CrsEntry } from "../crs/crs";
import { crsHits, fieldHits } from "./searchSources";

const layers = [
  { id: "A", name: "Parcels", temporary: false, fields: [{ name: "OWNER_NAME", type: "text" }, { name: "acres", type: "number" }] },
  { id: "B", name: "Roads", temporary: false, fields: [{ name: "name", type: "text" }, { name: "owner", type: "text" }] },
  { id: "T", name: "scratch", temporary: true, fields: [{ name: "owner", type: "text" }] },
  { id: "R", name: "DEM", temporary: false, fields: [] },
] as never[];

const cat: CrsEntry[] = [
  { id: "EPSG:26914", name: "NAD83 / UTM zone 14N", projected: true, area: "United States (USA) - 102°W to 96°W", bounds: null },
  { id: "EPSG:4326", name: "WGS 84", projected: false, area: "World", bounds: null },
  { id: "EPSG:32614", name: "WGS 84 / UTM zone 14N", projected: true, area: "Between 102°W and 96°W", bounds: null },
];

describe("fieldHits", () => {
  it("finds fields by name across layers, naming the layer, and skips temporary layers and rasters", () => {
    const h = fieldHits(layers, "owner");
    expect(h.map((x) => `${x.field}@${x.layerName}`).sort()).toEqual(["OWNER_NAME@Parcels", "owner@Roads"]);
  });
  it("matches whole-word starts, like the tool search, and finds a field through its layer's name too", () => {
    expect(fieldHits(layers, "acre")).toEqual([{ layerId: "A", layerName: "Parcels", field: "acres", type: "number" }]);
    expect(fieldHits(layers, "xyz")).toEqual([]);
    expect(fieldHits(layers, "parcels acres")).toHaveLength(1);
    expect(fieldHits(layers, "  ")).toEqual([]);
  });
  it("caps the count", () => {
    expect(fieldHits(layers, "name", 1)).toHaveLength(1);
  });
});

describe("crsHits", () => {
  it("finds coordinate systems by name, code or area, best first", () => {
    expect(crsHits(cat, "26914")[0].id).toBe("EPSG:26914");
    expect(crsHits(cat, "utm 14n").map((e) => e.id).sort()).toEqual(["EPSG:26914", "EPSG:32614"]);
  });
  it("stays quiet for a short query or before the list has loaded", () => {
    expect(crsHits(cat, "ut")).toEqual([]);
    expect(crsHits([], "wgs 84")).toEqual([]);
  });
});
