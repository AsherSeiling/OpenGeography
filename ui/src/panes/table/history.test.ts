import { describe, expect, it, vi } from "vitest";
import type { NodeResult } from "../../ipc/api";
import type { ModelNode, Tree } from "../../store/model";
import { defaultCompareInput, diffTotal, isDiffGone, isNotStored, liveTables, nodeTable, pickerEntries, pruneNodeCache, topoOrder } from "./history";

const SPECS = {
  input_layer: { name: "Layer", role: "data", inputs: [], output: "any" },
  layer_output: { name: "Output", role: "output", inputs: [{ id: "in", label: "Result", kind: "any" }], output: null },
  buffer: { name: "Buffer", role: "tool", inputs: [{ id: "in", label: "Input", kind: "vector" }], output: "vector" },
  filter: { name: "Filter", role: "tool", inputs: [{ id: "in", label: "Input", kind: "any" }], output: "any" },
  hillshade: { name: "Hillshade", role: "tool", inputs: [{ id: "dem", label: "DEM", kind: "raster" }], output: "raster" },
} as never;

const node = (id: string, tool: string, data: Partial<ModelNode["data"]> = {}): ModelNode => ({ id, type: "tool", position: { x: 0, y: 0 }, data: { tool, params: {}, ...data } });
const edge = (source: string, target: string, targetHandle = "in") => ({ id: `${source}-${target}`, source, target, targetHandle });
const res = (id: string, output: string | null, status: NodeResult["status"] = "done"): NodeResult => ({ id, status, error: null, rows: 1, crs: null, output, message: null, layer: null, reprojected: false, millis: 1 });

// Nodes deliberately listed out of order.
const tree: Tree = {
  nodes: [node("out", "layer_output", { fixed: true }), node("f", "filter"), node("b", "buffer"), node("in", "input_layer", { fixed: true })],
  edges: [edge("in", "b"), edge("b", "f"), edge("f", "out")],
};

describe("pickerEntries (H16)", () => {
  it("lists nodes in topological order", () => {
    expect(topoOrder(tree.nodes, tree.edges)).toEqual(["in", "b", "f", "out"]);
    expect(pickerEntries(tree, {}, SPECS).map((e) => e.id)).toEqual(["in", "b", "f", "out"]);
  });

  it("badges follow node state; saved lastOutput counts as stored", () => {
    const e = Object.fromEntries(pickerEntries(tree, { in: res("in", "__node_in"), f: res("f", "__node_f") }, SPECS, new Set(["f"]), { b: "__node_saved_b" }).map((x) => [x.id, x]));
    expect(e.in.state).toBe("ready");
    expect(e.b.state).toBe("ready"); // from lastOutput, no live result
    expect(e.b.table).toBe("__node_saved_b");
    expect(e.f.state).toBe("outofdate");
    expect(e.out.state).toBe("norun");
    expect(e.in.label).toBe("Layer Input");
  });

  it("failed results do not count as a table, muted and raster nodes are disabled", () => {
    const t: Tree = { nodes: [node("a", "filter", { muted: true }), node("h", "hillshade"), node("x", "filter")], edges: [] };
    const e = pickerEntries(t, { x: res("x", "__node_x", "error") }, SPECS);
    expect(e.find((x) => x.id === "a")).toMatchObject({ disabled: true, table: null });
    expect(e.find((x) => x.id === "h")?.disabled).toBe(true);
    expect(e.find((x) => x.id === "x")).toMatchObject({ disabled: false, state: "norun", table: null });
    expect(nodeTable(undefined, res("x", "__node_x", "cached"))).toBe("__node_x");
  });
});

describe("defaultCompareInput (H17)", () => {
  it("is the node feeding the first input", () => {
    expect(defaultCompareInput(tree, "f", SPECS)).toBe("b");
    expect(defaultCompareInput(tree, "out", SPECS)).toBe("f");
  });
  it("has none for Layer Input, and skips muted nodes", () => {
    expect(defaultCompareInput(tree, "in", SPECS)).toBeNull();
    const muted: Tree = { nodes: [node("in", "input_layer"), node("m", "filter", { muted: true }), node("f", "filter")], edges: [edge("in", "m"), edge("m", "f")] };
    expect(defaultCompareInput(muted, "f", SPECS)).toBe("in");
  });
});

describe("pruneNodeCache (H18)", () => {
  it("sends every tree's lastOutput and the pinned table", async () => {
    const trees = { L1: { b: "__node_saved_b" }, L2: { z: "__node_z", y: "__node_saved_b" } };
    expect(liveTables(trees).sort()).toEqual(["__node_saved_b", "__node_z"]);
    const api = { pruneNodeCache: vi.fn().mockResolvedValue(2) };
    expect(await pruneNodeCache(trees, "__node_z", api)).toBe(2);
    expect(api.pruneNodeCache).toHaveBeenCalledWith(expect.arrayContaining(["__node_saved_b", "__node_z"]), "__node_z");
  });
  it("never throws (housekeeping must not block a save)", async () => {
    expect(await pruneNodeCache({}, null, { pruneNodeCache: vi.fn().mockRejectedValue("boom") })).toBe(0);
  });
});

describe("diff helpers", () => {
  it("totals by chip", () => {
    const s = { added: 1, removed: 7, changed: 3 };
    expect(diffTotal(s, "all")).toBe(11);
    expect(diffTotal(s, "removed")).toBe(7);
  });
  it("recognises recoverable engine errors", () => {
    expect(isDiffGone("Comparison __diff_1 is no longer available")).toBe(true);
    expect(isNotStored("Table __node_x is not stored")).toBe(true);
    expect(isNotStored("syntax error")).toBe(false);
  });
});
