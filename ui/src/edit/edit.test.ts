import { describe, expect, it } from "vitest";
import { editProblem } from "./editActions";
import { crossesAntimeridian } from "./EditOverlay";
import type { LayerInfo } from "../ipc/api";

const layer = (p: Partial<LayerInfo>) => ({ id: "L1", name: "Wells", geomType: "Point", crs: "EPSG:4326", ...p }) as LayerInfo;

describe("editProblem", () => {
  it("says why a layer can't be edited, in a sentence", () => {
    expect(editProblem(null)).toMatch(/Select a layer/);
    expect(editProblem(layer({ geomType: "Raster" }))).toMatch(/raster/);
    expect(editProblem(layer({ geomType: "Table" }))).toMatch(/no shapes/);
    expect(editProblem(layer({}), true)).toMatch(/node-tree result/);
    expect(editProblem(layer({ crs: null }))).toMatch(/Define Projection/);
    expect(editProblem(layer({}))).toBeNull();
  });
});

describe("antimeridian", () => {
  it("is refused on a geographic map when a segment spans more than half the world", () => {
    expect(crossesAntimeridian([[179, 0], [-179, 0]], "EPSG:4326")).toBe(true);
    expect(crossesAntimeridian([[10, 0], [20, 5]], "EPSG:4326")).toBe(false);
    expect(crossesAntimeridian([[179, 0], [-179, 0]], "EPSG:3857")).toBe(false);
  });
});
