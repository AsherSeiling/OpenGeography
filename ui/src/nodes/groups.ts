// UX8: reusable node groups. A group is part of a tree saved by name (engine `node_groups.rs` keeps the library in the user's data
// folder and checks every file). Inserting one copies its nodes into the open tree as ordinary, editable nodes, as one undo step.
import type { Edge } from "@xyflow/react";
import { api, type NodeGroup, type NodeGroupNode } from "../ipc/api";
import { asOneStep, specs, useModel, type ModelNode } from "../store/model";

/** The group made from `nodes` and the connections between them. Layer References are left out (they mean nothing in another project). */
export function groupFromNodes(name: string, description: string, nodes: ModelNode[], edges: Edge[]): NodeGroup {
  const picked = nodes.filter((n) => !n.data.fixed);
  const ids = new Set(picked.map((n) => n.id));
  const minX = Math.min(...picked.map((n) => n.position.x));
  const minY = Math.min(...picked.map((n) => n.position.y));
  const layerParams = (tool: string) => new Set((specs()[tool]?.params ?? []).filter((p) => p.kind === "layer").map((p) => p.id));
  return {
    format: "gis-studio-node-group",
    version: 1,
    name: name.trim(),
    description: description.trim(),
    nodes: picked.map((n) => {
      const drop = layerParams(n.data.tool);
      if (n.data.tool === "input_layer") drop.add("layer");
      const params = Object.fromEntries(Object.entries(structuredClone(n.data.params)).filter(([k]) => !drop.has(k)));
      return { id: n.id, tool: n.data.tool, params, x: n.position.x - minX, y: n.position.y - minY };
    }),
    edges: edges
      .filter((e) => ids.has(e.source) && ids.has(e.target))
      .map((e) => ({ source: e.source, target: e.target, sourceHandle: e.sourceHandle ?? "out", targetHandle: e.targetHandle ?? "" })),
  };
}

/** Why the selection can't be saved as a group, or null. */
export function groupProblem(nodes: ModelNode[]): string | null {
  const picked = nodes.filter((n) => n.selected && !n.data.fixed);
  if (!picked.length) return "Select the nodes to save first. The tree's own Layer Input and Output can't be part of a group.";
  return null;
}

/** Input ports of the group's nodes that no connection inside the group feeds: what the user connects after inserting it. */
export function openInputs(group: NodeGroup): { node: string; port: string; label: string }[] {
  const out: { node: string; port: string; label: string }[] = [];
  for (const n of group.nodes) {
    for (const p of specs()[n.tool]?.inputs ?? []) {
      if (!group.edges.some((e) => e.target === n.id && e.targetHandle === p.id)) out.push({ node: n.id, port: p.id, label: p.label });
    }
  }
  return out;
}

/** Copy a group into the open tree with its top-left at `at` (flow coordinates). One undo step; the new nodes are selected. Returns the new ids. */
export function insertGroup(group: NodeGroup, at: { x: number; y: number }): string[] {
  const owner = useModel.getState().layer;
  if (!owner) return [];
  return asOneStep(() => {
    const map = new Map(group.nodes.map((n, i) => [n.id, `n${Date.now().toString(36)}g${i}${Math.random().toString(36).slice(2, 6)}`]));
    const fresh: ModelNode[] = group.nodes.map((n: NodeGroupNode) => {
      const spec = specs()[n.tool];
      const defaults: Record<string, unknown> = {};
      for (const p of spec?.params ?? []) if (p.default !== null) defaults[p.id] = p.default;
      const type = spec?.role === "data" ? "data" : spec?.role === "output" ? "output" : "tool";
      return { id: map.get(n.id)!, type, selected: true, position: { x: at.x + n.x, y: at.y + n.y }, data: { tool: n.tool, params: { ...defaults, ...structuredClone(n.params) } } };
    });
    const edges: Edge[] = group.edges.map((e, i) => ({ id: `e${map.get(e.source)}-${i}`, source: map.get(e.source)!, target: map.get(e.target)!, sourceHandle: e.sourceHandle ?? "out", targetHandle: e.targetHandle }));
    const s = useModel.getState();
    useModel.setState({ nodes: [...s.nodes.map((n) => ({ ...n, selected: false })), ...fresh], edges: [...s.edges, ...edges] });
    return fresh.map((n) => n.id);
  });
}

export async function saveSelectionAsGroup(name: string, description: string): Promise<NodeGroup> {
  const { nodes, edges } = useModel.getState();
  const problem = groupProblem(nodes);
  if (problem) throw new Error(problem);
  return api.nodeGroupSave(groupFromNodes(name, description, nodes.filter((n) => n.selected), edges));
}
