// Node-editor operations behind the Blender-style shortcuts. Each is one undo
// step; the tree's fixed Layer Input / Output are never deleted or duplicated.
import type { Edge } from "@xyflow/react";
import { asOneStep, connectProblem, specs, useModel, type ModelNode } from "../store/model";
import { useApp } from "../store/store";

const edgeId = () => `e${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`;
const nodeId = () => `n${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`;
const selected = () => useModel.getState().nodes.filter((n) => n.selected);

/** Delete selected nodes and connections. With `reconnect`, whatever fed a
 * deleted node's first input is wired to everything it fed, where compatible. */
export function deleteSelection(reconnect = false) {
  asOneStep(() => {
    const { nodes, edges } = useModel.getState();
    const doomed = new Set(nodes.filter((n) => n.selected && !n.data.fixed).map((n) => n.id));
    let kept = edges.filter((e) => !e.selected && !doomed.has(e.source) && !doomed.has(e.target));
    if (reconnect) {
      for (const id of doomed) {
        const n = nodes.find((x) => x.id === id)!;
        const first = specs()[n.data.tool]?.inputs[0]?.id;
        // Follow chains of deleted nodes back to a surviving source.
        let feed = edges.find((e) => e.target === id && e.targetHandle === first);
        const seen = new Set<string>();
        while (feed && doomed.has(feed.source) && !seen.has(feed.source)) {
          seen.add(feed.source);
          const up = nodes.find((x) => x.id === feed!.source)!;
          const upFirst = specs()[up.data.tool]?.inputs[0]?.id;
          feed = edges.find((e) => e.target === up.id && e.targetHandle === upFirst);
        }
        if (!feed) continue;
        for (const out of edges.filter((e) => e.source === id && !doomed.has(e.target))) {
          const taken = kept.some((e) => e.target === out.target && e.targetHandle === out.targetHandle);
          if (!taken && !connectProblem(feed.source, out.target, out.targetHandle)) {
            kept = [...kept, { id: edgeId(), source: feed.source, sourceHandle: feed.sourceHandle ?? "out", target: out.target, targetHandle: out.targetHandle }];
          }
        }
      }
    }
    useModel.setState({ nodes: nodes.filter((n) => !doomed.has(n.id)), edges: kept });
  });
}

interface Clip {
  nodes: ModelNode[];
  edges: Edge[];
}
let clipboard: Clip | null = null;

/** Copy the selected (non-fixed) nodes and the connections between them. */
export function copySelection(): number {
  const ns = selected().filter((n) => !n.data.fixed);
  const ids = new Set(ns.map((n) => n.id));
  clipboard = {
    nodes: ns.map((n) => ({ id: n.id, type: n.type, position: { ...n.position }, data: structuredClone(n.data) })),
    edges: useModel.getState().edges.filter((e) => ids.has(e.source) && ids.has(e.target)).map((e) => ({ ...e })),
  };
  return ns.length;
}

export const hasClipboard = () => !!clipboard?.nodes.length;

/** Paste copied nodes with their top-left at `at` (flow coordinates); they become the selection. */
export function paste(at: { x: number; y: number }): string[] {
  if (!clipboard?.nodes.length) return [];
  const clip = clipboard;
  return asOneStep(() => {
    const minX = Math.min(...clip.nodes.map((n) => n.position.x));
    const minY = Math.min(...clip.nodes.map((n) => n.position.y));
    const map = new Map(clip.nodes.map((n) => [n.id, nodeId()]));
    const owner = useModel.getState().layer;
    const fresh: ModelNode[] = clip.nodes.map((n) => ({
      ...n,
      id: map.get(n.id)!,
      selected: true,
      position: { x: at.x + n.position.x - minX, y: at.y + n.position.y - minY },
      // A Layer node pointing at this tree's own layer would duplicate its Input: clear it.
      data: n.data.tool === "input_layer" && n.data.params.layer === owner ? { ...structuredClone(n.data), params: { ...n.data.params, layer: undefined } } : structuredClone(n.data),
    }));
    const edges = clip.edges.map((e) => ({ ...e, id: edgeId(), source: map.get(e.source)!, target: map.get(e.target)!, selected: false }));
    const s = useModel.getState();
    useModel.setState({ nodes: [...s.nodes.map((n) => ({ ...n, selected: false })), ...fresh], edges: [...s.edges, ...edges] });
    return fresh.map((n) => n.id);
  });
}

/** Duplicate the selection next to itself; returns the new node ids (selected). */
export function duplicateSelection(): string[] {
  const ns = selected().filter((n) => !n.data.fixed);
  if (!ns.length) return [];
  const saved = clipboard;
  copySelection();
  const at = { x: Math.min(...ns.map((n) => n.position.x)) + 40, y: Math.min(...ns.map((n) => n.position.y)) + 40 };
  const ids = paste(at);
  clipboard = saved; // duplicating doesn't replace what the user copied
  return ids;
}

export function selectAll(on: boolean) {
  useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: on })), edges: useModel.getState().edges.map((e) => ({ ...e, selected: false })) });
}

export function invertSelection() {
  useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: !n.selected })) });
}

/** Toggle a data flag on the selected nodes (all on if any is off). */
function toggle(flag: "muted" | "collapsed", skipFixed: boolean) {
  const ns = selected().filter((n) => !(skipFixed && n.data.fixed));
  if (!ns.length) return;
  const on = ns.some((n) => !n.data[flag]);
  const ids = new Set(ns.map((n) => n.id));
  asOneStep(() => useModel.setState({ nodes: useModel.getState().nodes.map((n) => (ids.has(n.id) ? { ...n, data: { ...n.data, [flag]: on } } : n)) }));
}

/** Bypass the selected nodes (not the fixed Input/Output). */
export const toggleMute = () => toggle("muted", true);
export const toggleCollapse = () => toggle("collapsed", false);

/**
 * Blender's Make Links: connect the selected nodes left to right, each one's
 * output into the next one's first free compatible input. Returns how many
 * links were made.
 */
export function makeLinks(): number {
  const ns = [...selected()].sort((a, b) => a.position.x - b.position.x);
  let made = 0;
  asOneStep(() => {
    for (let i = 0; i + 1 < ns.length; i++) {
      const [a, b] = [ns[i], ns[i + 1]];
      if (!specs()[a.data.tool]?.output) continue;
      const edges = useModel.getState().edges;
      if (edges.some((e) => e.source === a.id && e.target === b.id)) continue;
      const port = specs()[b.data.tool]?.inputs.find((p) => !edges.some((e) => e.target === b.id && e.targetHandle === p.id) && !connectProblem(a.id, b.id, p.id));
      if (!port) continue;
      useModel.setState({ edges: [...edges, { id: edgeId(), source: a.id, sourceHandle: "out", target: b.id, targetHandle: port.id }] });
      made++;
    }
  });
  if (!made && ns.length > 1) useApp.getState().setStatus("Make Links: no free, compatible inputs between the selected nodes");
  return made;
}

/** Remove the connections whose paths cross the polyline `cut` (screen points). */
export function cutLinks(cut: { x: number; y: number }[], pathOf: (edgeId: string) => SVGPathElement | null, toScreen: (p: DOMPoint, el: SVGPathElement) => { x: number; y: number }): number {
  if (cut.length < 2) return 0;
  const crosses = (p: { x: number; y: number }, q: { x: number; y: number }, r: { x: number; y: number }, s: { x: number; y: number }) => {
    const d = (q.x - p.x) * (s.y - r.y) - (q.y - p.y) * (s.x - r.x);
    if (d === 0) return false;
    const t = ((r.x - p.x) * (s.y - r.y) - (r.y - p.y) * (s.x - r.x)) / d;
    const u = ((r.x - p.x) * (q.y - p.y) - (r.y - p.y) * (q.x - p.x)) / d;
    return t >= 0 && t <= 1 && u >= 0 && u <= 1;
  };
  const hit = new Set<string>();
  for (const e of useModel.getState().edges) {
    const path = pathOf(e.id);
    if (!path) continue;
    const len = path.getTotalLength();
    const steps = Math.max(8, Math.ceil(len / 12));
    const pts = Array.from({ length: steps + 1 }, (_, i) => toScreen(path.getPointAtLength((len * i) / steps), path));
    outer: for (let i = 0; i + 1 < pts.length; i++) {
      for (let j = 0; j + 1 < cut.length; j++) {
        if (crosses(pts[i], pts[i + 1], cut[j], cut[j + 1])) {
          hit.add(e.id);
          break outer;
        }
      }
    }
  }
  if (hit.size) asOneStep(() => useModel.setState({ edges: useModel.getState().edges.filter((e) => !hit.has(e.id)) }));
  return hit.size;
}
