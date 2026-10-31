import { describe, expect, it } from "vitest";
import type { TopoSymbol } from "../ipc/api";
import type { LayerStyle } from "../store/store";
import { clearSymbol, symbolGroups, symbolKinds, svgUri, wearSymbol } from "./symbols";

const sym = (id: string, geom: TopoSymbol["geom"], category: string, style: TopoSymbol["style"] = {}): TopoSymbol => ({ id, geom, category, name: id, style });
const base: LayerStyle = { kind: "categorized", field: "cls", classes: [{ value: "a", color: "#fff", label: "A" }], fill: "#111111", stroke: "#222222", strokeWidth: 0.7, opacity: 0.75, radius: 3 };

describe("topo symbols", () => {
  it("offers a layer the symbols of its geometry, grouped in catalogue order", () => {
    const all = [sym("a", "line", "Roads"), sym("b", "area", "Vegetation"), sym("c", "line", "Contours"), sym("d", "line", "Roads"), sym("e", "point", "Control")];
    expect(symbolGroups(all, "Line").map((g) => [g.category, g.items.map((i) => i.id)])).toEqual([["Roads", ["a", "d"]], ["Contours", ["c"]]]);
    expect(symbolGroups(all, "Polygon").map((g) => g.category)).toEqual(["Vegetation"]);
    expect(symbolGroups(all, "Raster")).toEqual([]);
    expect(symbolKinds("Mixed")).toEqual(["line", "area", "point"]);
  });
  it("wears a symbol: its fields win, classes go, unset fields stay", () => {
    const road = sym("road", "line", "Roads", { fill: "#c8202f", strokes: [{ color: "#000000", width: 4 }, { color: "#c8202f", width: 2.5 }] });
    const s = wearSymbol(base, road);
    expect(s.kind).toBe("single");
    expect(s.classes).toBeUndefined();
    expect(s.field).toBeUndefined();
    expect(s.strokes).toHaveLength(2);
    expect(s.fill).toBe("#c8202f");
    expect(s.stroke).toBe("#222222");
    expect(s.opacity).toBe(0.75);
    expect(s.symbol).toBe("road");
  });
  it("replaces the previous symbol's parts and can remove them", () => {
    const first = wearSymbol(base, sym("m", "area", "Veg", { pattern: "marsh", patternSpacing: 6 }));
    expect(first.pattern).toBe("marsh");
    const second = wearSymbol(first, sym("w", "area", "Veg", { fill: "#cfe8c5" }));
    expect(second.pattern).toBeUndefined();
    expect(clearSymbol({ ...first, strokes: [{ color: "#000", width: 1 }], marker: "star" })).toMatchObject({ strokes: undefined, pattern: undefined, marker: undefined });
  });
  it("makes an image source from SVG", () => {
    expect(svgUri("<svg a='1'/>")).toBe("data:image/svg+xml;utf8,%3Csvg%20a%3D'1'%2F%3E");
  });
});
