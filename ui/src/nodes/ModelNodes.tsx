// Blender-style node cards: a colored header (blue = input layer, yellow =
// tool, green = output), then output sockets (right edge), the step's
// settings, and input sockets (left edge), one labeled row each.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { ArrowSync16Regular, CheckmarkCircle16Filled, DismissCircle16Regular, ErrorCircle16Filled, Globe16Regular, History16Regular } from "@fluentui/react-icons";
import { saveResultAsLayer } from "../actions";
import type { NodeResult } from "../ipc/api";
import { useRun, type ModelNode } from "../store/model";
import { useApp } from "../store/store";
import { ParamControl } from "./ParamControl";

function Badge({ r }: { r?: NodeResult }) {
  if (!r) return null;
  const title = r.error ?? r.message ?? `${r.rows?.toLocaleString() ?? "?"} rows · ${r.millis} ms`;
  switch (r.status) {
    case "running":
      return <ArrowSync16Regular className="badge running" title="Running" />;
    case "error":
    case "skipped":
      return <ErrorCircle16Filled className={`badge ${r.status}`} title={title} />;
    case "cancelled":
      return <DismissCircle16Regular className="badge cancelled" title="Cancelled — this step didn't finish" />;
    case "cached":
      return <History16Regular className="badge cached" title={`Reused from cache · ${title}`} />;
    default:
      return <CheckmarkCircle16Filled className="badge done" title={title} />;
  }
}

function NodeCard({ id, data, selected, kind }: NodeProps<ModelNode> & { kind: "data" | "tool" | "output" }) {
  const r = useRun((s) => s.results[id]);
  const spec = useRun((s) => s.specs[data.tool]);
  if (!spec) return <div className="bnode">{data.tool}</div>;
  const ok = r?.status === "done" || r?.status === "cached";
  const outLabel = spec.output === "vector" || kind === "data" ? "Features" : "Result";
  const fixedInput = kind === "data" && data.fixed;
  const params = spec.params.filter((p) => !p.hidden && !(fixedInput && p.id === "layer"));
  const title = fixedInput ? "Layer Input" : kind === "data" ? "Layer" : spec.name;
  return (
    <div className={`bnode ${kind} ${data.fixed ? "fixed" : ""} ${selected ? "selected" : ""} ${r?.status ?? ""}`}>
      <div className="bnode-head">
        <span className="bnode-title">{title}</span>
        <Badge r={r} />
      </div>
      {fixedInput && <LayerName id={String(data.params.layer ?? "")} />}
      {kind === "data" && !fixedInput && !data.params.layer && <div className="bnode-drop">Drop a layer here from Contents</div>}

      {spec.output && (
        <div className="bnode-row out">
          <span className={ok ? "" : "dim"}>{ok && r?.rows !== null ? `${outLabel} · ${r!.rows!.toLocaleString()}` : outLabel}</span>
          <Handle type="source" position={Position.Right} id="out" className={`socket ${spec.output}`} />
        </div>
      )}

      {params.length > 0 && (
        <div className="bnode-params nodrag nowheel">
          {params.map((p) => (
            <ParamControl key={p.id} nodeId={id} param={p} value={data.params[p.id]} compact />
          ))}
        </div>
      )}

      {spec.inputs.map((p) => (
        <div key={p.id} className="bnode-row in">
          <Handle type="target" position={Position.Left} id={p.id} className={`socket ${p.kind}`} />
          <span>{p.label}</span>
        </div>
      ))}

      {data.tool === "layer_output" && <OutputControls owner={String(data.params.layer ?? "")} />}

      {r?.reprojected && (
        <div className="bnode-note" title="An input was reprojected to match the first input's coordinate system">
          <Globe16Regular /> input reprojected
        </div>
      )}
      {r?.status === "error" && <div className="bnode-error" title={r.error ?? ""}>{r.error}</div>}
    </div>
  );
}

function LayerName({ id }: { id: string }) {
  const layer = useApp((s) => s.layers[id]);
  return (
    <div className="bnode-layer" title="This layer's original data">
      {layer ? `${layer.name} · ${layer.geomType} · ${layer.featureCount.toLocaleString()}` : "Layer removed"}
    </div>
  );
}

/** The layer's Output: show the result in place of the original, or save it as a new layer. */
function OutputControls({ owner }: { owner: string }) {
  const result = useApp((s) => s.results[owner]);
  const showOriginal = useApp((s) => !!s.showOriginal[owner]);
  return (
    <div className="bnode-params nodrag nowheel">
      <label className="bparam check" title="Draw the node result in place of the original data. The original is never changed.">
        <input type="checkbox" checked={!showOriginal} onChange={(e) => useApp.getState().setShowOriginal(owner, !e.target.checked)} />
        <span>Show result on map</span>
      </label>
      <button className="bctl bbtn bsave" disabled={!result} onClick={() => saveResultAsLayer(owner)} title={result ? "Copy the result into a new layer" : "Run the node tree first"}>
        Save as new layer…
      </button>
      {result && <div className="bnode-layer">{result.featureCount.toLocaleString()} features in result</div>}
    </div>
  );
}

export const nodeTypes = {
  data: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="data" />,
  tool: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="tool" />,
  output: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="output" />,
};
