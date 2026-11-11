// UX5: the parts of the batch dialog that are plain logic, so they can be tested without a window.
import type { BatchItem, BatchSource } from "../ipc/api";
import type { ModelNode } from "../store/model";
import type { Edge } from "@xyflow/react";

/** The step whose result a batch writes by default: whatever feeds the tree's Output. */
export function defaultResultNode(nodes: ModelNode[], edges: Edge[]): string | null {
  const out = nodes.find((n) => n.data.tool === "layer_output");
  const feed = out && edges.find((e) => e.target === out.id);
  return feed?.source ?? null;
}

/** The Layer Input nodes a batch can feed, the tree's own first. */
export function inputNodes(nodes: ModelNode[]): ModelNode[] {
  return nodes.filter((n) => n.data.tool === "input_layer" && !n.data.muted).sort((a, b) => Number(!!b.data.fixed) - Number(!!a.data.fixed));
}

/** Add `paths` to the list without repeating one already there. */
export function addFiles(list: BatchSource[], paths: string[]): BatchSource[] {
  const have = new Set(list.map((s) => (s.kind === "file" ? `f:${s.path}` : `l:${s.id}`)));
  const fresh = paths.filter((p) => !have.has(`f:${p}`)).map((path) => ({ kind: "file" as const, path }));
  return [...list, ...fresh];
}

export function addLayers(list: BatchSource[], ids: string[]): BatchSource[] {
  const have = new Set(list.filter((s) => s.kind === "layer").map((s) => (s as { id: string }).id));
  return [...list, ...ids.filter((id) => !have.has(id)).map((id) => ({ kind: "layer" as const, id }))];
}

export function summarize(items: BatchItem[]): string {
  const n = (s: BatchItem["status"]) => items.filter((i) => i.status === s).length;
  const parts = [`${n("done")} succeeded`, `${n("failed")} failed`];
  if (n("skipped")) parts.push(`${n("skipped")} left as they were (file already exists)`);
  if (n("cancelled")) parts.push(`${n("cancelled")} cancelled`);
  return `${items.length} input${items.length === 1 ? "" : "s"}: ${parts.join(", ")}`;
}

/** The report as plain text (tab separated, pastes into a spreadsheet). */
export function reportText(items: BatchItem[]): string {
  const row = (i: BatchItem) => [i.name, i.status, i.rows ?? "", i.output ?? "", (i.message ?? "").replace(/\s+/g, " ")].join("\t");
  return [["Input", "Result", "Rows", "File written", "Notes"].join("\t"), ...items.map(row)].join("\n");
}
