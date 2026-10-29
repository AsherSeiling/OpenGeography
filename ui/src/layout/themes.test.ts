import { describe, expect, it } from "vitest";
import type { LayerInfo } from "../ipc/api";
import type { LayerStyle } from "../store/store";
import { recolour } from "./themes";

const info = (geomType: string) => ({ geomType }) as LayerInfo;
const st = (extra: Partial<LayerStyle> = {}): LayerStyle => ({ kind: "single", fill: "#111111", stroke: "#222222", strokeWidth: 0.7, opacity: 1, radius: 3, ...extra });
const palette = [
  { fill: "#aa0000", stroke: "#ffffff", strokeWidth: 0.4, opacity: 0.8 },
  { fill: "#00aa00", stroke: "#eeeeee", strokeWidth: 0.5, opacity: 0.9 },
];

describe("theme palette", () => {
  it("colours single-symbol vector layers from the bottom up and cycles", () => {
    const layers = { a: info("Polygon"), b: info("Line"), c: info("Polygon") };
    const out = recolour(["a", "b", "c"], layers, { a: st(), b: st({ strokeWidth: 2 }), c: st() }, palette);
    expect(out.c.fill).toBe("#aa0000"); // bottom layer first
    expect(out.b).toMatchObject({ fill: "#00aa00", stroke: "#00aa00", strokeWidth: 2 }); // lines keep a visible width, draw in the palette colour
    expect(out.a).toMatchObject({ fill: "#aa0000", stroke: "#ffffff", opacity: 0.8 }); // wrapped around
  });
  it("leaves classified, raster and unknown layers alone", () => {
    const layers = { a: info("Polygon"), r: info("Raster") };
    const out = recolour(["a", "r", "zz"], layers, { a: st({ kind: "graduated" }), r: st() }, palette);
    expect(out).toEqual({});
    expect(recolour(["a"], { a: info("Point") }, { a: st() }, [])).toEqual({});
  });
});
