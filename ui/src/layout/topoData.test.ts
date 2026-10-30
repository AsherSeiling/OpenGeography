import { describe, expect, it } from "vitest";
import { contourStyle, hillshadeStyle, pickDem, sheetLabel } from "./topoData";
import type { LayerInfo } from "../ipc/api";
import type { LayerStyle } from "../store/store";

const layer = (id: string, kind?: string) => ({ id, name: id, raster: kind ? { kind } : undefined }) as unknown as LayerInfo;

describe("Topo Map Builder data step", () => {
  it("picks the topmost elevation raster, ignoring imagery and vectors", () => {
    const layers = { a: layer("a", "rgb"), b: layer("b"), c: layer("c", "elevation"), d: layer("d", "elevation") };
    expect(pickDem(["a", "b", "c", "d"], layers)?.id).toBe("c");
    expect(pickDem(["a", "b"], layers)).toBeNull();
    expect(pickDem([], layers)).toBeNull();
  });
  it("draws index contours darker and labels only them with the elevation", () => {
    const s = contourStyle({ kind: "single", fill: "#fff", stroke: "#000", strokeWidth: 1, opacity: 1, radius: 3 } as LayerStyle);
    expect(s.kind).toBe("categorized");
    expect(s.field).toBe("INDEX");
    expect(s.classes?.map((c) => c.value)).toEqual(["1", "0"]);
    expect(s.label).toMatchObject({ enabled: true, field: "ELEV", indexOnly: true });
  });
  it("draws the hillshade in grays", () => {
    expect(hillshadeStyle({ kind: "single" } as LayerStyle).raster).toMatchObject({ renderer: "gray", band: 1 });
  });
  it("names the sheet", () => {
    expect(sheetLabel("  Example Quad ")).toBe("Example Quad");
    expect(sheetLabel("")).toBe("Topographic map");
  });
});
