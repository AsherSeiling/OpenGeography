import { beforeEach, describe, expect, it } from "vitest";
import type { ToolSpec } from "../ipc/api";
import { defaultTree, useModel, useRun } from "../store/model";
import { useApp } from "../store/store";
import { useToolMenu } from "./toolMenuStore";
import { describePlacement, placeTool, planPlacement } from "./toolPlacement";

const spec = (id: string, name: string, role: ToolSpec["role"], inputs: ToolSpec["inputs"], output: ToolSpec["output"]): ToolSpec => ({ id, name, category: "T", role, description: "", inputs, output, params: [], fields: { from: null, add: [], keepParam: null, noneIf: null, layerParam: null } });
const SPECS = [
  spec("input_layer", "Layer", "data", [], "any"),
  spec("layer_output", "Output", "output", [{ id: "in", label: "Result", kind: "any" }], null),
  spec("buffer", "Buffer", "tool", [{ id: "in", label: "Input", kind: "vector" }], "vector"),
  spec("clip", "Clip", "tool", [{ id: "in", label: "Input", kind: "vector" }, { id: "clip", label: "Clip features", kind: "vector" }], "vector"),
  spec("hillshade", "Hillshade", "tool", [{ id: "dem", label: "Elevation", kind: "raster" }], "raster"),
];

const nodes = () => useModel.getState().nodes;
const edges = () => useModel.getState().edges;
const select = (id: string | null) => useModel.setState({ nodes: nodes().map((n) => ({ ...n, selected: n.id === id })) });

beforeEach(() => {
  useRun.getState().setSpecs(SPECS);
  useApp.setState({ layers: { L1: { id: "L1", name: "Roads", geomType: "Line", version: 1 } as never } });
  const t = defaultTree("L1");
  useModel.setState({ layer: "L1", nodes: t.nodes, edges: t.edges, trees: {} });
  useModel.temporal.getState().clear();
  useToolMenu.setState({ wire: null });
});

describe("placing a tool from the menu", () => {
  it("rule 3: a tree of only Layer Input → Output gets the tool between them", () => {
    expect(planPlacement("buffer").rule).toBe(3);
    const id = placeTool("buffer");
    expect(edges().map((e) => [e.source, e.target])).toEqual([["in-L1", id], [id, "out-L1"]]);
    expect(nodes().find((n) => n.id === id)?.selected).toBe(true);
    expect(useApp.getState().properties).toEqual({ kind: "node", id });
  });

  it("rule 2: connects after the selected node, splicing in front of what it fed", () => {
    const a = placeTool("buffer");
    const b = placeTool("buffer");
    select(a);
    expect(planPlacement("clip").rule).toBe(2);
    const c = placeTool("clip");
    const order = (from: string): string[] => {
      const e = edges().find((x) => x.source === from);
      return e ? [from, ...order(e.target)] : [from];
    };
    expect(order("in-L1")).toEqual(["in-L1", a, c, b, "out-L1"]);
    select(a);
    expect(describePlacement("buffer")).toBe("Connects after Buffer and before Clip.");
  });

  it("rule 2 at the end of a chain connects the new node to the selected one only", () => {
    const a = placeTool("buffer");
    select("out-L1");
    expect(planPlacement("buffer").rule).toBe(4); // Output has no output to connect
    select(a);
    const before = edges().length;
    placeTool("clip");
    expect(edges().length).toBe(before + 1);
  });

  it("rule 1: splices into the connection under the pointer", () => {
    placeTool("buffer");
    select(null);
    const w = edges().find((e) => e.target === "out-L1")!;
    expect(planPlacement("clip", w.id).rule).toBe(1);
    const c = placeTool("clip", { wire: w.id });
    expect(edges().filter((e) => e.target === c || e.source === c)).toHaveLength(2);
    expect(edges().some((e) => e.id === w.id)).toBe(false);
  });

  it("rule 4: a tool whose first input does not suit the selection is placed unconnected, never connected wrongly", () => {
    const a = placeTool("buffer");
    select(a);
    expect(planPlacement("hillshade").rule).toBe(4);
    expect(describePlacement("hillshade")).toMatch(/unconnected/);
    const before = edges().length;
    const h = placeTool("hillshade");
    expect(edges().length).toBe(before);
    expect(edges().some((e) => e.source === h || e.target === h)).toBe(false);
  });

  it("a raster tool is not spliced into a vector connection", () => {
    const w = edges()[0];
    expect(planPlacement("hillshade", w.id).rule).toBe(4);
  });

  it("a layer card is added unconnected, with its layer", () => {
    const id = placeTool("input_layer", { params: { layer: "L2" } });
    expect(nodes().find((n) => n.id === id)?.data.params.layer).toBe("L2");
    expect(edges()).toHaveLength(1);
  });

  it("is one undo step", () => {
    placeTool("buffer");
    expect(useModel.temporal.getState().pastStates).toHaveLength(1);
  });
});
