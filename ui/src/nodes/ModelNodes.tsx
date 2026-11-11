// Blender-style node cards: a header tinted by tool family (Output and Save to File use the rule colour),
// then output sockets (right edge), the step's settings, and input sockets (left edge), one labeled row each.
// Sockets are shape-coded (vector circle, raster square, table diamond, any ring) in nodes.css.
import { ToolIcon } from "./toolIcon";
import { useShallow } from "zustand/react/shallow";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { useEffect } from "react";
import { applyFix, startChecks, useChecks } from "./checks";
import { Warning16Filled, Eye16Regular, ArrowSync16Regular, CheckmarkCircle16Filled, DismissCircle16Regular, ErrorCircle16Filled, Globe16Regular, History16Regular } from "@fluentui/react-icons";
import { saveResultAsLayer } from "../actions";
import type { NodeResult } from "../ipc/api";
import { Menu, MenuItem, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import { asOneStep, connectProblem, nodeSignature, outputKind, useModel, useRun, type ModelNode } from "../store/model";
import { useApp } from "../store/store";
import { projectLayerInfos } from "../store/maps";
import { splitCategory } from "./categories";
import { previewAtNode } from "./nodePreview";
import { HelpButton } from "./ToolHelp";
import { ParamControl } from "./ParamControl";
import { isWaiting, reviewScript } from "../scripts/script";

/** Card header family: the `--fam-<name>` token pair, or "Output" for the rule-coloured header. */
const FAMILY: Record<string, string> = { Input: "Input", Vector: "Vector", Raster: "Raster", Hydrology: "Hydrology", Tables: "Tables", "Coordinate Systems": "Coordinate", Analysis: "Analysis", Styling: "Styling", Output: "Output", "Save to File": "Output" };
export function familyOf(category: string, role?: string): string {
  if (role === "output") return "Output";
  if (role === "data") return "Input";
  return FAMILY[splitCategory(category)[0]] ?? "Analysis";
}

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

/** True when the node (or anything upstream) changed since it last ran successfully. */
function useChanged(id: string): boolean {
  const ran = useRun((s) => s.ranSig[id]);
  // Re-evaluate whenever the tree or the layers change.
  useModel((s) => s.nodes);
  useModel((s) => s.edges);
  useApp((s) => s.layers);
  return !!ran && nodeSignature(id) !== ran;
}

function NodeCard({ id, data, selected, kind }: NodeProps<ModelNode> & { kind: "data" | "tool" | "output" }) {
  const r = useRun((s) => s.results[id]);
  const spec = useRun((s) => s.specs[data.tool]);
  const changed = useChanged(id);
  if (!spec) return <div className="bnode">{data.tool}</div>;
  const ok = r?.status === "done" || r?.status === "cached";
  const outLabel = spec.output === "vector" || kind === "data" ? "Features" : "Result";
  const fixedInput = kind === "data" && data.fixed;
  const params = spec.params.filter((p) => !p.hidden && !(fixedInput && p.id === "layer"));
  const family = familyOf(spec.category, spec.role);
  const connected = useModel((st) => st.edges.filter((e) => e.target === id).map((e) => e.targetHandle).join("|"));
  // The output socket's shape follows the real data (a polygon layer is a circle even though its spec says "any").
  const outKind = useModel((st) => { st.nodes; st.edges; return outputKind(id); });
  useApp((st) => st.layers);
  const sourced = useModel((st) => st.edges.some((e) => e.source === id));
  const title = fixedInput ? "Layer Input" : kind === "data" ? "Layer" : spec.name;
  return (
    <div className={`bnode ${kind} fam-${family} ${data.fixed ? "fixed" : ""} ${selected ? "selected" : ""} ${r?.status ?? ""} ${data.muted ? "muted" : ""} ${data.collapsed ? "collapsed" : ""}`}>
      <div className="bnode-head">
        {kind !== "data" && <span className="bnode-icon"><ToolIcon tool={data.tool} /></span>}
        <span className="bnode-title">{title}</span>
        {data.muted && <span className="bnode-tag" title="Muted (M): bypassed — its first input passes straight through">muted</span>}
        {changed && !data.muted && <span className="bnode-changed" title="Changed since the last run — run the tree to update" />}
        {kind === "tool" && <HelpButton tool={data.tool} name={spec.name} />}
        {kind === "tool" && spec.output && (
          <button className="bnode-eye nodrag" aria-label={`Preview ${spec.name} on the map`} title="Preview this step's result on the map (runs only the steps it needs)" onClick={(e) => { e.stopPropagation(); void previewAtNode(id); }}>
            <Eye16Regular />
          </button>
        )}
        <Badge r={r} />
      </div>
      {fixedInput && <LayerName id={String(data.params.layer ?? "")} />}
      {kind === "data" && !fixedInput && !data.params.layer && <div className="bnode-drop">Drop a layer here from Map Layers</div>}
      {kind === "data" && !fixedInput && !!data.params.layer && <MissingLayer id={String(data.params.layer)} />}

      {spec.output && (
        <div className="bnode-row out">
          <span className={ok ? "" : "dim"}>{ok && r?.rows !== null ? `${outLabel} · ${r!.rows!.toLocaleString()}` : outLabel}</span>
          <Handle type="source" position={Position.Right} id="out" className={`socket ${outKind ?? spec.output} ${sourced ? "linked" : ""}`} />
        </div>
      )}

      {params.length > 0 && !data.collapsed && (
        <div className="bnode-params nodrag nowheel">
          {params.map((p) => (
            <ParamControl key={p.id} nodeId={id} param={p} value={data.params[p.id]} compact />
          ))}
        </div>
      )}

      {spec.inputs.map((p, i) => {
        const linked = connected.split("|").includes(p.id);
        return (
          <div key={p.id} className="bnode-row in">
            <Handle type="target" position={Position.Left} id={p.id} className={`socket ${p.kind} ${linked ? "linked" : ""}`} />
            <span>{p.label}</span>
            {i > 0 && !linked && !data.muted && <ChooseLayer nodeId={id} port={p.id} portLabel={p.label} />}
          </div>
        );
      })}

      {data.tool === "layer_output" && !data.collapsed && <OutputControls owner={String(data.params.layer ?? "")} status={r?.status} changed={changed} />}

      {!data.collapsed && <NodeProblems id={id} />}
      {r?.reprojected && (
        <div className="bnode-note" title="An input was reprojected to match the first input's coordinate system">
          <Globe16Regular /> input reprojected
        </div>
      )}
      {r?.status === "error" && (
        <div className="bnode-error" title={r.error ?? ""}>
          {isWaiting(r.error) ? <ScriptWaiting nodeId={id} params={data.params} /> : r.error}
        </div>
      )}
    </div>
  );
}

/** UX7: what is wrong with this step before it runs, each with its safe one-click fix if it has one. */
function NodeProblems({ id }: { id: string }) {
  const list = useChecks((s) => s.problems[id]);
  useEffect(() => startChecks(), []);
  if (!list?.length) return null;
  return (
    <div className="bnode-problems nodrag" data-testid="node-problems">
      {list.map((p, i) => (
        <div key={`${p.code}-${i}`} className={`bnode-problem ${p.severity}`} title={p.message}>
          {p.severity === "error" ? <ErrorCircle16Filled /> : <Warning16Filled />}
          <span>{p.message}</span>
          {p.fix && (
            <button className="bctl bbtn" onClick={(e) => { e.stopPropagation(); const said = applyFix(p); if (said) useApp.getState().setStatus(said); }}>
              {p.fix.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

/** Why `layer` can't feed `port` of `nodeId`, or null (same rule as dragging it in). */
export function layerProblem(layer: { id: string; geomType: string }, nodeId: string, port: string): string | null {
  const kind = layer.geomType === "Table" ? "table" : layer.geomType === "Raster" ? "raster" : "vector";
  return connectProblem(`pick-${layer.id}`, nodeId, port, kind);
}

/** Add a Layer card for `layer` and connect it to `port` of `nodeId`, as ModelPane's drop does, in one undo step. Returns the new id. */
export function connectLayerTo(nodeId: string, port: string, layer: string): string | null {
  const target = useModel.getState().nodes.find((n) => n.id === nodeId);
  const l = useApp.getState().layers[layer];
  if (!target || !l || layerProblem(l, nodeId, port)) return null;
  return asOneStep(() => {
    const m = useModel.getState();
    const id = m.addNode("input_layer", { x: target.position.x - 280, y: target.position.y + 60 }, { layer });
    m.connect({ source: id, sourceHandle: "out", target: nodeId, targetHandle: port });
    useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: n.id === id })) });
    useApp.getState().setProperties({ kind: "node", id });
    return id;
  });
}

/**
 * "Choose a layer" on an empty input: adds a Layer card and connects it, exactly what dragging a layer onto the card builds,
 * as one undo step. Layers the input can't take are listed but disabled, with connectProblem's reason.
 */
function ChooseLayer({ nodeId, port, portLabel }: { nodeId: string; port: string; portLabel: string }) {
  const layers = useApp(useShallow((s) => projectLayerInfos(s).filter((l) => l && !l.temporary)));
  const owner = useModel((s) => s.layer);
  const choices = layers.filter((l) => l.id !== owner);
  return (
    <Menu>
      <MenuTrigger disableButtonEnhancement>
        <button className="bchoose nodrag" title={`Pick a project layer to feed "${portLabel}"`} disabled={choices.length === 0}>
          {choices.length ? "Choose a layer" : "No other layers"}
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {choices.map((l) => {
            const problem = layerProblem(l, nodeId, port);
            return (
              <MenuItem key={l.id} disabled={!!problem} title={problem ?? undefined} onClick={() => connectLayerTo(nodeId, port, l.id)}>
                {l.name} · {l.geomType}
              </MenuItem>
            );
          })}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** A Layer node whose layer was removed from the project. */
function MissingLayer({ id }: { id: string }) {
  const exists = useApp((s) => !!s.layers[id]);
  return exists ? null : <div className="bnode-error">This layer was removed from the project. Drop another layer here from Map Layers, or delete this node.</div>;
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
function OutputControls({ owner, status, changed }: { owner: string; status?: string; changed: boolean }) {
  const result = useApp((s) => s.results[owner]);
  const showOriginal = useApp((s) => !!s.showOriginal[owner]);
  // Never present an old result as current.
  const stale =
    result && (status === "error" || status === "skipped" || status === "cancelled")
      ? "The last run didn't finish — the map shows the previous result."
      : result && changed
        ? "The tree changed since this result was made — run to update."
        : null;
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
      {stale && <div className="bnode-stale">{stale}</div>}
    </div>
  );
}

export const nodeTypes = {
  data: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="data" />,
  tool: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="tool" />,
  output: (p: NodeProps<ModelNode>) => <NodeCard {...p} kind="output" />,
};


/** A node whose Python script came from a project or model that this computer has not approved: show it and offer the approval dialog. */
function ScriptWaiting({ nodeId, params }: { nodeId: string; params: Record<string, unknown> }) {
  const script = String(params.expression ?? params.where ?? "");
  return (
    <span data-testid="node-script-waiting">
      Script waiting for approval.{" "}
      <button
        className="bctl bbtn"
        data-nodrag="true"
        onClick={async (e) => {
          e.preventDefault();
          if (await reviewScript(script)) useApp.getState().setStatus("Script approved. Run the model again to use it.");
        }}
        aria-label={`Review and approve the script of ${nodeId}`}
      >
        Review and approve…
      </button>
    </span>
  );
}
