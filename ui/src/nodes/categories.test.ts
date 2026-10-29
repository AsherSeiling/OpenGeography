import { describe, expect, it } from "vitest";
import { buildTree, compareCategories, splitCategory } from "./categories";

describe("tool categories", () => {
  it("splits group and sub-group", () => {
    expect(splitCategory("Vector › Overlay")).toEqual(["Vector", "Overlay"]);
    expect(splitCategory("Tables")).toEqual(["Tables", null]);
  });
  it("orders groups, then sub-groups; Output last, unknown groups before it", () => {
    const cats = ["Output", "Raster › Terrain", "Zeta", "Vector › Overlay", "Vector › Geometry", "Hydrology"];
    expect(cats.sort(compareCategories)).toEqual(["Vector › Geometry", "Vector › Overlay", "Raster › Terrain", "Hydrology", "Zeta", "Output"]);
  });
  it("builds group → sub-group → tools with counts", () => {
    const t = buildTree([
      { category: "Vector › Overlay", name: "Union" },
      { category: "Vector › Overlay", name: "Clip" },
      { category: "Vector › Geometry", name: "Voronoi" },
      { category: "Hydrology", name: "Basins" },
    ]);
    expect(t.map((g) => [g.group, g.count])).toEqual([["Vector", 3], ["Hydrology", 1]]);
    expect(t[0].subs.map((s) => s.name)).toEqual(["Geometry", "Overlay"]);
    expect(t[0].subs[1].tools.map((x) => x.name)).toEqual(["Clip", "Union"]);
    expect(t[1].tools.map((x) => x.name)).toEqual(["Basins"]);
  });
});
