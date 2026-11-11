// UX6: preview at any node. One action: run the tree only as far as this node (upstream steps that haven't changed come
// from the cache), then draw its result as a temporary layer with its row count. Nothing is written to the project's layers.
import { previewNode, runModel } from "../actions";
import { nodeSignature, useModel, useRun } from "../store/model";
import { useApp } from "../store/store";

/** The one rule: a step needs running before it can be previewed unless it finished and nothing changed since. */
export function previewNeedsRun(status: string | undefined, hasOutput: boolean, ranSig: string | undefined, nowSig: string): boolean {
  if (status !== "done" && status !== "cached") return true;
  return !hasOutput || ranSig !== nowSig;
}

/** Why a node can't be previewed on the map, or null. Rasters are files, not tables, so they have no map preview yet. */
export function previewBlocked(output: string | undefined | null, specOutput: string | null | undefined): string | null {
  if (!specOutput) return "This step has no output to preview.";
  if (specOutput === "raster" || (output && !output.startsWith("__node_"))) return "Raster steps can't be previewed on the map yet. Run to here and use the result's own layer.";
  return null;
}

export async function previewAtNode(nodeId: string) {
  const node = useModel.getState().nodes.find((n) => n.id === nodeId);
  if (!node) return;
  const run = useRun.getState();
  if (run.running) return;
  const blocked = previewBlocked(run.results[nodeId]?.output, run.specs[node.data.tool]?.output);
  if (blocked) {
    useApp.getState().setStatus(blocked);
    return;
  }
  const r = run.results[nodeId];
  if (previewNeedsRun(r?.status, !!r?.output, run.ranSig[nodeId], nodeSignature(nodeId))) {
    await runModel(nodeId);
    if (useRun.getState().results[nodeId]?.status !== "done" && useRun.getState().results[nodeId]?.status !== "cached") return; // the card shows why
  }
  await previewNode(nodeId);
}
