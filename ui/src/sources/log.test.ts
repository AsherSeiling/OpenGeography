import { describe, expect, it } from "vitest";
import type { LayerInfo, SourceEntry } from "../ipc/api";
import { PASTED_KIND, reference, referenceText } from "./cite";
import { uncitedLayers } from "./log";

const layer = (id: string) => ({ id, name: `Layer ${id}` }) as LayerInfo;
const entry = (key: string, ids: string[]) => ({ citation: { key } as never, firstUsed: "2026-01-01", uses: ids.map((layerId) => ({ layerId, layerName: layerId, at: "2026-01-01" })) }) as SourceEntry;

describe("UR-13 layers without a citation", () => {
  it("lists the layers no source entry mentions, in the given order", () => {
    const layers = { A: layer("A"), B: layer("B"), C: layer("C") };
    expect(uncitedLayers(layers, ["C", "B", "A"], [entry("s1", ["B"]), entry("s2", ["B", "X"])]).map((l) => l.id)).toEqual(["C", "A"]);
    expect(uncitedLayers(layers, ["A"], [])).toHaveLength(1);
    expect(uncitedLayers(layers, ["A", "gone"], [entry("s", ["A"])])).toEqual([]);
  });

  it("a pasted citation is printed exactly as pasted in every style", () => {
    const c = { key: "user:A-1", author: "", year: "", title: "Smith, J. (2020). Soil survey. USDA.", version: "", publisher: "", url: "", kind: PASTED_KIND, license: "", annotation: "", complete: true };
    for (const s of ["apa", "mla", "chicago"] as const) expect(referenceText(c, "2026-01-01", s)).toBe("Smith, J. (2020). Soil survey. USDA.");
    expect(reference(c, "2026-01-01").title).toBe("");
  });
});
