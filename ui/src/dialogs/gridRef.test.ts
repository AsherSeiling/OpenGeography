import { describe, expect, it } from "vitest";
import { exportGraph, gridField } from "./gridRef";

describe("Export Features with a grid reference", () => {
  it("is just input → export without one", () => {
    const g = exportGraph("L1", null, "/tmp/a.gpkg", "GPKG", null);
    expect(g.nodes.map((n) => n.tool)).toEqual(["input_layer", "export"]);
    expect(g.edges).toEqual([{ source: "in", target: "out", targetHandle: "in" }]);
  });
  it("puts Add USNG / MGRS Field between them, with the choices", () => {
    const g = exportGraph("L1", [3, 4], "/tmp/a.csv", "CSV", { style: "mgrs", precision: 3 });
    expect(g.nodes.map((n) => n.tool)).toEqual(["input_layer", "add_usng", "export"]);
    expect(g.nodes[0].params).toEqual({ layer: "L1", fids: [3, 4] });
    expect(g.nodes[1].params).toMatchObject({ field: "MGRS", style: "mgrs", precision: "3", datum: "wgs84" });
    expect(g.edges).toEqual([{ source: "in", target: "grid", targetHandle: "in" }, { source: "grid", target: "out", targetHandle: "in" }]);
  });
  it("names the column after the style", () => {
    expect(gridField("usng")).toBe("USNG");
  });
});
