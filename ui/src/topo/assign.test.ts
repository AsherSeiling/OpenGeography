import { describe, expect, it } from "vitest";
import type { LayerInfo, TopoSymbol } from "../ipc/api";
import type { LayerStyle } from "../store/store";
import { assignSymbols, RULES } from "./assign";

const sym = (id: string, geom: TopoSymbol["geom"], style: TopoSymbol["style"] = {}): TopoSymbol => ({ id, geom, category: "c", name: id, style });
const SYMS = [
  sym("hyd_stream", "line", { strokes: [{ color: "#169bd0", width: 1 }] }),
  sym("hyd_lake", "area", { fill: "#c8ebf7" }),
  sym("road_primary", "line", { strokes: [{ color: "#000", width: 4 }, { color: "#c8202f", width: 2.6 }] }),
  sym("mil_impact_area", "area", { fill: "#f2a9a9" }),
  sym("mil_firing_point", "point", { marker: "triangle" }),
  sym("con_index", "line", { fill: "#a8662f", strokes: [{ color: "#a8662f", width: 1.5 }] }),
  sym("con_intermediate", "line", { fill: "#a8662f", strokes: [{ color: "#a8662f", width: 0.7 }] }),
];
const info = (name: string, geomType: string) => ({ name, geomType }) as LayerInfo;
const style = (extra: Partial<LayerStyle> = {}): LayerStyle => ({ kind: "single", fill: "#111111", stroke: "#222222", strokeWidth: 1, opacity: 1, radius: 3, ...extra });

describe("assigning topographic symbols by layer", () => {
  it("every rule names a catalogue-style id", () => {
    for (const [, id] of RULES) expect(id).toMatch(/^[a-z]+_[a-z0-9_]+$/);
  });
  it("styles National Map layers, military overlays and contours, and says what it skipped", () => {
    const layers = {
      a: info("Streams and rivers (NHD flowlines)", "Line"),
      b: info("Lakes, ponds and reservoirs (NHD waterbodies)", "Polygon"),
      c: info("Highways (interstate, US and state routes, ramps)", "Line"),
      d: info("North Impact Area", "Polygon"),
      e: info("Firing points", "Point"),
      f: info("Contours 40 ft", "Line"),
      g: info("Mystery layer", "Polygon"),
      h: info("Highways (wrong geometry)", "Polygon"),
      r: info("Hillshade", "Raster"),
    };
    const styles = Object.fromEntries(Object.keys(layers).map((k) => [k, style()]));
    const out = assignSymbols(Object.keys(layers), layers, styles, SYMS);
    expect(Object.keys(out.changes).sort()).toEqual(["a", "b", "c", "d", "e", "f"]);
    expect(out.changes.c.strokes).toHaveLength(2);
    expect(out.changes.f.kind).toBe("categorized");
    expect(out.changes.f.classes?.map((c) => [c.value, c.strokes?.[0].width])).toEqual([["1", 1.5], ["0", 0.7]]);
    expect(out.assigned.find((x) => x.layer === "d")?.symbol).toBe("mil_impact_area");
    expect(out.skipped).toEqual(["Mystery layer: no symbol suggested by its name", "Highways (wrong geometry): “road_primary” is a line symbol and this layer holds polygon features"]);
  });
  it("keeps a layer that already wears a symbol", () => {
    const layers = { a: info("Streams", "Line") };
    expect(assignSymbols(["a"], layers, { a: style({ symbol: "road_trail" }) }, SYMS).changes).toEqual({});
  });
});
