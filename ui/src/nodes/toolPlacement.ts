// Where a tool picked in the Shift+A menu goes, and how it connects (plan §5, rules 1 to 4):
//   1. the pointer was on a connection: splice into it
//   2. a node is selected and its output suits the tool's first input: place to its right and connect; if it fed something, splice between
//   3. the tree is only Layer Input → Output: splice between them
//   4. otherwise at the pointer (or the middle of the canvas), unconnected
// A connection that `connectProblem` or `spliceProblem` rejects is never made: the node is placed unconnected instead.
import type { Edge, InternalNode, Node } from "@xyflow/react";
import { accepts, beginStep, connectProblem, kindAfter, outputKind, specs, useModel, type ModelNode } from "../store/model";
import { useApp } from "../store/store";
import { splice } from "./splice";
import { useToolMenu, type MenuPos } from "./toolMenuStore";

/** What the Node Editor offers the menu; it registers itself while it is on screen. */
export interface EditorHandle {
  screenToFlow: (p: MenuPos) => MenuPos;
  getInternalNode: (id: string) => InternalNode<Node> | undefined;
  /** The canvas box, or null while its tab is hidden. */
  rect: () => DOMRect | null;
  /** A node is being moved with G: keys belong to that. */
  busy: () => boolean;
}

let editor: EditorHandle | null = null;
const waiting: (() => void)[] = [];

export function registerEditor(h: EditorHandle | null) {
  editor = h;
  if (h) while (waiting.length) waiting.shift()!();
}
export const editorHandle = () => editor;

/** The Node Editor is mounted with room to work in (its tab is active and not squashed). */
export function editorVisible(): boolean {
  const r = editor?.rect();
  return !!r && r.width > 80 && r.height > 60;
}

/** Resolves once the Node Editor is on screen, or false after `ms`. */
export function whenEditorVisible(ms = 2500): Promise<boolean> {
  if (editorVisible()) return Promise.resolve(true);
  return new Promise((resolve) => {
    const t0 = Date.now();
    const tick = () => (editorVisible() ? resolve(true) : Date.now() - t0 > ms ? resolve(false) : setTimeout(tick, 40));
    tick();
  });
}

export type Plan =
  | { rule: 1; edge: Edge }
  | { rule: 2; from: ModelNode; splice: Edge | null }
  | { rule: 3; edge: Edge }
  | { rule: 4 };

/** Would `tool`'s first input take the data of `kind`? */
const firstInputAccepts = (tool: string, kind: ReturnType<typeof outputKind>) => {
  const first = specs()[tool]?.inputs[0];
  return !!first && accepts(first.kind, kind);
};

/** Can `tool` be put into connection `e` (checked on both sides, like `spliceProblem`, without the node existing yet)? */
function fitsWire(tool: string, e: Edge): boolean {
  const spec = specs()[tool];
  if (!spec?.output || !spec.inputs.length) return false;
  const incoming = outputKind(e.source);
  if (!firstInputAccepts(tool, incoming)) return false;
  const target = useModel.getState().nodes.find((n) => n.id === e.target);
  const port = target && specs()[target.data.tool]?.inputs.find((p) => p.id === e.targetHandle);
  return !!port && accepts(port.kind, kindAfter(tool, incoming));
}

/** Which placement rule applies to `tool` right now. Used for the menu's preview line and by `placeTool`. */
export function planPlacement(tool: string, wire: string | null = useToolMenu.getState().wire): Plan {
  const { nodes, edges } = useModel.getState();
  const spec = specs()[tool];
  if (!spec || !spec.inputs.length) return { rule: 4 };
  const w = wire ? edges.find((e) => e.id === wire) : undefined;
  if (w && fitsWire(tool, w)) return { rule: 1, edge: w };
  const sel = nodes.filter((n) => n.selected);
  const from = sel.length === 1 ? sel[0] : undefined;
  const fromSpec = from && specs()[from.data.tool];
  if (from && fromSpec?.output && firstInputAccepts(tool, outputKind(from.id))) {
    const fed = edges.find((e) => e.source === from.id);
    return { rule: 2, from, splice: fed && fitsWire(tool, fed) ? fed : null };
  }
  if (nodes.length === 2 && edges.length === 1 && fitsWire(tool, edges[0])) return { rule: 3, edge: edges[0] };
  return { rule: 4 };
}

const nodeName = (n: ModelNode) => (n.data.tool === "input_layer" ? "Layer Input" : (specs()[n.data.tool]?.name ?? n.data.tool));

/** One line for the preview: where picking this tool will connect it. */
export function describePlacement(tool: string): string {
  const p = planPlacement(tool);
  const { nodes } = useModel.getState();
  const name = (id: string) => nodeName(nodes.find((n) => n.id === id)!);
  switch (p.rule) {
    case 1:
      return `Inserted into the connection under the pointer, between ${name(p.edge.source)} and ${name(p.edge.target)}.`;
    case 2:
      return p.splice ? `Connects after ${nodeName(p.from)} and before ${name(p.splice.target)}.` : `Connects after ${nodeName(p.from)}.`;
    case 3:
      return `Goes between ${name(p.edge.source)} and ${name(p.edge.target)}.`;
    default:
      return specs()[tool]?.inputs.length ? "Placed unconnected. Connect it yourself." : "Placed unconnected.";
  }
}

const CARD_W = 220;

function selectNode(id: string) {
  useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: n.id === id })) });
  useApp.getState().setProperties({ kind: "node", id });
}

/** Where to put a card: at the pointer, else near the middle of the canvas, staggered so cards do not stack. */
function freeSpot(at: MenuPos | null): MenuPos {
  const r = editor?.rect();
  if (at && editor) return editor.screenToFlow(at);
  const n = useModel.getState().nodes.length;
  if (r && editor) {
    const p = editor.screenToFlow({ x: r.left + r.width / 2 - 70, y: r.top + r.height / 2 - 30 });
    return { x: p.x + (n % 5) * 24, y: p.y + (n % 5) * 24 };
  }
  return { x: 80 + (n % 5) * 24, y: 80 + (n % 5) * 24 };
}

const widthOf = (id: string) => editor?.getInternalNode(id)?.measured.width ?? CARD_W;

/**
 * Add `tool` (or a Layer card when `params.layer` is given) following the rules above. The node editor has to be on screen: the menu
 * opens it first (see `ensureEditor`). One undo step. Returns the new node's id.
 */
export function placeTool(tool: string, opts: { at?: MenuPos | null; params?: Record<string, unknown>; wire?: string | null } = {}): string {
  const end = beginStep();
  try {
    const m = useModel.getState();
    const plan = tool === "input_layer" ? ({ rule: 4 } as Plan) : planPlacement(tool, opts.wire ?? null);
    const pos = freeSpot(opts.at ?? null);
    const id = m.addNode(tool, plan.rule === 2 ? { x: plan.from.position.x + widthOf(plan.from.id) + 60, y: plan.from.position.y } : pos, opts.params);
    const first = specs()[tool]?.inputs[0];
    const note = (s: string) => useApp.getState().setStatus(s);
    if (plan.rule === 1 || plan.rule === 3) {
      const problem = splice(id, plan.edge);
      if (problem) note(`Not inserted into the connection: ${problem}.`);
      else if (plan.rule === 3) between(id, plan.edge);
      else useModel.setState({ nodes: useModel.getState().nodes.map((n) => (n.id === id ? { ...n, position: { x: pos.x - CARD_W / 2, y: pos.y - 20 } } : n)) });
    } else if (plan.rule === 2 && first) {
      if (plan.splice) {
        const problem = splice(id, plan.splice);
        if (problem) connectAfter(id, plan.from.id, first.id);
        else between(id, plan.splice);
      } else connectAfter(id, plan.from.id, first.id);
    }
    selectNode(id);
    return id;
  } finally {
    end();
  }
}

function connectAfter(id: string, from: string, port: string) {
  if (connectProblem(from, id, port) === null) useModel.getState().connect({ source: from, sourceHandle: "out", target: id, targetHandle: port });
}

/** Put a spliced node in the gap between its neighbours when there is room. */
function between(id: string, e: Edge) {
  const { nodes } = useModel.getState();
  const a = nodes.find((n) => n.id === e.source);
  const b = nodes.find((n) => n.id === e.target);
  if (!a || !b) return;
  const left = a.position.x + widthOf(a.id) + 20;
  const right = b.position.x - 20;
  const x = right - left >= CARD_W ? (left + right - CARD_W) / 2 : left;
  useModel.setState({ nodes: nodes.map((n) => (n.id === id ? { ...n, position: { x, y: a.position.y } } : n)) });
}
