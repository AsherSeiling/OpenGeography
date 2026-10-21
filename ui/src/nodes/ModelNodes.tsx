// ModelBuilder-style nodes: blue ovals for data, yellow rectangles for tools,
// green ovals for outputs. Status badges show stale/running/done/error.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { CheckmarkCircle16Filled, ErrorCircle16Filled, ArrowSync16Regular, History16Regular, Globe16Regular } from "@fluentui/react-icons";
import type { NodeResult } from "../ipc/api";
import { useRun, type ModelNode } from "../store/model";
import { useApp } from "../store/store";

function Badge({ r }: { r?: NodeResult }) {
  if (!r) return null;
  const title = r.error ?? r.message ?? `${r.rows?.toLocaleString() ?? "?"} rows · ${r.millis} ms`;
  switch (r.status) {
    case "running":
      return <ArrowSync16Regular className="badge running" title="Running" />;
    case "error":
    case "skipped":
      return <ErrorCircle16Filled className={`badge ${r.status}`} title={title} />;
    case "cached":
      return <History16Regular className="badge cached" title={`Reused from cache · ${title}`} />;
    default:
      return <CheckmarkCircle16Filled className="badge done" title={title} />;
  }
}

function rowsLabel(r?: NodeResult) {
  return r?.rows !== null && r?.rows !== undefined ? `${r.rows.toLocaleString()} rows` : null;
}

export function DataNode({ id, data, selected }: NodeProps<ModelNode>) {
  const r = useRun((s) => s.results[id]);
  const layer = useApp((s) => s.layers[String(data.params.layer ?? "")]);
  return (
    <div className={`mnode data ${selected ? "selected" : ""} ${r?.status ?? ""}`}>
      <Badge r={r} />
      <div className="mnode-title">{layer?.name ?? "Choose a layer…"}</div>
      {layer && <div className="mnode-sub">{layer.geomType} · {layer.featureCount.toLocaleString()}</div>}
      <Handle type="source" position={Position.Right} id="out" className="port out" />
    </div>
  );
}

export function ToolNode({ id, data, selected }: NodeProps<ModelNode>) {
  const r = useRun((s) => s.results[id]);
  const spec = useRun((s) => s.specs[data.tool]);
  const inputs = spec?.inputs ?? [];
  const h = Math.max(54, 22 + inputs.length * 22);
  return (
    <div className="mnode-tool-wrap">
      <div className={`mnode tool ${selected ? "selected" : ""} ${r?.status ?? ""}`} style={{ minHeight: h }}>
        <Badge r={r} />
        {inputs.map((p, i) => (
          <div key={p.id} className="port-label" style={{ top: 18 + i * 22 }}>
            <Handle type="target" position={Position.Left} id={p.id} className={`port in ${p.kind}`} style={{ top: 7 }} />
            <span>{p.label}</span>
          </div>
        ))}
        <div className="mnode-title" style={{ paddingLeft: inputs.length ? 72 : 8 }}>{spec?.name ?? data.tool}</div>
        {r?.reprojected && (
          <div className="mnode-flag" title="An input was reprojected to match the first input's coordinate system">
            <Globe16Regular /> reprojected
          </div>
        )}
      </div>
      {spec?.output && (
        <div className={`mnode-out ${r?.status === "done" || r?.status === "cached" ? "ready" : ""}`} title="Output — click the step and use Preview to see it on the map">
          <span>{rowsLabel(r) ?? "Output"}</span>
          <Handle type="source" position={Position.Right} id="out" className="port out" />
        </div>
      )}
    </div>
  );
}

export function OutputNode({ id, data, selected }: NodeProps<ModelNode>) {
  const r = useRun((s) => s.results[id]);
  const spec = useRun((s) => s.specs[data.tool]);
  const label = data.tool === "add_to_map" ? String(data.params.name ?? "Result") : String(data.params.path ?? "").split(/[\\/]/).pop() || "Choose file…";
  return (
    <div className={`mnode output ${selected ? "selected" : ""} ${r?.status ?? ""}`}>
      <Handle type="target" position={Position.Left} id="in" className="port in table" />
      <Badge r={r} />
      <div className="mnode-title">{spec?.name ?? data.tool}</div>
      <div className="mnode-sub">{label}</div>
    </div>
  );
}

export const nodeTypes = { data: DataNode, tool: ToolNode, output: OutputNode };
