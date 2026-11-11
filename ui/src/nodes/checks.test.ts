import { beforeEach, describe, expect, it } from "vitest";
import type { NodeProblem, ToolSpec } from "../ipc/api";
import { defaultTree, useModel, useRun } from "../store/model";
import { useApp } from "../store/store";
import { applyFix, byNode, fieldProblem, fromEngine, localProblems } from "./checks";

const rule = { from: null, add: [], keepParam: null, noneIf: null, layerParam: null };
const spec = (id: string, role: ToolSpec["role"], inputs: ToolSpec["inputs"], output: ToolSpec["output"], params: ToolSpec["params"] = []): ToolSpec => ({ id, name: id, category: "T", role, description: "", inputs, output, params, fields: rule });
const SPECS = [
  { ...spec("input_layer", "data", [], "any"), fields: { ...rule, layerParam: "layer" } },
  spec("layer_output", "output", [{ id: "in", label: "Result", kind: "any" }], null),
  spec("project", "tool", [{ id: "in", label: "Input", kind: "vector" }], "vector", [{ id: "crs", label: "CRS", kind: "crs", default: null, options: [], fieldOf: null, optional: false, help: "", hidden: false }]),
  spec("erase", "tool", [{ id: "in", label: "Input", kind: "vector" }, { id: "erase", label: "Erase features", kind: "vector" }], "vector"),
  spec("dissolve", "tool", [{ id: "in", label: "Input", kind: "vector" }], "vector", [{ id: "field", label: "Dissolve field", kind: "field", default: null, options: [], fieldOf: "in", optional: false, help: "", hidden: false }]),
];
const edge = (source: string, target: string, targetHandle = "in") => ({ id: `${source}-${target}-${targetHandle}`, source, sourceHandle: "out", target, targetHandle });

beforeEach(() => {
  useRun.getState().setSpecs(SPECS);
  useApp.setState({
    layers: { L1: { id: "L1", name: "Roads", geomType: "Polygon", version: 1, fields: [{ name: "County", type: "VARCHAR" }, { name: "pop_2020", type: "INTEGER" }] } as never },
  });
  const t = defaultTree("L1");
  useModel.setState({ layer: "L1", nodes: t.nodes, edges: t.edges, trees: {} });
  useModel.temporal.getState().clear();
});

describe("fieldProblem", () => {
  const fields = [{ name: "County", type: "VARCHAR" }, { name: "pop_2020", type: "INTEGER" }];
  it("is quiet for a field that exists, any case, an empty value, or unknown fields", () => {
    expect(fieldProblem("Input", "County", fields)).toBeNull();
    expect(fieldProblem("Input", "COUNTY", fields)).toBeNull();
    expect(fieldProblem("Input", "", fields)).toBeNull();
    expect(fieldProblem("Input", "x", [])).toBeNull();
  });
  it("names a missing field and suggests the one near match", () => {
    expect(fieldProblem("Input", "Pop 2020", fields)?.suggestion).toBe("pop_2020");
    const none = fieldProblem("Input", "Zip", fields)!;
    expect(none.suggestion).toBeUndefined();
    expect(none.message).toContain('"Zip"');
  });
});

describe("localProblems", () => {
  it("marks an input with nothing connected", () => {
    const id = useModel.getState().addNode("erase", { x: 100, y: 0 });
    useModel.setState({ edges: [...useModel.getState().edges, edge("in-L1", id, "in")] });
    const p = localProblems().filter((x) => x.node === id);
    expect(p.map((x) => x.code)).toEqual(["unconnected"]);
    expect(p[0].message).toContain("Erase features");
  });
  it("marks a chosen field the input data does not have, with a safe suggestion", () => {
    const id = useModel.getState().addNode("dissolve", { x: 100, y: 0 }, { field: "Pop 2020" });
    useModel.setState({ edges: [...useModel.getState().edges, edge("in-L1", id, "in")] });
    const p = localProblems().filter((x) => x.node === id);
    expect(p).toHaveLength(1);
    expect(p[0].code).toBe("missing-field");
    expect(p[0].fix).toMatchObject({ kind: "set-param", value: "pop_2020" });
    applyFix(p[0]);
    expect(useModel.getState().nodes.find((n) => n.id === id)!.data.params.field).toBe("pop_2020");
  });
  it("is quiet for a muted node", () => {
    const id = useModel.getState().addNode("erase", { x: 100, y: 0 });
    useModel.setState({ nodes: useModel.getState().nodes.map((n) => (n.id === id ? { ...n, data: { ...n.data, muted: true } } : n)) });
    expect(localProblems().filter((x) => x.node === id)).toEqual([]);
  });
});

describe("engine problems and the Project fix", () => {
  const mismatch = (node: string): NodeProblem => ({ node, port: "erase", severity: "warning", code: "crs-mismatch", message: "m", fix: { port: "erase", crs: "EPSG:32615" } });
  it("groups by node, errors first", () => {
    const g = byNode([
      { node: "a", severity: "warning", code: "w", message: "w" },
      { node: "a", severity: "error", code: "e", message: "e" },
    ]);
    expect(g.a.map((p) => p.code)).toEqual(["e", "w"]);
  });
  it("puts an explicit Project step in front of the port, as one undo step, keeping the rest of the wiring", () => {
    const s = useModel.getState();
    const other = s.addNode("input_layer", { x: 0, y: 200 }, { layer: "L1" });
    const t = s.addNode("erase", { x: 400, y: 0 });
    useModel.setState({ edges: [...useModel.getState().edges, edge("in-L1", t, "in"), edge(other, t, "erase")] });
    useModel.temporal.getState().clear();
    const p = fromEngine([mismatch(t)])[0];
    expect(p.fix).toMatchObject({ kind: "project", crs: "EPSG:32615" });
    expect(applyFix(p)).toMatch(/Project step/);
    const { nodes, edges } = useModel.getState();
    const proj = nodes.find((n) => n.data.tool === "project")!;
    expect(proj.data.params.crs).toBe("EPSG:32615");
    expect(edges.find((e) => e.target === t && e.targetHandle === "erase")!.source).toBe(proj.id);
    expect(edges.find((e) => e.target === proj.id)!.source).toBe(other);
    expect(edges.find((e) => e.target === t && e.targetHandle === "in")!.source).toBe("in-L1");
    useModel.temporal.getState().undo();
    const after = useModel.getState();
    expect(after.nodes.some((n) => n.data.tool === "project")).toBe(false);
    expect(after.edges.find((e) => e.target === t && e.targetHandle === "erase")!.source).toBe(other);
  });
  it("gives no fix for a missing CRS", () => {
    const p = fromEngine([{ node: "x", port: "in", severity: "error", code: "crs-missing", message: "m", fix: null }])[0];
    expect(p.fix).toBeUndefined();
  });
});
