import { describe, expect, it } from "vitest";
import type { LayerStyle } from "../store/store";
import { buildLayers, parseScale, rampColors, rasterToken, styleFields } from "./style";

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

describe("OpenLayers variant", () => {
  it("carries no feature-state expressions (OL draws the selection itself)", () => {
    const all = JSON.stringify(buildLayers("L1", "Mixed", base, true));
    expect(all).not.toContain("feature-state");
    expect(JSON.stringify(buildLayers("L1", "Mixed", base))).toContain("feature-state");
  });
});

describe("scale", () => {
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

describe("raster style token", () => {
  it("is base64url JSON the engine can read, only for non-default renderers", () => {
    const raster = { renderer: "pseudocolor" as const, ramp: "viridis", invert: true };
    const tok = rasterToken(raster);
    expect(tok).toMatch(/^[A-Za-z0-9_-]+$/);
    expect(JSON.parse(atob(tok.replace(/-/g, "+").replace(/_/g, "/")))).toEqual(raster);
    expect(styleFields({ ...base, raster })).toEqual([tok]);
    expect(styleFields({ ...base, raster: { renderer: "default" } })).toEqual([]);
    expect(styleFields(base)).toEqual([]);
  });
});
