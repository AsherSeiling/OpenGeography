// Drop a node onto a connection to insert it there (Blender behavior):
// A → B becomes A → node → B.
import type { Edge, InternalNode, Node } from "@xyflow/react";
import { outputKind, specs, useModel, type ModelNode } from "../store/model";

type Box = { x: number; y: number; w: number; h: number };

function box(n: InternalNode<Node> | undefined): Box | null {
  if (!n) return null;
  const p = n.internals.positionAbsolute;
  return { x: p.x, y: p.y, w: n.measured.width ?? 200, h: n.measured.height ?? 80 };
}

/** The connection that a free (unconnected) node is sitting on, if any. */
export function edgeUnder(nodeId: string, getNode: (id: string) => InternalNode<Node> | undefined): Edge | null {
  const { nodes, edges } = useModel.getState();
  const node = nodes.find((n) => n.id === nodeId);
  if (!node || node.data.fixed) return null;
  const spec = specs()[node.data.tool];
  if (!spec?.output || !spec.inputs.length) return null;
  if (edges.some((e) => e.source === nodeId || e.target === nodeId)) return null; // only free nodes
  const b = box(getNode(nodeId));
  if (!b) return null;
  const inside = (x: number, y: number) => x >= b.x - 8 && x <= b.x + b.w + 8 && y >= b.y - 8 && y <= b.y + b.h + 8;
  for (const e of edges) {
    const s = box(getNode(e.source));
    const t = box(getNode(e.target));
    if (!s || !t) continue;
    // Straight line from the source's right edge to the target's left edge.
    const [x0, y0, x1, y1] = [s.x + s.w, s.y + 30, t.x, t.y + t.h / 2];
    for (let i = 1; i < 24; i++) {
      const f = i / 24;
      if (inside(x0 + (x1 - x0) * f, y0 + (y1 - y0) * f)) {
        if (spec.inputs[0].kind === "vector" && outputKind(e.source) === "table") return null;
        return e;
      }
    }
  }
  return null;
}

/** Insert `nodeId` into connection `e`. */
export function splice(nodeId: string, e: Edge) {
  const { nodes, edges } = useModel.getState();
  const node = nodes.find((n) => n.id === nodeId) as ModelNode;
  const firstIn = specs()[node.data.tool].inputs[0].id;
  const rest = edges.filter((x) => x.id !== e.id);
  const id = () => `e${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  useModel.setState({
    edges: [
      ...rest,
      { id: id(), source: e.source, sourceHandle: e.sourceHandle ?? "out", target: nodeId, targetHandle: firstIn },
      { id: id(), source: nodeId, sourceHandle: "out", target: e.target, targetHandle: e.targetHandle },
    ],
  });
}
