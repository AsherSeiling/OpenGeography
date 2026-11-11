import { describe, expect, it } from "vitest";
import { buildLayers } from "./style";
import { applyStyleParts } from "../layers/multiselect";
import { styleFromSymbologyNode } from "../layers/nodeStyle";
import type { LayerStyle } from "../store/store";

const base: LayerStyle = { kind: "single", fill: "#336699", stroke: "#4d4d4d", strokeWidth: 1, opacity: 0.8, radius: 4 };

describe("colours with alpha on the map", () => {
  it("old #rrggbb styles produce exactly the colours they always did", () => {
    const layers = buildLayers("a", "Polygon", base, true);
    expect(layers.find((l) => l.type === "fill")?.paint?.["fill-color"]).toBe("#336699");
    expect(layers.find((l) => l.type === "line")?.paint?.["line-color"]).toBe("#4d4d4d");
    expect(buildLayers("a", "Point", base, true)[0].paint?.["circle-stroke-color"]).toBe("#333333");
  });
  it("alpha becomes rgba() and a polygon outline keeps its own alpha (line-opacity 1)", () => {
    const s = { ...base, fill: "#ff000080", stroke: "#0000ff80" };
    const layers = buildLayers("a", "Polygon", s, true);
    expect(layers.find((l) => l.type === "fill")?.paint?.["fill-color"]).toBe("rgba(255,0,0,0.502)");
    const line = layers.find((l) => l.type === "line")!;
    expect(line.paint?.["line-color"]).toBe("rgba(0,0,255,0.502)");
    expect(line.paint?.["line-opacity"]).toBe(1);
  });
  it("a line layer hands colour alpha and the layer opacity to the renderer, which multiplies them", () => {
    const l = buildLayers("a", "Line", { ...base, fill: "#ff000080", opacity: 0.5 }, true).find((x) => x.type === "line")!;
    expect(l.paint?.["line-color"]).toBe("rgba(255,0,0,0.502)");
    expect(l.paint?.["line-opacity"]).toBe(0.5);
  });
  it("class and stroke-stack colours convert too", () => {
    const cat = buildLayers("a", "Polygon", { ...base, kind: "categorized", field: "f", classes: [{ value: "x", color: "#11223344", label: "X" }] }, true)[0];
    expect(JSON.stringify(cat.paint?.["fill-color"])).toContain("rgba(17,34,51,0.267)");
    const stack = buildLayers("a", "Line", { ...base, strokes: [{ color: "#00ff0080", width: 2 }] }, true).find((x) => x.type === "line")!;
    expect(stack.paint?.["line-color"]).toBe("rgba(0,255,0,0.502)");
  });
  it("the point outline is optional", () => {
    expect(buildLayers("a", "Point", { ...base, pointStroke: "#ff000080" }, true)[0].paint?.["circle-stroke-color"]).toBe("rgba(255,0,0,0.502)");
  });
});

describe("alpha survives copying and the Symbology node", () => {
  it("copy style keeps the stored colours and the point outline", () => {
    const src = { ...base, fill: "#ff000080", stroke: "#00ff0040", pointStroke: "#0000ff80" };
    const out = applyStyleParts(src, { ...base, fill: "#000000", stroke: "#000000" }, { geomType: "Polygon", fields: [] }, "symbology")!.style;
    expect(out.fill).toBe("#ff000080");
    expect(out.stroke).toBe("#00ff0040");
    expect(out.pointStroke).toBe("#0000ff80");
  });
  it("the node accepts #rrggbbaa", () => {
    expect(styleFromSymbologyNode({ fill: "#ff000080" }, base).fill).toBe("#ff000080");
  });
});
