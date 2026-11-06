import { describe, expect, it } from "vitest";
import { exportParams, formatById, formatsFor, refusal } from "./exportFormats";
import { exportGraph } from "./gridRef";

describe("UR-15 export formats and choices", () => {
  it("lists the new vector formats and the table formats", () => {
    expect(formatsFor("Polygon").map((f) => f.id)).toEqual(["GPKG", "ESRI Shapefile", "GeoJSON", "FlatGeobuf", "PARQUET", "CSV", "KML", "GPX", "DXF"]);
    expect(formatsFor("Table").map((f) => f.ext)).toEqual(["csv", "tsv", "xlsx", "ods", "parquet", "json", "dbf", "gpkg"]);
    expect(formatById("Point", "nope").id).toBe("GPKG");
  });
  it("refuses GPX for polygons with the reason, and nothing else", () => {
    expect(refusal("GPX", "Polygon")).toContain("points and lines only");
    expect(refusal("GPX", "Point")).toBeNull();
    expect(refusal("KML", "Polygon")).toBeNull();
  });
  it("passes only the choices that were made, and into the export node", () => {
    expect(exportParams({})).toEqual({});
    expect(exportParams({ crs: "EPSG:4326", fields: ["a", "b"], clipExtent: "1,2,3,4", clipLayer: "L2" })).toEqual({ crs: "EPSG:4326", fields: ["a", "b"], clip_extent: "1,2,3,4", clip_layer: "L2" });
    expect(exportParams({ fields: null, crs: "" })).toEqual({});
    const g = exportGraph("L1", null, "/tmp/a.kml", "KML", null, exportParams({ crs: "EPSG:4326" }));
    expect(g.nodes.find((n) => n.id === "out")!.params).toEqual({ path: "/tmp/a.kml", format: "KML", crs: "EPSG:4326" });
  });
});
