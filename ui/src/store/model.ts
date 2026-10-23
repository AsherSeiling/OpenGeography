// Model (node graph) state. Graph edits are undoable; run results are not.
import { addEdge, applyEdgeChanges, applyNodeChanges, type Connection, type Edge, type EdgeChange, type Node, type NodeChange } from "@xyflow/react";
import { create } from "zustand";
import { temporal } from "zundo";
import type { Field, NodeResult, PortKind, ToolSpec } from "../ipc/api";
import { displayInfo, useApp } from "./store";

export interface ModelNodeData extends Record<string, unknown> {
  tool: string;
  params: Record<string, unknown>;
  /** The tree's own Layer Input / Output nodes: always present, not deletable. */
  fixed?: boolean;
  /** Bypassed (Blender "mute"): its first input passes straight through. */
  muted?: boolean;
  /** Settings hidden (Blender "hide"). */
  collapsed?: boolean;
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
        groupParamEdit(`${nodeId}:${key}`);
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

const snapshot = () => {
  const s = useModel.getState();
  return {
    nodes: s.nodes.map(({ id, type, position, data }) => ({ id, type, position, data })),
    edges: s.edges.map(({ id, source, target, sourceHandle, targetHandle }) => ({ id, source, target, sourceHandle, targetHandle })),
  };
};

/**
 * Typing in a node setting edits it once per keystroke; a burst of edits to
 * the same setting is one undo step, ending after a pause or when anything
 * else starts its own step.
 */
let paramBurst: { key: string; end: () => void; timer: ReturnType<typeof setTimeout> } | null = null;

export function flushParamEdit() {
  const b = paramBurst;
  if (!b) return;
  paramBurst = null;
  clearTimeout(b.timer);
  b.end();
}

function groupParamEdit(key: string) {
  if (paramBurst?.key !== key) {
    flushParamEdit();
    const end = beginStep();
    paramBurst = { key, end, timer: setTimeout(flushParamEdit, 800) };
  } else {
    clearTimeout(paramBurst.timer);
    paramBurst.timer = setTimeout(flushParamEdit, 800);
  }
}

/**
 * Group several edits (possibly across async steps) into ONE undo step.
 * Call the returned function when done; nothing is recorded if nothing changed.
 */
export function beginStep(): () => void {
  if (paramBurst) flushParamEdit(); // a typing burst ends before anything else
  const before = snapshot();
  const t = useModel.temporal.getState();
  t.pause();
  return () => {
    useModel.temporal.getState().resume();
    if (JSON.stringify(before) === JSON.stringify(snapshot())) return;
    useModel.temporal.setState((s) => ({ pastStates: [...s.pastStates, before as (typeof s.pastStates)[number]].slice(-100), futureStates: [] }));
    useApp.getState().set({ dirty: true });
  };
}

/** Run synchronous edits as one undo step. */
export function asOneStep<T>(f: () => T): T {
  const end = beginStep();
  try {
    return f();
  } finally {
    end();
  }
}

interface RunState {
  specs: Record<string, ToolSpec>;
  results: Record<string, NodeResult>;
  /** Signature of each node when it last ran successfully (see nodeSignature). */
  ranSig: Record<string, string>;
  running: boolean;
  setSpecs: (s: ToolSpec[]) => void;
  setResult: (r: NodeResult) => void;
  set: (p: Partial<RunState>) => void;
}

export const useRun = create<RunState>((set) => ({
  specs: {},
  results: {},
  ranSig: {},
  running: false,
  setSpecs: (list) => set({ specs: Object.fromEntries(list.map((s) => [s.id, s])) }),
  setResult: (r) => set((s) => ({ results: { ...s.results, [r.id]: r } })),
  set: (p) => set(p),
}));

export const specs = () => useRun.getState().specs;

export function graphPayload() {
  const { nodes, edges } = bypassMuted(useModel.getState().nodes, useModel.getState().edges);
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

/**
 * Remove muted nodes, wiring whatever fed a muted node's first input straight
 * to everything it fed (Blender's mute). Fixed nodes can't be muted.
 */
export function bypassMuted(nodes: ModelNode[], edges: Edge[]): { nodes: ModelNode[]; edges: Edge[] } {
  let ns = nodes;
  let es = edges;
  for (const m of nodes.filter((n) => n.data.muted && !n.data.fixed)) {
    const first = specs()[m.data.tool]?.inputs[0]?.id;
    const feed = es.find((e) => e.target === m.id && e.targetHandle === first);
    es = es.flatMap((e) => {
      if (e.target === m.id) return [];
      if (e.source === m.id) return feed ? [{ ...e, id: `${e.id}~`, source: feed.source, sourceHandle: feed.sourceHandle }] : [];
      return [e];
    });
    ns = ns.filter((n) => n.id !== m.id);
  }
  return { nodes: ns, edges: es };
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

/** Can data of `kind` (undefined = not known yet) go into a port of `port` kind? */
export function accepts(port: PortKind, kind: "vector" | "table" | "raster" | undefined): boolean {
  if (port === "any") return true;
  if (port === "raster") return kind === "raster" || kind === undefined;
  if (kind === "raster") return false;
  return port === "table" || kind !== "table";
}

/** What a node of `tool` outputs when fed `input` on its first port. */
export function kindAfter(tool: string, input: "vector" | "table" | "raster" | undefined): "vector" | "table" | "raster" | undefined {
  const spec = specs()[tool];
  if (!spec) return undefined;
  if (spec.output === "vector" || spec.output === "raster" || spec.output === "table") return spec.output;
  return input;
}

/** Why `source` can't feed `target`'s `port`, or null if it can. */
export function connectProblem(source: string, target: string, port: string | null | undefined, sourceKind = outputKind(source)): string | null {
  if (source === target) return "A node can't feed itself";
  const t = useModel.getState().nodes.find((n) => n.id === target);
  const spec = t && specs()[t.data.tool];
  const p = spec?.inputs.find((x) => x.id === port);
  if (!p) return "That node has no such input";
  if (accepts(p.kind, sourceKind)) return null;
  const what = { vector: "features", table: "a table", raster: "a raster" }[sourceKind ?? "vector"];
  const needs = { vector: "features", table: "a table or features", raster: "a raster", any: "data" }[p.kind];
  return `${spec!.name}'s "${p.label}" needs ${needs}, not ${what}`;
}

/**
 * A node's identity for change tracking: its tool, settings, bypass state,
 * its source layer's version, and (recursively) everything upstream.
 */
export function nodeSignature(id: string, memo = new Map<string, string>(), seen = new Set<string>()): string {
  if (memo.has(id)) return memo.get(id)!;
  if (seen.has(id)) return "cycle";
  seen.add(id);
  const { nodes, edges } = useModel.getState();
  const n = nodes.find((x) => x.id === id);
  if (!n) return "missing";
  const layer = n.data.tool === "input_layer" ? useApp.getState().layers[String(n.data.params.layer ?? "")] : undefined;
  const ins = edges
    .filter((e) => e.target === id)
    .map((e) => `${e.targetHandle}<${nodeSignature(e.source, memo, seen)}`)
    .sort();
  const sig = JSON.stringify([n.data.tool, n.data.params, !!n.data.muted, layer ? `${layer.id}@${layer.version}` : null, ins]);
  memo.set(id, sig);
  return sig;
}

/**
 * The fields a node outputs, for field pickers before running — evaluated
 * from the tool's declared rule (engine `Tool::output_fields`), so there is
 * no second copy of each tool's behavior here.
 */
export function outputFields(nodeId: string, seen = new Set<string>()): Field[] {
  if (seen.has(nodeId)) return [];
  seen.add(nodeId);
  const node = useModel.getState().nodes.find((n) => n.id === nodeId);
  const spec = node && specs()[node.data.tool];
  if (!node || !spec) return [];
  const p = node.data.params;
  const rule = spec.fields;
  if (!rule) return spec.inputs[0] ? fieldsFrom(nodeId, spec.inputs[0].id, seen) : [];
  if (rule.layerParam) return useApp.getState().layers[String(p[rule.layerParam] ?? "")]?.fields ?? [];
  if (rule.noneIf && p[rule.noneIf] === true) return [];
  let out: Field[] = (rule.from ?? (spec.inputs[0] ? [spec.inputs[0].id] : [])).flatMap((port) => fieldsFrom(nodeId, port, seen));
  const keep = rule.keepParam;
  if (keep) out = p[keep] ? out.filter((f) => f.name === p[keep]) : [];
  for (const f of rule.add) {
    if (f.whenPositive && !(Number(p[f.whenPositive]) > 0)) continue;
    const base = (f.nameParam && typeof p[f.nameParam] === "string" && p[f.nameParam]) || f.name;
    const name = f.prefixParam ? `${p[f.prefixParam[0]] ?? f.prefixParam[1]}${base}` : base;
    if (name && !out.some((x) => x.name === name)) out = [...out, { name: String(name), type: f.type }];
  }
  return out;
}

function fieldsFrom(nodeId: string, port: string, seen: Set<string>): Field[] {
  const src = upstream(nodeId, port);
  return src ? outputFields(src, seen) : [];
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
