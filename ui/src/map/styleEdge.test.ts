import { describe, expect, it } from "vitest";
import type { LayerStyle } from "../store/store";
import { buildLayers, formatScale, layerIds, parseScale, rampColors, RAMPS, styleFields } from "./style";

const single: LayerStyle = { kind: "single", fill: "#112233", stroke: "#000000", strokeWidth: 1, opacity: 0.5, radius: 4 };

describe("parseScale edge inputs", () => {
  it.each([
    ["24000", 24000],
    ["1:24,000", 24000],
    ["1: 24 000", 24000],
    ["50k", 50000],
    ["1:2.5M", 2_500_000],
    ["1.5k", 1500],
    ["1", 1],
  ])("accepts %s", (text, v) => expect(parseScale(text)).toBe(v));

  it.each(["", "   ", "abc", "1:", "1:0", "0", "0.5", "-5", "1e5", "NaN", "Infinity", "1:2:3", "1/24000", "24000x"])("rejects %j", (text) => expect(parseScale(text)).toBeNull());

  it("handles absurdly large input without throwing", () => {
    expect(parseScale("9".repeat(400))).toBe(Infinity);
    expect(formatScale(Infinity)).toBe("1:∞");
  });

  it("formats with thousands separators and rounds", () => {
    expect(formatScale(24000)).toBe("1:24,000");
    expect(formatScale(24000.6)).toBe("1:24,001");
    expect(formatScale(1)).toBe("1:1");
  });
});

describe("rampColors", () => {
  const blues = RAMPS.Blues;
  it("hits the ends exactly and has n entries", () => {
    const c = rampColors(blues, 9);
    expect(c).toHaveLength(9);
    expect(c[0]).toBe(blues[0]);
    expect(c[8]).toBe(blues[4]);
  });
  it("n of 0, 1 and negative give the dark end only (never an empty ramp that breaks `step`)", () => {
    for (const n of [0, 1, -3]) expect(rampColors(blues, n)).toEqual([blues[4]]);
  });
  it("NaN and fractional n don't throw", () => {
    expect(() => rampColors(blues, NaN)).not.toThrow();
    expect(rampColors(blues, 2.5).every((c) => /^#[0-9a-f]{6}$/.test(c))).toBe(true);
  });
  it("a one-colour ramp repeats it", () => {
    expect(rampColors(["#102030"], 3)).toEqual(["#102030", "#102030", "#102030"]);
  });
  it("outputs valid hex", () => {
    for (const r of Object.values(RAMPS)) for (const n of [2, 3, 7, 20]) expect(rampColors(r, n).every((c) => /^#[0-9a-f]{6}$/.test(c))).toBe(true);
  });
});

describe("buildLayers", () => {
  it("emits only the layer kinds a geometry type needs", () => {
    const kinds = (g: Parameters<typeof buildLayers>[1]) => buildLayers("L", g, single).map((l) => l.type).sort();
    expect(kinds("Point")).toEqual(["circle"]);
    expect(kinds("Line")).toEqual(["line"]);
    expect(kinds("Polygon").sort()).toEqual(["fill", "line"]);
    expect(kinds("Raster")).toEqual(["raster"]);
    expect(kinds("Table")).toEqual([]);
  });

  it("every emitted layer id is listed by layerIds (so removal cleans up)", () => {
    for (const g of ["Point", "Line", "Polygon", "Raster", "Mixed"] as const) {
      const ids = layerIds("L");
      for (const l of buildLayers("L", g, single)) expect(ids).toContain(l.id);
    }
  });

  it("categorized without classes, graduated without breaks or colours fall back to the single colour", () => {
    const base = { ...single, field: "kind" };
    const flat = JSON.stringify(buildLayers("L", "Polygon", single, true));
    expect(JSON.stringify(buildLayers("L", "Polygon", { ...base, kind: "categorized", classes: [] }, true))).toBe(flat);
    expect(JSON.stringify(buildLayers("L", "Polygon", { ...base, kind: "graduated", breaks: [], colors: ["#fff"] }, true))).toBe(flat);
    expect(JSON.stringify(buildLayers("L", "Polygon", { ...base, kind: "graduated", breaks: [1], colors: [] }, true))).toBe(flat);
    expect(JSON.stringify(buildLayers("L", "Polygon", { ...base, kind: "categorized" }, true))).toBe(flat);
  });

  it("a categorized style with no field is ignored; with a field the field is requested in tiles", () => {
    expect(styleFields({ ...single, kind: "categorized", classes: [{ value: "a", label: "a", color: "#fff" }] })).toEqual([]);
    expect(styleFields({ ...single, kind: "categorized", field: "f", classes: [] })).toEqual(["f"]);
    expect(styleFields(single)).toEqual([]);
  });

  it("class values with quotes, unicode and empty strings pass through as data", () => {
    const style: LayerStyle = { ...single, kind: "categorized", field: "n", classes: [{ value: "O'Brien", label: "x", color: "#111111" }, { value: "", label: "x", color: "#222222" }, { value: "日本", label: "x", color: "#333333" }] };
    const text = JSON.stringify(buildLayers("L", "Polygon", style, true));
    expect(text).toContain("O'Brien");
    expect(text).toContain("日本");
  });

  it("zero or huge stroke widths and NaN radius don't throw", () => {
    expect(() => buildLayers("L", "Line", { ...single, strokeWidth: 0 })).not.toThrow();
    expect(() => buildLayers("L", "Point", { ...single, radius: NaN })).not.toThrow();
    expect(() => buildLayers("L", "Polygon", { ...single, strokeWidth: 1e9, opacity: 7 })).not.toThrow();
  });
});

describe("TP-1 graduated styles never build an invalid step expression", () => {
  const base = { kind: "graduated", field: "m", fill: "#000000", stroke: "#000000", strokeWidth: 1, opacity: 1 } as never as LayerStyle;
  it("drops tied breaks and breaks without a colour", () => {
    const layers = buildLayers("L", "Point", { ...base, breaks: [10, 10, 10, 72], colors: ["#eff3ff", "#bdd7e7", "#6baed6", "#3182bd", "#08519c"] } as never, true);
    const expr = JSON.stringify(layers[0].paint);
    expect(expr).toContain('"step"');
    expect(expr).not.toMatch(/NaN|undefined/);
    const nums = JSON.parse(expr)["circle-color"].filter((x: unknown) => typeof x === "number" && x !== 0);
    expect(nums).toEqual([10, 72]);
  });
  it("a colour list shorter than the breaks does not put undefined in the expression", () => {
    const layers = buildLayers("L", "Point", { ...base, breaks: [1, 2, 3], colors: ["#111111", "#222222"] } as never, true);
    expect(JSON.stringify(layers[0].paint)).not.toMatch(/null|undefined/);
  });
});
