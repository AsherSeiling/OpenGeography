// Model (node graph) state. Graph edits are undoable; run results are not.
import { addEdge, applyEdgeChanges, applyNodeChanges, type Connection, type Edge, type EdgeChange, type Node, type NodeChange } from "@xyflow/react";
import { create } from "zustand";
import { temporal } from "zundo";
import type { Field, NodeResult, ToolSpec } from "../ipc/api";
import { useApp } from "./store";

export interface ModelNodeData extends Record<string, unknown> {
  tool: string;
  params: Record<string, unknown>;
}

export type ModelNode = Node<ModelNodeData>;

interface ModelState {
  nodes: ModelNode[];
  edges: Edge[];
  onNodesChange: (c: NodeChange<ModelNode>[]) => void;
  onEdgesChange: (c: EdgeChange[]) => void;
  connect: (c: Connection) => void;
  addNode: (tool: string, position: { x: number; y: number }, params?: Record<string, unknown>) => string;
  setParam: (nodeId: string, key: string, value: unknown) => void;
  removeNodes: (ids: string[]) => void;
  load: (nodes: ModelNode[], edges: Edge[]) => void;
}

let counter = 1;
const newId = () => `n${Date.now().toString(36)}${counter++}`;

export const useModel = create<ModelState>()(
  temporal(
    (set, get) => ({
      nodes: [],
      edges: [],
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
      removeNodes: (ids) =>
        set({
          nodes: get().nodes.filter((n) => !ids.includes(n.id)),
          edges: get().edges.filter((e) => !ids.includes(e.source) && !ids.includes(e.target)),
        }),
      load: (nodes, edges) => set({ nodes, edges }),
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
  return {
    nodes: nodes.map((n) => ({ id: n.id, tool: n.data.tool, params: n.data.params })),
    edges: edges.map((e) => ({ source: e.source, target: e.target, targetHandle: e.targetHandle ?? "" })),
  };
}

function upstream(nodeId: string, port?: string): string | undefined {
  const { edges } = useModel.getState();
  return edges.find((e) => e.target === nodeId && (!port || e.targetHandle === port))?.source;
}

/** Does a node output features (vector) or a plain table? Undefined = unknown yet. */
export function outputKind(nodeId: string, seen = new Set<string>()): "vector" | "table" | undefined {
  if (seen.has(nodeId)) return undefined;
  seen.add(nodeId);
  const node = useModel.getState().nodes.find((n) => n.id === nodeId);
  if (!node) return undefined;
  const spec = specs()[node.data.tool];
  if (!spec) return undefined;
  if (spec.role === "data") {
    const layer = useApp.getState().layers[String(node.data.params.layer ?? "")];
    return layer ? (layer.geomType === "Table" ? "table" : "vector") : undefined;
  }
  if (spec.output === "vector") return "vector";
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
