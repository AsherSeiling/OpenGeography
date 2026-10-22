// Model (node graph) state. Graph edits are undoable; run results are not.
import { addEdge, applyEdgeChanges, applyNodeChanges, type Connection, type Edge, type EdgeChange, type Node, type NodeChange } from "@xyflow/react";
import { create } from "zustand";
import { temporal } from "zundo";
import type { Field, NodeResult, ToolSpec } from "../ipc/api";
import { displayInfo, useApp } from "./store";

export interface ModelNodeData extends Record<string, unknown> {
  tool: string;
  params: Record<string, unknown>;
  /** The tree's own Layer Input / Output nodes: always present, not deletable. */
  fixed?: boolean;
}

export type ModelNode = Node<ModelNodeData>;

export interface Tree {
  nodes: ModelNode[];
  edges: Edge[];
}

interface ModelState {
  /** Layer whose node tree is open; null when no layer is selected. */
  layer: string | null;
  /** Saved trees of the other layers (the open one lives in nodes/edges). */
  trees: Record<string, Tree>;
  nodes: ModelNode[];
  edges: Edge[];
  openTree: (layer: string | null) => void;
  onNodesChange: (c: NodeChange<ModelNode>[]) => void;
  onEdgesChange: (c: EdgeChange[]) => void;
  connect: (c: Connection) => void;
  addNode: (tool: string, position: { x: number; y: number }, params?: Record<string, unknown>) => string;
  setParam: (nodeId: string, key: string, value: unknown) => void;
  removeNodes: (ids: string[]) => void;
  load: (trees: Record<string, Tree>, layer: string | null) => void;
  allTrees: () => Record<string, Tree>;
}

/** A new layer's tree: its Layer Input connected straight to its Output. */
export function defaultTree(layer: string): Tree {
  const inId = `in-${layer}`;
  const outId = `out-${layer}`;
  return {
    nodes: [
      { id: inId, type: "data", position: { x: 0, y: 40 }, deletable: false, data: { tool: "input_layer", params: { layer }, fixed: true } },
      { id: outId, type: "output", position: { x: 560, y: 40 }, deletable: false, data: { tool: "layer_output", params: { layer }, fixed: true } },
    ],
    edges: [{ id: `e-${layer}`, source: inId, sourceHandle: "out", target: outId, targetHandle: "in" }],
  };
}

const strip = (t: Tree): Tree => ({
  nodes: t.nodes.map(({ id, type, position, data }) => ({ id, type, position, data, ...(data.fixed ? { deletable: false } : {}) })),
  edges: t.edges.map(({ id, source, target, sourceHandle, targetHandle }) => ({ id, source, target, sourceHandle, targetHandle })),
});

let counter = 1;
const newId = () => `n${Date.now().toString(36)}${counter++}`;

export const useModel = create<ModelState>()(
  temporal(
    (set, get) => ({
      layer: null,
      trees: {},
      nodes: [],
      edges: [],
      openTree: (layer) => {
        const s = get();
        if (layer === s.layer) return;
        const trees = { ...s.trees };
        if (s.layer && useApp.getState().layers[s.layer]) trees[s.layer] = strip({ nodes: s.nodes, edges: s.edges });
        else if (s.layer) delete trees[s.layer];
        const next = layer ? (trees[layer] ?? defaultTree(layer)) : { nodes: [], edges: [] };
        set({ layer, trees, nodes: next.nodes, edges: next.edges });
        useModel.temporal.getState().clear(); // undo history is per tree
      },
      allTrees: () => {
        const s = get();
        return s.layer ? { ...s.trees, [s.layer]: strip({ nodes: s.nodes, edges: s.edges }) } : s.trees;
      },
      onNodesChange: (changes) => set({ nodes: applyNodeChanges(changes, get().nodes) }),
      onEdgesChange: (changes) => set({ edges: applyEdgeChanges(changes, get().edges) }),
      connect: (c) =>
        set({
          // One connection per input port: a new one replaces the old.
          edges: addEdge(
            { ...c, id: `e${newId()}`, animated: false },
            get().edges.filter((e) => !(e.target === c.target && e.targetHandle === c.targetHandle)),
          ),
        }),
      addNode: (tool, position, params) => {
        const spec = specs()[tool];
        const defaults: Record<string, unknown> = {};
        for (const p of spec?.params ?? []) if (p.default !== null) defaults[p.id] = p.default;
        const id = newId();
        const type = spec?.role === "data" ? "data" : spec?.role === "output" ? "output" : "tool";
        set({ nodes: [...get().nodes, { id, type, position, data: { tool, params: { ...defaults, ...params } } }] });
        useApp.getState().set({ dirty: true });
        return id;
      },
      setParam: (nodeId, key, value) => {
        set({
          nodes: get().nodes.map((n) => (n.id === nodeId ? { ...n, data: { ...n.data, params: { ...n.data.params, [key]: value } } } : n)),
        });
        useApp.getState().set({ dirty: true });
      },
      removeNodes: (all) => {
        const ids = all.filter((id) => !get().nodes.find((n) => n.id === id)?.data.fixed);
        set({
          nodes: get().nodes.filter((n) => !ids.includes(n.id)),
          edges: get().edges.filter((e) => !ids.includes(e.source) && !ids.includes(e.target)),
        });
      },
      load: (trees, layer) => {
        set({ trees, layer: null, nodes: [], edges: [] });
        get().openTree(layer);
      },
    }),
    {
      limit: 100,
      // Only graph structure and params are undoable, not selection/drag state.
      partialize: (s) => ({
        nodes: s.nodes.map(({ id, type, position, data }) => ({ id, type, position, data })),
        edges: s.edges.map(({ id, source, target, sourceHandle, targetHandle }) => ({ id, source, target, sourceHandle, targetHandle })),
      }),
      equality: (a, b) => JSON.stringify(a) === JSON.stringify(b),
    },
  ),
);

interface RunState {
  specs: Record<string, ToolSpec>;
  results: Record<string, NodeResult>;
  running: boolean;
  setSpecs: (s: ToolSpec[]) => void;
  setResult: (r: NodeResult) => void;
  set: (p: Partial<RunState>) => void;
}

export const useRun = create<RunState>((set) => ({
  specs: {},
  results: {},
  running: false,
  setSpecs: (list) => set({ specs: Object.fromEntries(list.map((s) => [s.id, s])) }),
  setResult: (r) => set((s) => ({ results: { ...s.results, [r.id]: r } })),
  set: (p) => set(p),
}));

export const specs = () => useRun.getState().specs;

export function graphPayload() {
  const { nodes, edges } = useModel.getState();
  const app = useApp.getState();
  return {
    nodes: nodes.map((n) => {
      // Other layers brought into this tree contribute what they show
      // (their own node-tree result, if any). The tree's own Layer Input
      // always reads the original data.
      if (n.data.tool === "input_layer" && !n.data.fixed && typeof n.data.params.layer === "string") {
        const shown = displayInfo(app, n.data.params.layer);
        return { id: n.id, tool: n.data.tool, params: { ...n.data.params, layer: shown?.id ?? n.data.params.layer } };
      }
      return { id: n.id, tool: n.data.tool, params: n.data.params };
    }),
    edges: edges.map((e) => ({ source: e.source, target: e.target, targetHandle: e.targetHandle ?? "" })),
  };
}

function upstream(nodeId: string, port?: string): string | undefined {
  const { edges } = useModel.getState();
  return edges.find((e) => e.target === nodeId && (!port || e.targetHandle === port))?.source;
}

/** Does a node output features (vector) or a plain table? Undefined = unknown yet. */
export function outputKind(nodeId: string, seen = new Set<string>()): "vector" | "table" | "raster" | undefined {
  if (seen.has(nodeId)) return undefined;
  seen.add(nodeId);
  const node = useModel.getState().nodes.find((n) => n.id === nodeId);
  if (!node) return undefined;
  const spec = specs()[node.data.tool];
  if (!spec) return undefined;
  if (spec.role === "data") {
    const layer = useApp.getState().layers[String(node.data.params.layer ?? "")];
    return layer ? (layer.geomType === "Table" ? "table" : layer.geomType === "Raster" ? "raster" : "vector") : undefined;
  }
  if (spec.output === "vector") return "vector";
  if (spec.output === "raster") return "raster";
  const first = spec.inputs[0];
  const src = first && upstream(nodeId, first.id);
  return src ? outputKind(src, seen) : undefined;
}

/** Best guess at the fields a node outputs, for field pickers before running. */
export function outputFields(nodeId: string, seen = new Set<string>()): Field[] {
  if (seen.has(nodeId)) return [];
  seen.add(nodeId);
  const node = useModel.getState().nodes.find((n) => n.id === nodeId);
  if (!node) return [];
  const p = node.data.params;
  const from = (port: string) => {
    const src = upstream(nodeId, port);
    return src ? outputFields(src, seen) : [];
  };
  const add = (fields: Field[], name: unknown, type = "") =>
    typeof name === "string" && name && !fields.some((f) => f.name === name) ? [...fields, { name, type }] : fields;
  switch (node.data.tool) {
    case "input_layer":
      return useApp.getState().layers[String(p.layer ?? "")]?.fields ?? [];
    case "calculate_field":
    case "calculate_geometry":
      return add(from("in"), p.field);
    case "dissolve":
      return add(p.field ? from("in").filter((f) => f.name === p.field) : [], "COUNT", "BIGINT");
    case "buffer":
      return p.dissolve ? [] : from("in");
    case "sample_raster":
      return add(from("in"), p.field || "VALUE", "DOUBLE");
    case "zonal_statistics": {
      const pre = String(p.prefix ?? "Z_");
      return ["COUNT", "MEAN", "MIN", "MAX"].reduce((f, s) => add(f, `${pre}${s}`, "DOUBLE"), from("in"));
    }
    case "extract_vertices":
      return add(from("in"), "VERTEX_INDEX", "BIGINT");
    case "points_along_lines":
      return add(from("in"), "DISTANCE", "DOUBLE");
    case "join_nearest":
      return add([...from("in"), ...from("near")], "DISTANCE", "DOUBLE");
    case "near":
      return add(add(from("in"), "NEAR_DIST", "DOUBLE"), "NEAR_ROW", "BIGINT");
    case "summarize_within":
      return add(from("polys"), "COUNT", "BIGINT");
    case "polygonize":
      return [{ name: "DN", type: "DOUBLE" }];
    case "fishnet":
      return [{ name: "CELL_ID", type: "BIGINT" }, { name: "COL", type: "BIGINT" }, { name: "ROW", type: "BIGINT" }];
    case "contours":
      return [{ name: "ID", type: "INTEGER" }, { name: "ELEV", type: "DOUBLE" }, ...(Number(p.index_every) > 0 ? [{ name: "INDEX", type: "INTEGER" }] : [])];
    case "intersect":
      return [...from("a"), ...from("b")];
    case "spatial_join":
    case "attribute_join":
      return [...from("target"), ...from("join")];
    default: {
      const spec = specs()[node.data.tool];
      return spec?.inputs[0] ? from(spec.inputs[0].id) : [];
    }
  }
}

export function fieldsForPort(nodeId: string, port: string): Field[] {
  const src = upstream(nodeId, port);
  return src ? outputFields(src) : [];
}

/** The Contents layer that ultimately feeds a node's input port (for value suggestions). */
export function sourceLayerFor(nodeId: string, port: string, seen = new Set<string>()): string | undefined {
  const src = upstream(nodeId, port);
  if (!src || seen.has(src)) return undefined;
  seen.add(src);
  const node = useModel.getState().nodes.find((n) => n.id === src);
  if (!node) return undefined;
  if (node.data.tool === "input_layer") return String(node.data.params.layer ?? "") || undefined;
  const first = specs()[node.data.tool]?.inputs[0];
  return first ? sourceLayerFor(src, first.id, seen) : undefined;
}
