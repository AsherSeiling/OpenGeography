import { describe, expect, it } from "vitest";
import type { ToolSpec } from "../ipc/api";
import { menuToolIds, toolMenus } from "./toolMenus";

const spec = (id: string, category: string, role: ToolSpec["role"] = "tool", name = id): ToolSpec =>
  ({ id, name, category, role, description: "", inputs: [], output: null, params: [], fields: { kind: "same" } }) as unknown as ToolSpec;

const SPECS = [
  spec("buffer", "Vector › Geometry"),
  spec("clip", "Vector › Overlay"),
  spec("hillshade", "Raster › Terrain and Contours"),
  spec("flow_dir", "Hydrology"),
  spec("moran", "Analysis › Spatial Statistics"),
  spec("route", "Analysis › Network"),
  spec("tab_area", "Analysis › Summaries"),
  spec("sort", "Tables"),
  spec("reproject", "Coordinate Systems"),
  spec("layer_input", "Input", "data"),
  spec("save_gpkg", "Output", "output"),
  spec("layer_output", "Output"),
  spec("add_to_map", "Output"),
];

describe("Analysis tool dropdowns", () => {
  it("puts every real tool in exactly one dropdown and leaves out inputs, outputs and the fixed steps", () => {
    const ids = toolMenus(SPECS).flatMap(menuToolIds);
    expect(ids.slice().sort()).toEqual(["buffer", "clip", "flow_dir", "hillshade", "moran", "reproject", "route", "sort", "tab_area"]);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("groups by category with splitCategory: top level is the dropdown, the sub-group a headed section", () => {
    const menus = toolMenus(SPECS);
    expect(menus.map((m) => m.label)).toEqual(["Vector", "Raster", "Hydrology", "Spatial Statistics", "Network", "Summaries", "Tables", "Coordinate Systems"]);
    const vector = menus[0];
    expect(vector.subs.map((s) => s.name)).toEqual(["Geometry", "Overlay"]);
    expect(vector.subs[0].tools.map((t) => t.id)).toEqual(["buffer"]);
    expect(menus.find((m) => m.label === "Tables")!.tools.map((t) => t.id)).toEqual(["sort"]);
  });

  it("shows a tool in a new category with no ribbon change", () => {
    const menus = toolMenus([...SPECS, spec("kde", "Density › Kernel")]);
    expect(menus.at(-1)).toMatchObject({ label: "Density" });
    expect(menus.flatMap(menuToolIds)).toContain("kde");
  });
});
