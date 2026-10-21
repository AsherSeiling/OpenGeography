import { describe, expect, it } from "vitest";
import type { LayerStyle } from "../store/store";
import { buildLayers, parseScale, rampColors, scaleForZoom, styleFields, zoomForScale } from "./style";

const base: LayerStyle = { kind: "single", fill: "#ff0000", stroke: "#000000", strokeWidth: 1, opacity: 0.8, radius: 3 };

describe("symbology", () => {
  it("builds fill + outline for polygons and circles for points", () => {
    expect(buildLayers("L1", "Polygon", base).map((l) => l.type)).toEqual(["fill", "line"]);
    expect(buildLayers("L1", "Point", base).map((l) => l.type)).toEqual(["circle"]);
    expect(buildLayers("L1", "Line", base).map((l) => l.type)).toEqual(["line"]);
  });

  it("only requests style fields in tiles when needed", () => {
    expect(styleFields(base)).toEqual([]);
    expect(styleFields({ ...base, kind: "categorized", field: "TYPE", classes: [] })).toEqual(["TYPE"]);
  });

  it("uses match for unique values and step for graduated", () => {
    const cat = buildLayers("L1", "Polygon", { ...base, kind: "categorized", field: "T", classes: [{ value: "a", color: "#111111", label: "a" }] });
    expect(JSON.stringify(cat[0].paint)).toContain('"match"');
    const grad = buildLayers("L1", "Polygon", { ...base, kind: "graduated", field: "P", breaks: [10, 20], colors: ["#000000", "#888888", "#ffffff"] });
    expect(JSON.stringify(grad[0].paint)).toContain('"step"');
  });
});

describe("scale", () => {
  it("round-trips scale and zoom", () => {
    const z = zoomForScale(24000, 35);
    expect(scaleForZoom(z, 35)).toBeCloseTo(24000, 6);
  });

  it("parses typed scales", () => {
    expect(parseScale("1:24,000")).toBe(24000);
    expect(parseScale("50k")).toBe(50000);
    expect(parseScale("1:1m")).toBe(1000000);
    expect(parseScale("abc")).toBeNull();
  });

  it("interpolates color ramps", () => {
    expect(rampColors(["#000000", "#ffffff"], 3)).toEqual(["#000000", "#808080", "#ffffff"]);
  });
});
