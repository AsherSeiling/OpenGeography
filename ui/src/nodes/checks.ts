// UX7: check before running. Problems are marked on the node that has them, with a one-click fix only where the fix is safe.
//
//  * Coordinate-system problems come from the engine (`check_graph`; its single `crs_problems` function holds every CRS rule).
//    This file never decides anything about a CRS.
//  * Structural problems (an input with nothing connected, the wrong kind of data connected, a chosen field the data feeding
//    the node doesn't have) are worked out here from the tool specs, as the editor already does for connections and pickers.
import { create } from "zustand";
import { api, type Field, type NodeProblem } from "../ipc/api";
import { asOneStep, connectProblem, fieldsForPort, graphPayload, outputKind, specs, useModel } from "../store/model";
import { useApp } from "../store/store";

export interface Problem {
  node: string;
  severity: "error" | "warning";
  code: string;
  message: string;
  fix?: Fix;
}

/** A fix the user can apply with one click; always an ordinary, undoable edit to the tree. */
export type Fix =
  | { kind: "project"; port: string; crs: string; label: string }
  | { kind: "set-param"; param: string; value: string; label: string };

/** Fold a field name for comparison: DuckDB treats names case-insensitively. */
const norm = (s: string) => s.toLowerCase();
/** Fold further for suggesting a near match (case, spaces, underscores and punctuation ignored). */
const loose = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");

/** A chosen field the data feeding the node doesn't have. Only when the fields are known, and only as a warning: field lists are the tools' declared rules. */
export function fieldProblem(label: string, value: string, fields: Field[]): { message: string; suggestion?: string } | null {
  if (!value || fields.length === 0) return null;
  if (fields.some((f) => norm(f.name) === norm(value))) return null;
  const near = fields.filter((f) => loose(f.name) === loose(value) && loose(value) !== "");
  return {
    message: `Field "${value}" is not among the fields of "${label}"${near.length === 1 ? `; did you mean "${near[0].name}"?` : ""}`,
    suggestion: near.length === 1 ? near[0].name : undefined,
  };
}

/** Structural problems of the open tree, found without the engine. */
export function localProblems(): Problem[] {
  const { nodes, edges } = useModel.getState();
  const out: Problem[] = [];
  for (const n of nodes) {
    if (n.data.muted) continue;
    const spec = specs()[n.data.tool];
    if (!spec) continue;
    for (const port of spec.inputs) {
      const e = edges.find((x) => x.target === n.id && x.targetHandle === port.id);
      if (!e) {
        out.push({ node: n.id, severity: "error", code: "unconnected", message: `Connect an input to "${port.label}".` });
        continue;
      }
      const why = connectProblem(e.source, n.id, port.id, outputKind(e.source));
      if (why) out.push({ node: n.id, severity: "error", code: "wrong-kind", message: why });
    }
    for (const p of spec.params) {
      if (p.kind !== "field" || !p.fieldOf || p.hidden) continue;
      const value = n.data.params[p.id];
      if (typeof value !== "string") continue;
      const port = spec.inputs.find((i) => i.id === p.fieldOf);
      const fp = fieldProblem(port?.label ?? p.fieldOf, value.trim(), fieldsForPort(n.id, p.fieldOf));
      if (fp) {
        out.push({
          node: n.id,
          severity: "warning",
          code: "missing-field",
          message: fp.message,
          fix: fp.suggestion ? { kind: "set-param", param: p.id, value: fp.suggestion, label: `Use "${fp.suggestion}"` } : undefined,
        });
      }
    }
  }
  return out;
}

export function fromEngine(list: NodeProblem[]): Problem[] {
  return list.map((p) => ({
    node: p.node,
    severity: p.severity,
    code: p.code,
    message: p.message,
    fix: p.fix ? { kind: "project", port: p.fix.port, crs: p.fix.crs, label: `Project "${portLabel(p.node, p.fix.port)}" to ${p.fix.crs} first` } : undefined,
  }));
}

function portLabel(nodeId: string, port: string): string {
  const n = useModel.getState().nodes.find((x) => x.id === nodeId);
  return (n && specs()[n.data.tool]?.inputs.find((i) => i.id === port)?.label) || port;
}

/** Group by node, errors first. */
export function byNode(list: Problem[]): Record<string, Problem[]> {
  const out: Record<string, Problem[]> = {};
  for (const p of list) (out[p.node] ??= []).push(p);
  for (const v of Object.values(out)) v.sort((a, b) => (a.severity === b.severity ? 0 : a.severity === "error" ? -1 : 1));
  return out;
}

interface ChecksState {
  problems: Record<string, Problem[]>;
  set: (p: Record<string, Problem[]>) => void;
}
export const useChecks = create<ChecksState>((set) => ({ problems: {}, set: (problems) => set({ problems }) }));

let seq = 0;
/** Re-check the open tree now. A slow or failed engine check never leaves stale CRS claims: they are dropped. */
export async function refreshChecks(): Promise<void> {
  const mine = ++seq;
  const m = useModel.getState();
  if (!m.layer || !m.nodes.length || !Object.keys(specs()).length) return useChecks.getState().set({});
  const local = localProblems();
  useChecks.getState().set(byNode(local));
  let engine: Problem[] = [];
  try {
    engine = fromEngine(await api.checkGraph(graphPayload()));
  } catch {
    engine = [];
  }
  if (mine !== seq) return; // a newer check is under way
  useChecks.getState().set(byNode([...local, ...engine]));
}

let started = false;
let timer: ReturnType<typeof setTimeout> | undefined;
/** Re-check (after a short pause) whenever the tree, the layers or the tool list change. Safe to call many times. */
export function startChecks() {
  if (started) return;
  started = true;
  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(() => void refreshChecks(), 350);
  };
  useModel.subscribe((s, prev) => {
    if (s.nodes !== prev.nodes || s.edges !== prev.edges || s.layer !== prev.layer) later();
  });
  useApp.subscribe((s, prev) => {
    if (s.layers !== prev.layers) later();
  });
  later();
}

/** Apply a fix as ONE undo step. Returns a sentence for the status bar, or null when the tree no longer matches the problem. */
export function applyFix(p: Problem): string | null {
  const fix = p.fix;
  if (!fix) return null;
  const { nodes, edges } = useModel.getState();
  const target = nodes.find((n) => n.id === p.node);
  if (!target) return null;
  if (fix.kind === "set-param") {
    useModel.getState().setParam(p.node, fix.param, fix.value);
    return `Set ${fix.param} to "${fix.value}"`;
  }
  const edge = edges.find((e) => e.target === p.node && e.targetHandle === fix.port);
  if (!edge) return null;
  const from = nodes.find((n) => n.id === edge.source);
  if (!from) return null;
  asOneStep(() => {
    const m = useModel.getState();
    const id = m.addNode("project", { x: target.position.x - 300, y: target.position.y + 80 }, { crs: fix.crs });
    useModel.setState({ edges: useModel.getState().edges.filter((e) => e.id !== edge.id) });
    m.connect({ source: edge.source, sourceHandle: edge.sourceHandle ?? "out", target: id, targetHandle: "in" });
    m.connect({ source: id, sourceHandle: "out", target: p.node, targetHandle: fix.port });
  });
  return `Added a Project step to ${fix.crs} in front of "${portLabel(p.node, fix.port)}"`;
}
