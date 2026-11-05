import { beforeEach, describe, expect, it } from "vitest";
import type { LayerInfo } from "../ipc/api";
import { effectiveStyle, restoredResultStyles, resultRasterStyle, savedResultStyles, setResultLookResolver, type LayerStyle } from "./store";
import { resultLookFor, type Tree } from "./model";
import type { ToolSpec } from "../ipc/api";

const raster = (id: string, min: number, max: number, extra: Partial<LayerInfo> = {}) =>
  ({ id, name: id, geomType: "Raster", raster: { kind: "elevation", min, max, empty: false, bands: 1 }, ...extra }) as unknown as LayerInfo;
const dem: LayerStyle = { kind: "single", fill: "#000", stroke: "#000", strokeWidth: 1, opacity: 1, radius: 3, raster: { renderer: "pseudocolor", ramp: "magma", shading: { mode: "hillshade", strength: 1 } } };

describe("TP-11 result rasters get a renderer of their own", () => {
  beforeEach(() => setResultLookResolver(() => "derived"));
  it("a result takes a plain ramp over its own range, never the source's renderer, and the source is untouched", () => {
    const s = { styles: { L1: dem }, results: { L1: raster("L1r", 0, 9000, { resultOf: "L1" }) }, showOriginal: {}, resultRasterStyles: {} };
    const eff = effectiveStyle(s, "L1")!;
    expect(eff.raster).toEqual({ renderer: "pseudocolor", ramp: "viridis", stretch: { mode: "minmax" } });
    expect(eff.raster?.shading).toBeUndefined();
    expect(s.styles.L1.raster?.ramp).toBe("magma"); // the source raster keeps its look
  });
  it("a single-valued result (a stream mask) is one visible class", () => {
    expect(resultRasterStyle(raster("L1r", 1, 1))).toEqual({ renderer: "paletted", entries: [{ value: 1, color: "#1f78b4", label: "1" }] });
  });
  it("showing the original, or a non-raster, keeps the layer's own style; a style the user set on the result wins", () => {
    const res = raster("L1r", 0, 5, { resultOf: "L1" });
    expect(effectiveStyle({ styles: { L1: dem }, results: { L1: res }, showOriginal: { L1: true }, resultRasterStyles: {} }, "L1")).toBe(dem);
    expect(effectiveStyle({ styles: { L1: dem }, results: {}, showOriginal: {}, resultRasterStyles: {} }, "L1")).toBe(dem);
    const mine = { renderer: "gray" as const };
    expect(effectiveStyle({ styles: { L1: dem }, results: { L1: res }, showOriginal: {}, resultRasterStyles: { L1: mine } }, "L1")!.raster).toBe(mine);
  });
  it("imagery and empty rasters stay on the default renderer", () => {
    expect(resultRasterStyle({ ...raster("x", 0, 1), raster: { kind: "rgb", min: 0, max: 255, empty: false } } as unknown as LayerInfo)).toEqual({ renderer: "default" });
    expect(resultRasterStyle(raster("x", 0, 0, { raster: { kind: "elevation", min: 0, max: 0, empty: true } } as any))).toEqual({ renderer: "default" });
  });

  it("by what made it: a hillshade is grey, a clip keeps the source's renderer (and the source style object is never replaced or written)", () => {
    const res = raster("L1r", 0, 255, { resultOf: "L1" });
    const s = { styles: { L1: dem }, results: { L1: res }, showOriginal: {}, resultRasterStyles: {} };
    const frozen = JSON.stringify(dem);
    setResultLookResolver(() => "gray");
    expect(effectiveStyle(s, "L1")!.raster).toEqual({ renderer: "gray", stretch: { mode: "minmax" } });
    setResultLookResolver(() => "source");
    expect(effectiveStyle(s, "L1")).toBe(dem);
    setResultLookResolver(() => "derived");
    expect(effectiveStyle(s, "L1")!.raster?.renderer).toBe("pseudocolor");
    expect(JSON.stringify(dem)).toBe(frozen);
  });

  it("the look comes from the tool that feeds the tree's Output (ToolSpec.resultLook)", () => {
    const specs = { hillshade: { resultLook: "gray" }, clip_raster: { resultLook: "source" }, slope: { resultLook: "derived" }, mystery: {} } as unknown as Record<string, ToolSpec>;
    const tree = (tool: string): Tree => ({
      nodes: [{ id: "in", data: { tool: "input_layer", params: {} } }, { id: "t", data: { tool, params: {} } }, { id: "out", data: { tool: "layer_output", params: {} } }] as never,
      edges: [{ id: "e1", source: "in", target: "t" }, { id: "e2", source: "t", target: "out" }] as never,
    });
    expect(resultLookFor(tree("hillshade"), specs)).toBe("gray");
    expect(resultLookFor(tree("clip_raster"), specs)).toBe("source");
    expect(resultLookFor(tree("slope"), specs)).toBe("derived");
    expect(resultLookFor(tree("mystery"), specs)).toBe("derived");
    expect(resultLookFor(undefined, specs)).toBe("source"); // no tree: nothing made it
    const direct: Tree = { nodes: [{ id: "in", data: { tool: "input_layer", params: {} } }, { id: "out", data: { tool: "layer_output", params: {} } }] as never, edges: [{ id: "e", source: "in", target: "out" }] as never };
    expect(resultLookFor(direct, specs)).toBe("source");
  });

  it("styles the user set on a result are saved only when there are any, and come back only for layers that exist", () => {
    expect(savedResultStyles({})).toBeUndefined(); // an old project's file gets no new key
    const mine = { renderer: "gray" as const };
    const saved = JSON.parse(JSON.stringify({ resultRasterStyles: savedResultStyles({ L1: mine, L9: mine }) }));
    expect(restoredResultStyles(saved.resultRasterStyles, ["L1", "L2"])).toEqual({ L1: mine });
    expect(restoredResultStyles(undefined, ["L1"])).toEqual({}); // an old project opens as before
    expect(restoredResultStyles({ L1: "junk" } as never, ["L1"])).toEqual({});
  });
});
