import { beforeEach, describe, expect, it } from "vitest";
import type { LayerInfo, ToolSpec } from "../ipc/api";
import { defaultTree, useModel, useRun, type ModelNode } from "../store/model";
import { useApp } from "../store/store";
import { connectLayerTo, familyOf, layerProblem } from "./ModelNodes";

const spec = (id: string, role: ToolSpec["role"], inputs: ToolSpec["inputs"], output: ToolSpec["output"]): ToolSpec => ({ id, name: id, category: "T", role, description: "", inputs, output, params: [], fields: { from: null, add: [], keepParam: null, noneIf: null, layerParam: null } });
const layer = (id: string, geomType: string) => ({ id, name: id, geomType, version: 1, fields: [] }) as unknown as LayerInfo;

beforeEach(() => {
  useRun.setState({ specs: {}, results: {}, ranSig: {} });
  useRun.getState().setSpecs([
    spec("input_layer", "data", [], "any"),
    spec("layer_output", "output", [{ id: "in", label: "Result", kind: "any" }], null),
    spec("clip", "tool", [{ id: "in", label: "Input", kind: "vector" }, { id: "clip", label: "Clip features", kind: "vector" }], "vector"),
  ]);
  useApp.setState({ layers: { L1: layer("L1", "Polygon"), L2: layer("L2", "Polygon"), T: layer("T", "Table"), R: layer("R", "Raster") }, order: ["L1", "L2", "T", "R"], dirty: false });
  const t = defaultTree("L1");
  const clip: ModelNode = { id: "c", type: "tool", position: { x: 300, y: 100 }, data: { tool: "clip", params: {} } };
  useModel.setState({ layer: "L1", nodes: [...t.nodes, clip], edges: t.edges, trees: {} });
  useModel.temporal.getState().clear();
});

describe("Choose a layer", () => {
  it("adds a Layer card wired to the empty input as ONE undo step", () => {
    const id = connectLayerTo("c", "clip", "L2")!;
    const s = useModel.getState();
    expect(s.nodes.find((n) => n.id === id)?.data).toMatchObject({ tool: "input_layer", params: { layer: "L2" } });
    expect(s.edges.some((e) => e.source === id && e.target === "c" && e.targetHandle === "clip")).toBe(true);
    useModel.temporal.getState().undo();
    expect(useModel.getState().nodes.some((n) => n.id === id)).toBe(false);
    expect(useModel.getState().edges.some((e) => e.target === "c" && e.targetHandle === "clip")).toBe(false);
  });
  it("honours connectProblem: a table or raster can't feed a features input", () => {
    expect(layerProblem(layer("T", "Table"), "c", "clip")).toMatch(/needs features/);
    expect(connectLayerTo("c", "clip", "T")).toBeNull();
    expect(connectLayerTo("c", "clip", "R")).toBeNull();
    expect(useModel.getState().nodes).toHaveLength(3);
  });
});

describe("card family", () => {
  it("maps categories to a family; Output and Save to File use the rule colour", () => {
    expect(familyOf("Vector › Overlay")).toBe("Vector");
    expect(familyOf("Coordinate Systems")).toBe("Coordinate");
    expect(familyOf("Save to File")).toBe("Output");
    expect(familyOf("Whatever", "output")).toBe("Output");
    expect(familyOf("Input", "data")).toBe("Input");
    expect(familyOf("Unknown")).toBe("Analysis");
  });
});
