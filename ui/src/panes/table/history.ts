// Attribute history: pure helpers for the table pane's node mode and diff mode
// (docs/ATTRIBUTE_HISTORY_PLAN.md), plus the small store that says which
// node's table the pane shows.
import type { Edge } from "@xyflow/react";
import { create } from "zustand";
import type { NodeResult, ToolSpec } from "../../ipc/api";
import type { ModelNode, Tree } from "../../store/model";

type SpecMap = Record<string, Pick<ToolSpec, "name" | "inputs" | "output" | "role">>;

export type PickerState = "ready" | "outofdate" | "norun";

export interface PickerEntry {
  id: string;
  label: string;
  /** The node's snapshot table, if it has one. */
  table: string | null;
  state: PickerState;
  disabled: boolean;
  /** Why it can't be picked. */
  reason?: string;
}

/** The snapshot table of a node: this session's result, else the one saved in the project. */
export function nodeTable(saved: string | undefined, result: NodeResult | undefined): string | null {
  const live = result?.status === "done" || result?.status === "cached" ? result.output : null;
  return live ?? saved ?? null;
}

/** Node ids in topological order (sources first); ties keep the tree's own order. */
export function topoOrder(nodes: ModelNode[], edges: Edge[]): string[] {
  const indeg = new Map(nodes.map((n) => [n.id, 0]));
  for (const e of edges) if (indeg.has(e.target) && indeg.has(e.source)) indeg.set(e.target, indeg.get(e.target)! + 1);
  const out: string[] = [];
  const queue = nodes.filter((n) => indeg.get(n.id) === 0).map((n) => n.id);
  while (queue.length) {
    const id = queue.shift()!;
    out.push(id);
    for (const e of edges) {
      if (e.source !== id || !indeg.has(e.target)) continue;
      indeg.set(e.target, indeg.get(e.target)! - 1);
      if (indeg.get(e.target) === 0) queue.push(e.target);
    }
  }
  // A cycle can't run, but keep its nodes visible.
  for (const n of nodes) if (!out.includes(n.id)) out.push(n.id);
  return out;
}

/**
 * The nodes of a tree a table can be shown for, in topological order.
 * `changed` = ids edited since they last ran (their table is out of date); `saved` = the project's lastOutputs for this tree.
 */
export function pickerEntries(tree: Tree, results: Record<string, NodeResult>, specs: SpecMap, changed: ReadonlySet<string> = new Set(), saved: Record<string, string> = {}): PickerEntry[] {
  const byId = new Map(tree.nodes.map((n) => [n.id, n]));
  return topoOrder(tree.nodes, tree.edges).flatMap((id) => {
    const node = byId.get(id)!;
    const spec = specs[node.data.tool];
    if (!spec) return [];
    const r = results[id];
    const table = nodeTable(saved[id], r);
    const label = node.data.fixed && node.data.tool === "input_layer" ? "Layer Input" : node.data.fixed ? "Output" : spec.name;
    let state: PickerState = !table ? "norun" : changed.has(id) ? "outofdate" : "ready";
    let reason: string | undefined;
    if (node.data.muted) reason = "Muted: this node is bypassed, so it has no table of its own";
    else if (spec.output === "raster") reason = "Rasters have no attribute table";
    else if (!spec.output && spec.role !== "output") reason = "This node produces no table";
    if (reason) state = "norun";
    return [{ id, label, table: reason ? null : table, state, disabled: !!reason, reason }];
  });
}

/** The node a diff compares against by default: whatever feeds the node's first input (muted nodes are skipped). */
export function defaultCompareInput(tree: Tree, nodeId: string, specs: SpecMap): string | null {
  const byId = new Map(tree.nodes.map((n) => [n.id, n]));
  const first = (id: string) => {
    const n = byId.get(id);
    const port = n && specs[n.data.tool]?.inputs[0]?.id;
    return port ? (tree.edges.find((e) => e.target === id && e.targetHandle === port)?.source ?? null) : null;
  };
  const seen = new Set<string>();
  let src = first(nodeId);
  while (src && byId.get(src)?.data.muted && !seen.has(src)) {
    seen.add(src);
    src = first(src);
  }
  return src;
}

/** Every snapshot table the project's trees still point at. */
export function liveTables(lastOutputs: Record<string, Record<string, string>>): string[] {
  const out = new Set<string>();
  for (const t of Object.values(lastOutputs)) for (const table of Object.values(t)) if (table) out.add(table);
  return [...out];
}

/** Tell the engine which snapshots are still referenced (on save and after a tree is deleted). */
export async function pruneNodeCache(
  lastOutputs: Record<string, Record<string, string>>,
  pinned: string | null,
  api: { pruneNodeCache: (live: string[], pinned?: string | null) => Promise<number> },
): Promise<number> {
  try {
    return await api.pruneNodeCache(liveTables(lastOutputs), pinned);
  } catch {
    return 0; // housekeeping only; never block a save
  }
}

/** Which node's table the pane shows, and what it is compared with. */
export interface TableTarget {
  /** Layer whose tree the node is in. */
  tree: string;
  nodeId: string;
  /** Compare with this node (null = no diff). */
  compareId: string | null;
}

interface TableTargetState {
  target: TableTarget | null;
  set: (t: TableTarget | null) => void;
}

export const useTableTarget = create<TableTargetState>((set) => ({ target: null, set: (target) => set({ target }) }));

/** True when the engine says a diff table was lost (project reopened or first saved). */
export const isDiffGone = (message: string) => /no longer available/i.test(message);
export const isNotStored = (message: string) => /is not stored/i.test(message);

/** Row count a diff view shows for the active chip ("all" = no chip active). */
export function diffTotal(s: { added: number; removed: number; changed: number }, kind: "all" | "added" | "removed" | "changed"): number {
  return kind === "all" ? s.added + s.removed + s.changed : s[kind];
}
