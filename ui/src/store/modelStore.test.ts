import type { Edge } from "@xyflow/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { LayerInfo, ToolSpec } from "../ipc/api";
import { accepts, bypassMuted, connectProblem, defaultTree, flushParamEdit, graphPayload, kindAfter, nodeSignature, outputFields, outputKind, sourceLayerFor, specs, useModel, useRun, type ModelNode } from "./model";
import { useApp } from "./store";

type Fields = ToolSpec["fields"];
const noRule: Fields = { from: null, add: [], keepParam: null, noneIf: null, layerParam: null };
const spec = (id: string, role: ToolSpec["role"], inputs: ToolSpec["inputs"], output: ToolSpec["output"], fields: Partial<Fields> = {}): ToolSpec => ({ id, name: id, category: "T", role, description: "", inputs, output, params: [], fields: { ...noRule, ...fields } });
const IN = (id = "in", kind: "vector" | "table" | "raster" | "any" = "vector") => ({ id, label: id, kind });
const SPECS = [
  spec("input_layer", "data", [], "any", { layerParam: "layer" }),
  spec("layer_output", "output", [IN("in", "any")], null),
  spec("buffer", "tool", [IN()], "vector", { add: [{ name: "BUF_DIST", type: "DOUBLE", whenPositive: "distance", nameParam: null, prefixParam: null } as never] }),
  spec("filter", "tool", [IN("in", "table")], null),
  spec("clip", "tool", [IN(), IN("clip")], null),
  spec("dissolve", "tool", [IN()], "vector", { keepParam: "field" }),
  spec("hillshade", "tool", [IN("dem", "raster")], "raster"),
  spec("summary", "tool", [IN("in", "table")], "table", { noneIf: "off" }),
];

const layer = (id: string, over: Partial<LayerInfo> = {}): LayerInfo => ({ id, name: id, geomType: "Polygon", version: 1, fields: [{ name: "A", type: "VARCHAR" }, { name: "B", type: "DOUBLE" }], ...over }) as never;

const node = (id: string, tool: string, params: Record<string, unknown> = {}, extra: Partial<ModelNode["data"]> = {}): ModelNode => ({ id, type: "tool", position: { x: 0, y: 0 }, data: { tool, params, ...extra } });
const edge = (source: string, target: string, targetHandle = "in"): Edge => ({ id: `${source}>${target}:${targetHandle}`, source, sourceHandle: "out", target, targetHandle });

function setTree(nodes: ModelNode[], edges: Edge[]) {
  useModel.setState({ layer: "L1", nodes, edges, trees: {} });
  useModel.temporal.getState().clear();
}

beforeEach(() => {
  useRun.setState({ specs: {}, results: {}, ranSig: {} });
  useRun.getState().setSpecs(SPECS);
  useApp.setState({ layers: { L1: layer("L1"), L2: layer("L2", { geomType: "Table" }), L3: layer("L3", { geomType: "Raster" }) }, results: {}, showOriginal: {}, dirty: false });
  const t = defaultTree("L1");
  setTree(t.nodes, t.edges);
});

describe("port type compatibility", () => {
  it("accepts() truth table", () => {
    const kinds = [undefined, "vector", "table", "raster"] as const;
    const expected: Record<string, boolean[]> = {
      //          unknown vector table  raster
      vector: [true, true, false, false],
      table: [true, true, true, false],
      raster: [true, false, false, true],
      any: [true, true, true, true],
    };
    for (const port of ["vector", "table", "raster", "any"] as const) kinds.forEach((k, i) => expect(accepts(port, k), `${port} ← ${k}`).toBe(expected[port][i]));
  });

  it("kindAfter: fixed outputs win, pass-through tools keep the input kind, unknown tools are unknown", () => {
    expect(kindAfter("buffer", "table")).toBe("vector");
    expect(kindAfter("hillshade", undefined)).toBe("raster");
    expect(kindAfter("filter", "table")).toBe("table");
    expect(kindAfter("filter", "vector")).toBe("vector");
    expect(kindAfter("no_such_tool", "vector")).toBeUndefined();
  });

  it("outputKind follows layers through pass-through tools and is unknown for a missing layer", () => {
    setTree(
      [node("a", "input_layer", { layer: "L2" }), node("f", "filter"), node("b", "input_layer", { layer: "gone" }), node("r", "input_layer", { layer: "L3" })],
      [edge("a", "f")],
    );
    expect(outputKind("a")).toBe("table");
    expect(outputKind("f")).toBe("table");
    expect(outputKind("b")).toBeUndefined();
    expect(outputKind("r")).toBe("raster");
    expect(outputKind("nope")).toBeUndefined();
  });

  it("outputKind survives a cycle", () => {
    setTree([node("f", "filter"), node("g", "filter")], [edge("f", "g"), edge("g", "f")]);
    expect(outputKind("f")).toBeUndefined();
  });

  it("connectProblem explains each refusal and allows the rest", () => {
    setTree([node("a", "input_layer", { layer: "L2" }), node("b", "buffer"), node("h", "hillshade"), node("c", "clip")], []);
    expect(connectProblem("b", "b", "in")).toMatch(/itself/);
    expect(connectProblem("a", "b", "nope")).toMatch(/no such input/);
    expect(connectProblem("a", "ghost", "in")).toMatch(/no such input/);
    expect(connectProblem("a", "b", "in")).toMatch(/needs features, not a table/);
    expect(connectProblem("a", "h", "dem")).toMatch(/needs a raster/);
    expect(connectProblem("b", "c", "clip")).toBeNull();
  });
});

describe("editing the graph", () => {
  it("a second connection into the same port replaces the first; other ports are untouched", () => {
    setTree([node("a", "input_layer", { layer: "L1" }), node("b", "input_layer", { layer: "L1" }), node("c", "clip")], []);
    const m = useModel.getState();
    m.connect({ source: "a", sourceHandle: "out", target: "c", targetHandle: "in" });
    m.connect({ source: "b", sourceHandle: "out", target: "c", targetHandle: "clip" });
    m.connect({ source: "b", sourceHandle: "out", target: "c", targetHandle: "in" });
    const into = (h: string) => useModel.getState().edges.filter((e) => e.target === "c" && e.targetHandle === h).map((e) => e.source);
    expect(into("in")).toEqual(["b"]);
    expect(into("clip")).toEqual(["b"]);
  });

  it("removeNodes never removes the tree's fixed nodes and drops edges of removed ones", () => {
    const m = useModel.getState();
    const id = m.addNode("buffer", { x: 100, y: 0 });
    m.connect({ source: "in-L1", sourceHandle: "out", target: id, targetHandle: "in" });
    m.removeNodes(["in-L1", "out-L1", id]);
    const s = useModel.getState();
    expect(s.nodes.map((n) => n.id).sort()).toEqual(["in-L1", "out-L1"]);
    expect(s.edges.every((e) => s.nodes.some((n) => n.id === e.source) && s.nodes.some((n) => n.id === e.target))).toBe(true);
  });

  it("addNode applies parameter defaults but lets explicit params win, and marks the project dirty", () => {
    useRun.getState().setSpecs([{ ...SPECS[2], params: [{ id: "distance", label: "d", kind: "number", default: 100, options: [], fieldOf: null, optional: false, help: "", hidden: false }] } as never, ...SPECS.filter((s) => s.id !== "buffer")]);
    const m = useModel.getState();
    const a = m.addNode("buffer", { x: 0, y: 0 });
    const b = m.addNode("buffer", { x: 0, y: 0 }, { distance: 5 });
    const get = (id: string) => useModel.getState().nodes.find((n) => n.id === id)!.data.params;
    expect(get(a)).toEqual({ distance: 100 });
    expect(get(b)).toEqual({ distance: 5 });
    expect(useApp.getState().dirty).toBe(true);
  });

  it("addNode for a tool the engine doesn't know still adds a bare node", () => {
    const id = useModel.getState().addNode("mystery", { x: 1, y: 2 });
    expect(useModel.getState().nodes.find((n) => n.id === id)?.data).toEqual({ tool: "mystery", params: {} });
  });

  it("recording run outputs is neither an undo step nor a project edit, and ignores empty maps", () => {
    const m = useModel.getState();
    m.setLastOutputs({});
    m.setLastOutputs({ "in-L1": "__node_abc" });
    expect(useModel.getState().lastOutputs).toEqual({ L1: { "in-L1": "__node_abc" } });
    expect(useModel.temporal.getState().pastStates).toHaveLength(0);
    expect(useApp.getState().dirty).toBe(false);
    m.setLastOutputs({ nobody: "__node_x" }); // unknown node ids are ignored
    expect(useModel.getState().nodes).toHaveLength(2);
    expect(useModel.getState().lastOutputs.L1).toEqual({ "in-L1": "__node_abc" });
  });

  it("B5: undo and redo keep each node's last output", () => {
    const m = useModel.getState();
    const id = m.addNode("buffer", { x: 100, y: 0 });
    useModel.temporal.getState().clear();
    m.setParam(id, "distance", 5);
    flushParamEdit();
    useModel.getState().setLastOutputs({ [id]: "__node_b1" });
    useModel.temporal.getState().undo(); // back to before the edit
    expect(useModel.getState().lastOutputs.L1[id]).toBe("__node_b1");
    useModel.temporal.getState().redo();
    expect(useModel.getState().lastOutputs.L1[id]).toBe("__node_b1");
    useModel.getState().removeNodes([id]);
    expect(useModel.getState().lastOutputs.L1?.[id]).toBeUndefined();
  });

  it("B5: load() moves a lastOutput found in an older project's node data into the map", () => {
    const old = defaultTree("L1");
    old.nodes[0].data = { ...old.nodes[0].data, lastOutput: "__node_old" } as ModelNode["data"];
    useModel.getState().load({ L1: old }, "L1");
    expect(useModel.getState().lastOutputs).toEqual({ L1: { "in-L1": "__node_old" } });
    expect(useModel.getState().nodes[0].data).not.toHaveProperty("lastOutput");
    useModel.getState().load({ L1: old }, "L1", { L1: { "in-L1": "__node_new" } }); // the map wins
    expect(useModel.getState().lastOutputs.L1["in-L1"]).toBe("__node_new");
  });
});

describe("trees", () => {
  it("allTrees returns stripped data (no selection or measurements) and load() restores the open tree", () => {
    const t = defaultTree("L1");
    const withUiState = t.nodes.map((n) => ({ ...n, selected: true, measured: { width: 9, height: 9 } }));
    useModel.setState({ layer: "L1", nodes: withUiState as never, edges: t.edges, trees: {} });
    const saved = useModel.getState().allTrees();
    expect(JSON.stringify(saved)).not.toMatch(/selected|measured/);
    expect(saved.L1.nodes.every((n) => n.deletable === false)).toBe(true);
    useModel.getState().load(structuredClone(saved), "L1");
    expect(useModel.getState().nodes.map((n) => n.id)).toEqual(["in-L1", "out-L1"]);
    expect(useModel.temporal.getState().pastStates).toHaveLength(0);
  });

  it("opening the same layer again keeps edits and history; opening none empties the canvas", () => {
    useModel.getState().addNode("buffer", { x: 0, y: 0 });
    useModel.getState().openTree("L1");
    expect(useModel.getState().nodes).toHaveLength(3);
    useModel.getState().openTree(null);
    expect(useModel.getState().nodes).toEqual([]);
    expect(useModel.getState().allTrees().L1.nodes).toHaveLength(3); // kept for saving
  });

  it("load() with a tree for a layer that has none opens a fresh default tree", () => {
    useModel.getState().load({}, "L2");
    expect(useModel.getState().nodes.map((n) => n.id)).toEqual(["in-L2", "out-L2"]);
  });
});

describe("bypassing muted nodes", () => {
  const chain = () => [
    [node("in", "input_layer", { layer: "L1" }, { fixed: true }), node("m1", "buffer", {}, { muted: true }), node("m2", "buffer", {}, { muted: true }), node("out", "layer_output", {}, { fixed: true })] as ModelNode[],
    [edge("in", "m1"), edge("m1", "m2"), edge("m2", "out")],
  ] as const;

  it("two muted nodes in a row collapse to a direct link", () => {
    const [n, e] = chain();
    const r = bypassMuted([...n], [...e]);
    expect(r.nodes.map((x) => x.id)).toEqual(["in", "out"]);
    expect(r.edges.map((x) => [x.source, x.target])).toEqual([["in", "out"]]);
  });

  it("a muted node with nothing connected to its first input cuts its outputs", () => {
    const [n] = chain();
    const r = bypassMuted([...n], [edge("m1", "m2"), edge("m2", "out")]);
    expect(r.edges).toEqual([]);
  });

  it("fixed nodes are never bypassed even if flagged muted", () => {
    const n = [node("in", "input_layer", { layer: "L1" }, { fixed: true, muted: true })];
    expect(bypassMuted(n, []).nodes).toHaveLength(1);
  });

  it("only the first input is passed through; edges into other ports of the muted node are dropped", () => {
    const n = [node("a", "input_layer", { layer: "L1" }), node("b", "input_layer", { layer: "L1" }), node("c", "clip", {}, { muted: true }), node("o", "layer_output")];
    const r = bypassMuted(n, [edge("a", "c", "in"), edge("b", "c", "clip"), edge("c", "o")]);
    expect(r.edges.map((x) => `${x.source}>${x.target}`)).toEqual(["a>o"]);
  });
});

describe("what is sent to the engine", () => {
  it("Layer Input nodes added from another layer use that layer's displayed result; the fixed one reads the original", () => {
    useApp.setState({ results: { L2: layer("L2-result", { resultOf: "L2" }) } });
    setTree(
      [node("fixed", "input_layer", { layer: "L1" }, { fixed: true }), node("extra", "input_layer", { layer: "L2" }), node("plain", "input_layer", { layer: "L3" }), node("f", "filter", { where: "x" })],
      [{ ...edge("extra", "f"), targetHandle: null as never }],
    );
    const p = graphPayload();
    const byId = Object.fromEntries(p.nodes.map((n) => [n.id, n.params]));
    expect(byId.extra).toEqual({ layer: "L2-result" });
    expect(byId.fixed).toEqual({ layer: "L1" });
    expect(byId.plain).toEqual({ layer: "L3" });
    expect(p.edges[0].targetHandle).toBe(""); // never null on the wire
  });

  it("a deleted layer is sent unchanged so the engine can say 'no longer exists'", () => {
    setTree([node("x", "input_layer", { layer: "deleted" })], []);
    expect(graphPayload().nodes[0].params).toEqual({ layer: "deleted" });
  });
});

describe("change signatures", () => {
  it("handles cycles and missing nodes without recursing forever", () => {
    setTree([node("a", "filter"), node("b", "filter")], [edge("a", "b"), edge("b", "a")]);
    expect(() => nodeSignature("a")).not.toThrow();
    expect(nodeSignature("zzz")).toBe("missing");
  });

  it("is independent of the order edges are listed in, and changes with the layer version", () => {
    const nodes = [node("a", "input_layer", { layer: "L1" }), node("b", "input_layer", { layer: "L1" }), node("c", "clip")];
    const e1 = edge("a", "c", "in");
    const e2 = edge("b", "c", "clip");
    setTree(nodes, [e1, e2]);
    const s1 = nodeSignature("c");
    setTree(nodes, [e2, e1]);
    expect(nodeSignature("c")).toBe(s1);
    useApp.setState({ layers: { ...useApp.getState().layers, L1: layer("L1", { version: 2 }) } });
    expect(nodeSignature("c")).not.toBe(s1);
  });
});

describe("output fields before running", () => {
  it("a missing node or tool has no fields; Layer Input reads its layer's fields (or none if gone)", () => {
    setTree([node("a", "input_layer", { layer: "L1" }), node("g", "input_layer", { layer: "gone" }), node("w", "wizardry")], []);
    expect(outputFields("nope")).toEqual([]);
    expect(outputFields("w")).toEqual([]);
    expect(outputFields("a").map((f) => f.name)).toEqual(["A", "B"]);
    expect(outputFields("g")).toEqual([]);
  });

  it("keepParam with no value keeps nothing; with a value only that field", () => {
    const base = [node("a", "input_layer", { layer: "L1" })];
    setTree([...base, node("d", "dissolve", {})], [edge("a", "d")]);
    expect(outputFields("d")).toEqual([]);
    setTree([...base, node("d", "dissolve", { field: "B" })], [edge("a", "d")]);
    expect(outputFields("d").map((f) => f.name)).toEqual(["B"]);
  });

  it("conditional added fields appear only for a positive number", () => {
    const nodes = (distance: unknown) => [node("a", "input_layer", { layer: "L1" }), node("b", "buffer", { distance })];
    for (const [d, has] of [[10, true], [0, false], [-5, false], ["abc", false], [undefined, false]] as const) {
      setTree(nodes(d), [edge("a", "b")]);
      expect(outputFields("b").some((f) => f.name === "BUF_DIST"), String(d)).toBe(has);
    }
  });

  it("noneIf drops everything when the flag is set", () => {
    setTree([node("a", "input_layer", { layer: "L2" }), node("s", "summary", { off: true })], [edge("a", "s")]);
    expect(outputFields("s")).toEqual([]);
  });

  it("sourceLayerFor walks back to the Contents layer and stops at cycles", () => {
    setTree([node("a", "input_layer", { layer: "L1" }), node("b", "buffer"), node("c", "filter")], [edge("a", "b"), edge("b", "c")]);
    expect(sourceLayerFor("c", "in")).toBe("L1");
    expect(sourceLayerFor("a", "in")).toBeUndefined();
    setTree([node("f", "filter"), node("g", "filter")], [edge("f", "g"), edge("g", "f")]);
    expect(sourceLayerFor("f", "in")).toBeUndefined();
  });

  it("specs() reflects what the engine reported", () => {
    expect(Object.keys(specs())).toContain("buffer");
  });
});
