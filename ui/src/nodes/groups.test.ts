import { beforeEach, describe, expect, it } from "vitest";
import type { ToolSpec } from "../ipc/api";
import { defaultTree, useModel, useRun } from "../store/model";
import { useApp } from "../store/store";
import { groupFromNodes, groupProblem, insertGroup, openInputs } from "./groups";

const rule = { from: null, add: [], keepParam: null, noneIf: null, layerParam: null };
const num = (id: string) => ({ id, label: id, kind: "number" as const, default: 10, options: [], fieldOf: null, optional: false, help: "", hidden: false });
const spec = (id: string, role: ToolSpec["role"], inputs: ToolSpec["inputs"], output: ToolSpec["output"], params: ToolSpec["params"] = []): ToolSpec => ({ id, name: id, category: "T", role, description: "", inputs, output, params, fields: rule });
const SPECS = [
  spec("input_layer", "data", [], "any", [{ ...num("layer"), kind: "layer", default: null }]),
  spec("layer_output", "output", [{ id: "in", label: "Result", kind: "any" }], null),
  spec("buffer", "tool", [{ id: "in", label: "Input", kind: "vector" }], "vector", [num("distance")]),
  spec("dissolve", "tool", [{ id: "in", label: "Input", kind: "vector" }], "vector"),
  spec("erase", "tool", [{ id: "in", label: "Input", kind: "vector" }, { id: "erase", label: "Erase features", kind: "vector" }], "vector"),
];
const edge = (source: string, target: string, targetHandle = "in") => ({ id: `${source}-${target}-${targetHandle}`, source, sourceHandle: "out", target, targetHandle });

beforeEach(() => {
  useRun.getState().setSpecs(SPECS);
  useApp.setState({ layers: { L1: { id: "L1", name: "Roads", geomType: "Polygon", version: 1 } as never } });
  const t = defaultTree("L1");
  useModel.setState({ layer: "L1", nodes: t.nodes, edges: t.edges, trees: {} });
  useModel.temporal.getState().clear();
});

function build() {
  const m = useModel.getState();
  const a = m.addNode("buffer", { x: 300, y: 100 }, { distance: 50 });
  const b = m.addNode("erase", { x: 600, y: 140 });
  const lay = m.addNode("input_layer", { x: 100, y: 300 }, { layer: "L1" });
  useModel.setState({ edges: [...useModel.getState().edges, edge(a, b, "in"), edge(lay, b, "erase"), edge("in-L1", a)] });
  return { a, b, lay };
}

describe("groupFromNodes", () => {
  it("keeps settings and inner connections, drops outside connections, layer references and the fixed nodes", () => {
    const { a, b, lay } = build();
    const nodes = useModel.getState().nodes.filter((n) => [a, b, lay, "in-L1"].includes(n.id));
    const g = groupFromNodes("  Buffer then erase ", "", nodes, useModel.getState().edges);
    expect(g.name).toBe("Buffer then erase");
    expect(g.nodes.map((n) => n.tool).sort()).toEqual(["buffer", "erase", "input_layer"]);
    expect(g.nodes.find((n) => n.tool === "buffer")!.params.distance).toBe(50);
    expect(g.nodes.find((n) => n.tool === "input_layer")!.params).toEqual({});
    expect(g.edges).toHaveLength(2); // buffer->erase, layer->erase; the Layer Input -> buffer edge is outside
    expect(Math.min(...g.nodes.map((n) => n.x))).toBe(0);
    expect(Math.min(...g.nodes.map((n) => n.y))).toBe(0);
  });
  it("needs a selection", () => {
    expect(groupProblem(useModel.getState().nodes)).toMatch(/Select the nodes/);
  });
});

describe("insertGroup", () => {
  it("copies nodes with new ids, rebuilds the connections, selects them and is one undo step", () => {
    const { a, b } = build();
    const nodes = useModel.getState().nodes.filter((n) => [a, b].includes(n.id));
    const g = groupFromNodes("G", "", nodes, useModel.getState().edges);
    expect(openInputs(g).map((o) => o.label)).toEqual(["Input", "Erase features"]);
    useModel.temporal.getState().clear();
    const before = useModel.getState().nodes.length;
    const ids = insertGroup(g, { x: 1000, y: 1000 });
    const s = useModel.getState();
    expect(ids).toHaveLength(2);
    expect(s.nodes.length).toBe(before + 2);
    expect(ids.every((id) => id !== a && id !== b)).toBe(true);
    expect(s.nodes.filter((n) => n.selected).map((n) => n.id).sort()).toEqual([...ids].sort());
    const copy = s.nodes.find((n) => n.id === ids[0])!;
    expect(copy.data.params.distance).toBe(50);
    expect(copy.position).toEqual({ x: 1000, y: 1000 });
    expect(s.edges.some((e) => e.source === ids[0] && e.target === ids[1] && e.targetHandle === "in")).toBe(true);
    useModel.temporal.getState().undo();
    expect(useModel.getState().nodes.length).toBe(before);
  });
  it("does nothing without an open tree", () => {
    useModel.setState({ layer: null });
    expect(insertGroup({ format: "gis-studio-node-group", version: 1, name: "G", description: "", nodes: [{ id: "x", tool: "buffer", params: {}, x: 0, y: 0 }], edges: [] }, { x: 0, y: 0 })).toEqual([]);
  });
});
