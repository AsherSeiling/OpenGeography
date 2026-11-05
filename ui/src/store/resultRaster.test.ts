import { describe, expect, it } from "vitest";
import type { LayerInfo } from "../ipc/api";
import { effectiveStyle, resultRasterStyle, type LayerStyle } from "./store";

const raster = (id: string, min: number, max: number, extra: Partial<LayerInfo> = {}) =>
  ({ id, name: id, geomType: "Raster", raster: { kind: "elevation", min, max, empty: false, bands: 1 }, ...extra }) as unknown as LayerInfo;
const dem: LayerStyle = { kind: "single", fill: "#000", stroke: "#000", strokeWidth: 1, opacity: 1, radius: 3, raster: { renderer: "pseudocolor", ramp: "magma", shading: { mode: "hillshade", strength: 1 } } };

describe("TP-11 result rasters get a renderer of their own", () => {
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
});
