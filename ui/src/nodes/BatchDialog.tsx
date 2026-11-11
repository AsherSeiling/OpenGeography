// UX5: "Batch" in the node editor: run this node tree over a folder of files or a list of layers, with a report per input.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Option, Tooltip } from "@fluentui/react-components";
import { PlayMultiple16Regular } from "@fluentui/react-icons";
import { useState } from "react";
import { api, errorText, type BatchItem, type BatchSource } from "../ipc/api";
import { open } from "../ipc/dialogs";
import { track, useProgress } from "../progress/progress";
import { graphPayload, specs, useModel } from "../store/model";
import { useApp } from "../store/store";
import { projectLayerInfos } from "../store/maps";
import { addFiles, addLayers, defaultResultNode, inputNodes, reportText, summarize } from "./batchModel";

const FORMATS: [string, string][] = [
  ["GPKG", "GeoPackage (.gpkg)"],
  ["ESRI Shapefile", "Shapefile (.shp)"],
  ["GeoJSON", "GeoJSON (.geojson)"],
  ["FlatGeobuf", "FlatGeobuf (.fgb)"],
  ["PARQUET", "GeoParquet (.parquet)"],
  ["CSV", "CSV (.csv)"],
];

const say = (t: string) => useApp.getState().setStatus(t);
const nameOf = (s: BatchSource) => (s.kind === "file" ? s.path.split(/[\\/]/).pop() || s.path : (useApp.getState().layers[s.id]?.name ?? s.id));

export function BatchButton() {
  const [shown, setShown] = useState(false);
  const hasTree = useModel((s) => !!s.layer);
  return (
    <>
      <Tooltip content="Run this node tree over a folder of files or a list of layers, with a report for each" relationship="label">
        <Button size="small" appearance="subtle" icon={<PlayMultiple16Regular />} disabled={!hasTree} onClick={() => setShown(true)} data-testid="batch-button">
          Batch
        </Button>
      </Tooltip>
      {shown && <BatchDialog onClose={() => setShown(false)} />}
    </>
  );
}

function BatchDialog({ onClose }: { onClose: () => void }) {
  const nodes = useModel((s) => s.nodes);
  const edges = useModel((s) => s.edges);
  const treeLayer = useModel((s) => s.layer);
  const layers = useApp((s) => projectLayerInfos(s).filter((l) => l && !l.temporary && l.id !== treeLayer));
  const ins = inputNodes(nodes);
  const [inputNode, setInputNode] = useState(ins[0]?.id ?? "");
  const [resultNode, setResultNode] = useState(defaultResultNode(nodes, edges) ?? "");
  const [sources, setSources] = useState<BatchSource[]>([]);
  const [outDir, setOutDir] = useState("");
  const [format, setFormat] = useState("GPKG");
  const [overwrite, setOverwrite] = useState(false);
  const [addToProject, setAddToProject] = useState(false);
  const [running, setRunning] = useState(false);
  const [items, setItems] = useState<BatchItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pickLayers, setPickLayers] = useState<string[]>([]);
  const task = useProgress((s) => s.tasks.find((t) => t.label.startsWith("Batch run")));

  const stepName = (id: string) => {
    const n = nodes.find((x) => x.id === id);
    return n ? (specs()[n.data.tool]?.name ?? n.data.tool) : id;
  };
  const candidates = nodes.filter((n) => !n.data.fixed || n.data.tool === "input_layer").filter((n) => n.data.tool !== "layer_output" && n.data.tool !== "export" && !n.data.muted);

  const pickFolder = async () => {
    try {
      const folder = (await open({ directory: true, title: "Folder of data files" })) as string | null;
      if (!folder) return;
      const files = await api.batchListFolder(folder);
      if (!files.length) return setError("That folder has no data files this version can read (shapefiles, GeoPackages, GeoJSON, rasters and similar).");
      setError(null);
      setSources((l) => addFiles(l, files));
    } catch (e) {
      setError(errorText(e));
    }
  };
  const pickFiles = async () => {
    try {
      const p = await open({ multiple: true, title: "Data files to run over" });
      if (!p) return;
      setError(null);
      setSources((l) => addFiles(l, Array.isArray(p) ? p : [p]));
    } catch (e) {
      setError(errorText(e));
    }
  };
  const pickOut = async () => {
    try {
      const d = (await open({ directory: true, title: "Folder for the results (new files only)" })) as string | null;
      if (d) setOutDir(d);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const ready = !!inputNode && !!resultNode && sources.length > 0 && !!outDir && !running;
  const run = async () => {
    setRunning(true);
    setError(null);
    setItems(null);
    try {
      const res = await track(api.runBatch({ graph: graphPayload(), inputNode, resultNode, sources, outDir, format, overwrite, addToProject }));
      setItems(res);
      const added = res.map((r) => r.layer).filter((l): l is NonNullable<typeof l> => !!l);
      if (added.length) useApp.getState().addLayers(added);
      say(summarize(res));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !running && onClose()}>
      <DialogSurface aria-label="Batch run" style={{ maxWidth: 760 }}>
        <DialogBody>
          <DialogTitle>Batch run over many inputs</DialogTitle>
          <DialogContent>
            <div className="stack" data-testid="batch-dialog">
              <div className="muted">
                Runs this node tree once for each input and writes each result as a new file. Your source files are never changed, and a result file that already exists is left alone unless you allow replacing.
              </div>
              <label>
                Layer Input that receives each input
                <Dropdown value={stepName(inputNode)} selectedOptions={[inputNode]} onOptionSelect={(_, d) => setInputNode(d.optionValue ?? "")} disabled={running}>
                  {ins.map((n) => (
                    <Option key={n.id} value={n.id} text={n.data.fixed ? "Layer Input (this tree's own)" : `Layer ${useApp.getState().layers[String(n.data.params.layer ?? "")]?.name ?? n.id}`}>
                      {n.data.fixed ? "Layer Input (this tree's own)" : `Layer ${useApp.getState().layers[String(n.data.params.layer ?? "")]?.name ?? n.id}`}
                    </Option>
                  ))}
                </Dropdown>
              </label>
              <label>
                Write the result of this step
                <Dropdown value={stepName(resultNode)} selectedOptions={[resultNode]} onOptionSelect={(_, d) => setResultNode(d.optionValue ?? "")} disabled={running}>
                  {candidates.filter((n) => n.id !== inputNode).map((n) => (
                    <Option key={n.id} value={n.id} text={stepName(n.id)}>
                      {stepName(n.id)}
                    </Option>
                  ))}
                </Dropdown>
              </label>
              <div>
                <b>Inputs</b> ({sources.length})
                <div className="btn-row">
                  <Button size="small" onClick={pickFolder} disabled={running}>Add a folder…</Button>
                  <Button size="small" onClick={pickFiles} disabled={running}>Add files…</Button>
                  <Dropdown
                    size="small"
                    multiselect
                    placeholder="Add project layers…"
                    disabled={running || layers.length === 0}
                    selectedOptions={pickLayers}
                    onOptionSelect={(_, d) => {
                      setPickLayers(d.selectedOptions);
                      setSources((l) => addLayers(l, d.selectedOptions));
                    }}
                  >
                    {layers.map((l) => (
                      <Option key={l.id} value={l.id} text={l.name}>
                        {l.name}
                      </Option>
                    ))}
                  </Dropdown>
                  {sources.length > 0 && <Button size="small" appearance="subtle" onClick={() => { setSources([]); setPickLayers([]); }} disabled={running}>Clear</Button>}
                </div>
                <div className="batch-inputs">
                  {sources.map((s, i) => (
                    <div key={i} className="batch-input" title={s.kind === "file" ? s.path : "Project layer (its original data)"}>
                      {nameOf(s)} <span className="muted">{s.kind === "layer" ? "project layer" : ""}</span>
                    </div>
                  ))}
                </div>
              </div>
              <div className="btn-row">
                <Button size="small" onClick={pickOut} disabled={running}>Results folder…</Button>
                <span className="muted" title={outDir}>{outDir || "none chosen"}</span>
              </div>
              <label>
                Result format
                <Dropdown value={FORMATS.find((f) => f[0] === format)?.[1] ?? format} selectedOptions={[format]} onOptionSelect={(_, d) => setFormat(d.optionValue ?? "GPKG")} disabled={running}>
                  {FORMATS.map(([id, label]) => (
                    <Option key={id} value={id} text={label}>
                      {label}
                    </Option>
                  ))}
                </Dropdown>
              </label>
              <Checkbox checked={addToProject} onChange={(_, d) => setAddToProject(!!d.checked)} label="Also add each result to the project as a layer" disabled={running} />
              <Checkbox checked={overwrite} onChange={(_, d) => setOverwrite(!!d.checked)} label="Replace result files that already exist in the results folder" disabled={running} />
              {error && <div className="bnode-error" role="alert">{error}</div>}
              {items && (
                <div data-testid="batch-report">
                  <b>{summarize(items)}</b>
                  <table className="batch-report">
                    <thead>
                      <tr><th>Input</th><th>Result</th><th>Rows</th><th>Notes</th></tr>
                    </thead>
                    <tbody>
                      {items.map((i, k) => (
                        <tr key={k} className={`batch-${i.status}`}>
                          <td title={i.source}>{i.name}</td>
                          <td>{{ done: "Succeeded", failed: "Failed", skipped: "Left as it was", cancelled: "Cancelled" }[i.status]}</td>
                          <td>{i.rows?.toLocaleString() ?? ""}</td>
                          <td title={i.output ?? undefined}>{i.message ?? (i.output ? "Written" : "")}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </DialogContent>
          <DialogActions>
            {items && (
              <Button onClick={() => void navigator.clipboard?.writeText(reportText(items)).then(() => say("Batch report copied"))}>Copy report</Button>
            )}
            {running && (
              <Button disabled={!task || task.detail === "Cancelling…"} onClick={() => task && api.cancelTask(task.id)}>
                {task?.detail === "Cancelling…" ? "Cancelling…" : "Cancel"}
              </Button>
            )}
            <Button onClick={onClose} disabled={running}>Close</Button>
            <Button appearance="primary" disabled={!ready} onClick={() => void run()}>
              {running ? (task?.detail ? task.detail : "Running…") : "Run batch"}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
