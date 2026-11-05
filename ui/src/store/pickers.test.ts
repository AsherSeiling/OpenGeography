import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { LayerInfo } from "../ipc/api";
import { projectLayerInfos } from "./maps";

const layer = (id: string) => ({ id, name: id, extent: [0, 0, 1, 1] }) as unknown as LayerInfo;

describe("MM-4 layer pickers offer every project layer", () => {
  // Two maps: the active one shows A and B, the other shows C. D is on no map.
  const layers = { A: layer("A"), B: layer("B"), C: layer("C"), D: layer("D") };
  it("lists the active map's layers first, then the layers of other maps and unplaced ones", () => {
    expect(projectLayerInfos({ order: ["B", "A"], layers }).map((l) => l.id)).toEqual(["B", "A", "C", "D"]);
    // Switching to the other map changes the order of the list, never what is in it.
    expect(projectLayerInfos({ order: ["C"], layers }).map((l) => l.id)).toEqual(["C", "A", "B", "D"]);
  });
  it("ignores ids in the order that are not layers any more", () => {
    expect(projectLayerInfos({ order: ["gone", "A"], layers }).map((l) => l.id)).toEqual(["A", "B", "C", "D"]);
  });
  it("no picker reads the active map's order directly", () => {
    const files = ["nodes/ModelNodes.tsx", "nodes/ToolMenu.tsx", "nodes/ParamControl.tsx", "panes/PropertiesPane.tsx", ...["Overture", "Stac", "NationalMap", "ArcGis", "Ogc", "Elevation", "HazardsSoils"].map((d) => `dialogs/${d}Dialog.tsx`)];
    for (const f of files) {
      const src = readFileSync(new URL(`../${f}`, import.meta.url), "utf8");
      expect(src, f).not.toMatch(/s\.order\.map\(/);
      expect(src, f).toContain("projectLayerInfos");
    }
  });
});
