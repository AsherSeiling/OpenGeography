// Blender-style node cards: a colored header (blue = input layer, yellow =
// tool, green = output), then output sockets (right edge), the step's
// settings, and input sockets (left edge), one labeled row each.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { ArrowSync16Regular, CheckmarkCircle16Filled, ErrorCircle16Filled, Globe16Regular, History16Regular } from "@fluentui/react-icons";
import type { NodeResult } from "../ipc/api";
import { useRun, type ModelNode } from "../store/model";
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
  return (
    <div className={`bnode ${kind} ${selected ? "selected" : ""} ${r?.status ?? ""}`}>
      <div className="bnode-head">
        <span className="bnode-title">{spec.name}</span>
        <Badge r={r} />
      </div>

      {spec.output && (
        <div className="bnode-row out">
          <span className={ok ? "" : "dim"}>{ok && r?.rows !== null ? `${outLabel} · ${r!.rows!.toLocaleString()}` : outLabel}</span>
          <Handle type="source" position={Position.Right} id="out" className={`socket ${spec.output}`} />
        </div>
      )}

      {spec.params.length > 0 && (
        <div className="bnode-params nodrag nowheel">
          {spec.params.map((p) => (
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

      {r?.reprojected && (
        <div className="bnode-note" title="An input was reprojected to match the first input's coordinate system">
          <Globe16Regular /> input reprojected
        </div>
      )}
      {r?.status === "error" && <div className="bnode-error" title={r.error ?? ""}>{r.error}</div>}
    </div>
  );
}

export const nodeTypes = {
  data: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="data" />,
  tool: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="tool" />,
  output: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="output" />,
};
