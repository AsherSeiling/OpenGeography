import { beforeEach, describe, expect, it } from "vitest";
import type { ToolSpec } from "../ipc/api";
import { asOneStep, bypassMuted, connectProblem, defaultTree, nodeSignature, useModel, useRun } from "../store/model";
import { useApp } from "../store/store";
import { copySelection, deleteSelection, makeLinks, paste, toggleMute } from "./graphOps";
import { actionFor, conflicts, defaultBindings, displayCombo, eventCombo, useKeymap } from "./keymap";
import { splice } from "./splice";

const spec = (id: string, role: ToolSpec["role"], inputs: ToolSpec["inputs"], output: ToolSpec["output"]): ToolSpec => ({ id, name: id, category: "T", role, description: "", inputs, output, params: [] });
const SPECS = [
  spec("input_layer", "data", [], "any"),
  spec("layer_output", "output", [{ id: "in", label: "Result", kind: "any" }], null),
  spec("buffer", "tool", [{ id: "in", label: "Input", kind: "vector" }], "vector"),
  spec("clip", "tool", [{ id: "in", label: "Input", kind: "vector" }, { id: "clip", label: "Clip features", kind: "vector" }], null),
  spec("contours", "tool", [{ id: "dem", label: "Elevation", kind: "raster" }], "vector"),
  spec("hillshade", "tool", [{ id: "dem", label: "Elevation", kind: "raster" }], "raster"),
];

const key = (k: string, mods: Partial<Record<"ctrl" | "meta" | "alt" | "shift", boolean>> = {}) => ({
  key: k,
  code: /^[a-z]$/i.test(k) ? `Key${k.toUpperCase()}` : k === "." ? "Period" : k,
  ctrlKey: !!mods.ctrl,
  metaKey: !!mods.meta,
  altKey: !!mods.alt,
  shiftKey: !!mods.shift,
});

beforeEach(() => {
  useRun.getState().setSpecs(SPECS);
  useApp.setState({
    layers: {
      L1: { id: "L1", name: "Roads", geomType: "Line", version: 1 } as never,
      L2: { id: "L2", name: "DEM", geomType: "Raster", version: 1 } as never,
    },
  });
  const t = defaultTree("L1");
  useModel.setState({ layer: "L1", nodes: t.nodes, edges: t.edges, trees: {} });
  useModel.temporal.getState().clear();
});

const add = (tool: string, x = 200) => useModel.getState().addNode(tool, { x, y: 0 });
const edge = (source: string, target: string, targetHandle = "in") => ({ id: `${source}-${target}-${targetHandle}`, source, sourceHandle: "out", target, targetHandle });

describe("keymap", () => {
  it("normalizes key events across platforms and layouts", () => {
    expect(eventCombo(key("a", { shift: true }), false)).toBe("Shift+A");
    expect(eventCombo(key("z", { ctrl: true, shift: true }), false)).toBe("Mod+Shift+Z");
    expect(eventCombo(key("z", { meta: true }), true)).toBe("Mod+Z"); // ⌘ on macOS
    expect(eventCombo(key("z", { ctrl: true }), true)).toBe("Z"); // Ctrl isn't Mod on macOS
    expect(eventCombo({ ...key("a", { alt: true }), key: "å" }, true)).toBe("Alt+A"); // Option+A types å
    expect(eventCombo(key("Shift", { shift: true }))).toBeNull();
    expect(eventCombo(key("."))).toBe(".");
    expect(displayCombo("Mod+Shift+Z", false)).toBe("Ctrl+Shift+Z");
    expect(displayCombo("Mod+Shift+Z", true)).toBe("⌘⇧Z");
  });

  it("maps Blender defaults and moves a combo when it is rebound", () => {
    const d = defaultBindings();
    expect(actionFor(d, "Shift+A")).toBe("add");
    expect(actionFor(d, "G")).toBe("grab");
    expect(actionFor(d, "M")).toBe("mute");
    expect(conflicts(d, "X", "grab")).toEqual(["delete"]);
    useKeymap.getState().reset();
    useKeymap.getState().setBinding("grab", ["G", "X"]);
    const b = useKeymap.getState().bindings;
    expect(b.grab).toEqual(["G", "X"]);
    expect(b.delete).not.toContain("X");
    useKeymap.getState().reset("delete");
    expect(useKeymap.getState().bindings.delete).toContain("X");
    expect(useKeymap.getState().bindings.grab).toEqual(["G"]);
    useKeymap.getState().reset();
  });
});

describe("connections and insertion", () => {
  it("checks port types with a readable reason", () => {
    const b = add("buffer");
    const c = add("contours");
    expect(connectProblem("in-L1", b, "in")).toBeNull();
    expect(connectProblem("in-L1", c, "dem")).toMatch(/needs a raster, not features/);
    expect(connectProblem(b, b, "in")).toMatch(/itself/);
  });

  it("inserts a node into a connection as one undo step, keeping other links", () => {
    const b = add("buffer");
    const e = useModel.getState().edges[0];
    const before = useModel.getState().edges.length;
    expect(asOneStep(() => splice(b, e))).toBeNull();
    const es = useModel.getState().edges;
    expect(es).toHaveLength(before + 1);
    expect(es.some((x) => x.source === "in-L1" && x.target === b)).toBe(true);
    expect(es.some((x) => x.source === b && x.target === "out-L1")).toBe(true);
    useModel.temporal.getState().undo();
    expect(useModel.getState().edges.map((x) => [x.source, x.target])).toEqual([["in-L1", "out-L1"]]);
  });

  it("refuses an insert that doesn't fit either side, changing nothing", () => {
    const h = add("hillshade");
    const e = useModel.getState().edges[0];
    expect(splice(h, e)).toMatch(/needs a raster/);
    expect(useModel.getState().edges).toEqual([e]);
    // A node whose output the next step can't take.
    const b = add("buffer");
    const c = add("contours");
    useModel.setState({ edges: [...useModel.getState().edges, edge("in-L1", b), edge(b, c, "dem")] });
    const dem = add("input_layer");
    useModel.getState().setParam(dem, "layer", "L2");
    const rasterEdge = edge(dem, c, "dem");
    useModel.setState({ edges: [...useModel.getState().edges.filter((x) => x.target !== c), rasterEdge] });
    const b2 = add("buffer");
    expect(splice(b2, rasterEdge)).toMatch(/needs features, not a raster|needs a raster, not features/);
  });
});

describe("graph operations", () => {
  it("delete with reconnect bridges the gap; plain delete doesn't", () => {
    const b = add("buffer");
    splice(b, useModel.getState().edges[0]);
    useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: n.id === b })) });
    deleteSelection(true);
    expect(useModel.getState().edges.map((x) => [x.source, x.target])).toEqual([["in-L1", "out-L1"]]);
    const b2 = add("buffer");
    splice(b2, useModel.getState().edges[0]);
    useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: n.id === b2 || n.data.fixed === true })) });
    deleteSelection(false);
    expect(useModel.getState().nodes.map((n) => n.id).sort()).toEqual(["in-L1", "out-L1"]); // fixed nodes survive
    expect(useModel.getState().edges).toHaveLength(0);
  });

  it("copy/paste keeps internal links, makes new ids, skips fixed nodes", () => {
    const a = add("buffer", 100);
    const b = add("buffer", 300);
    useModel.setState({ edges: [...useModel.getState().edges, edge(a, b)] });
    useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: true })) });
    expect(copySelection()).toBe(2);
    const ids = paste({ x: 1000, y: 500 });
    expect(ids).toHaveLength(2);
    expect(ids).not.toContain(a);
    const es = useModel.getState().edges.filter((e) => ids.includes(e.source) && ids.includes(e.target));
    expect(es).toHaveLength(1);
    expect(Math.min(...useModel.getState().nodes.filter((n) => ids.includes(n.id)).map((n) => n.position.x))).toBe(1000);
  });

  it("make links connects selected nodes left to right into free compatible inputs", () => {
    const b = add("buffer", 200);
    const c = add("clip", 400);
    useModel.setState({ edges: [], nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: n.id !== "out-L1" })) });
    expect(makeLinks()).toBe(2);
    const es = useModel.getState().edges.map((e) => `${e.source}>${e.target}:${e.targetHandle}`);
    expect(es).toEqual([`in-L1>${b}:in`, `${b}>${c}:in`]);
  });
});

describe("mute and change tracking", () => {
  it("a muted node is bypassed in what runs", () => {
    const b = add("buffer");
    splice(b, useModel.getState().edges[0]);
    useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: n.id === b })) });
    toggleMute();
    const { nodes, edges } = bypassMuted(useModel.getState().nodes, useModel.getState().edges);
    expect(nodes.map((n) => n.id)).not.toContain(b);
    expect(edges.map((e) => [e.source, e.target])).toEqual([["in-L1", "out-L1"]]);
  });

  it("signatures change with settings, upstream edits and source-layer edits only", () => {
    const b = add("buffer");
    splice(b, useModel.getState().edges[0]);
    const out0 = nodeSignature("out-L1");
    const b0 = nodeSignature(b);
    useModel.setState({ nodes: useModel.getState().nodes.map((n) => (n.id === b ? { ...n, position: { x: 999, y: 9 }, selected: true } : n)) });
    expect(nodeSignature("out-L1")).toBe(out0); // moving or selecting isn't a change
    useModel.getState().setParam(b, "distance", 50);
    expect(nodeSignature(b)).not.toBe(b0);
    expect(nodeSignature("out-L1")).not.toBe(out0); // downstream sees it
    const out1 = nodeSignature("out-L1");
    useApp.setState({ layers: { ...useApp.getState().layers, L1: { ...useApp.getState().layers.L1, version: 2 } } });
    expect(nodeSignature("out-L1")).not.toBe(out1); // the source layer was edited
  });
});
