// Compact, Blender-style parameter controls drawn inside Model nodes.
// Native inputs keep nodes light; "nodrag" stops React Flow from treating
// clicks in them as node drags.
import { save } from "@tauri-apps/plugin-dialog";
import { useShallow } from "zustand/react/shallow";
import { useDialog } from "../actions";
import { crsName } from "../crs/crs";
import type { ToolParam } from "../ipc/api";
import { fieldsForPort, useModel } from "../store/model";
import { useApp } from "../store/store";

const quote = (f: string) => (/^[A-Za-z_][A-Za-z0-9_]*$/.test(f) ? f : `"${f}"`);

export function ParamControl({ nodeId, param: p, value }: { nodeId: string; param: ToolParam; value: unknown; compact?: boolean }) {
  const set = (v: unknown) => useModel.getState().setParam(nodeId, p.id, v);
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && !l.temporary)));
  // Re-render when connections change so field lists stay current.
  useModel((s) => s.edges.length);
  const fields = p.fieldOf ? fieldsForPort(nodeId, p.fieldOf) : [];
  const str = value === undefined || value === null ? "" : String(value);

  const row = (control: React.ReactNode, wide = false) => (
    <label className={`bparam ${wide ? "wide" : ""}`} title={p.help || p.label}>
      <span className="bparam-label">{p.label}</span>
      {control}
    </label>
  );

  switch (p.kind) {
    case "number":
      return row(<input type="number" className="bctl" value={str} onChange={(e) => set(e.target.value === "" ? null : Number(e.target.value))} />);
    case "bool":
      return (
        <label className="bparam check" title={p.help || p.label}>
          <input type="checkbox" checked={!!value} onChange={(e) => set(e.target.checked)} />
          <span>{p.label}</span>
        </label>
      );
    case "enum":
      return row(
        <select className="bctl" value={str} onChange={(e) => set(e.target.value)}>
          {p.options.map(([v, l]) => (
            <option key={v} value={v}>{l}</option>
          ))}
        </select>,
      );
    case "layer":
      return row(
        <select className="bctl" value={str} onChange={(e) => set(e.target.value)}>
          {!str && <option value="">Choose…</option>}
          {layers.map((l) => (
            <option key={l.id} value={l.id}>{l.name}</option>
          ))}
        </select>,
      );
    case "field":
      return row(
        <select className="bctl" value={str} onChange={(e) => set(e.target.value || null)}>
          <option value="">{p.optional ? "(none)" : fields.length ? "Choose…" : "Connect input"}</option>
          {fields.map((f) => (
            <option key={f.name} value={f.name}>{f.name}</option>
          ))}
          {str && !fields.some((f) => f.name === str) && <option value={str}>{str}</option>}
        </select>,
      );
    case "expression":
      return row(
        <div className="bexpr">
          <textarea className="bctl mono" rows={2} value={str} placeholder={p.help.replace(/^SQL expression, e\.g\. /, "")} onChange={(e) => set(e.target.value)} />
          {fields.length > 0 && (
            <select className="bctl bfield-insert" value="" onChange={(e) => e.target.value && set(`${str}${str && !str.endsWith(" ") ? " " : ""}${quote(e.target.value)}`)} title="Insert a field">
              <option value="">+ field</option>
              {fields.map((f) => (
                <option key={f.name} value={f.name}>{f.name}</option>
              ))}
            </select>
          )}
        </div>,
        true,
      );
    case "crs":
      return row(
        <button
          className="bctl bbtn"
          onClick={(e) => {
            e.preventDefault();
            useDialog.getState().show({ kind: "crs", title: p.label, initial: str || null, onPick: (id) => set(id) });
          }}
        >
          {str ? crsName(str) : "Choose…"}
        </button>,
      );
    case "savePath":
      return row(
        <div className="bpath">
          <input className="bctl" value={str} placeholder="Choose a file…" onChange={(e) => set(e.target.value)} />
          <button
            className="bctl bbtn"
            onClick={async (e) => {
              e.preventDefault();
              const path = await save({ title: p.label });
              if (path) set(path);
            }}
          >
            …
          </button>
        </div>,
      );
    default:
      return row(<input className="bctl" value={str} onChange={(e) => set(e.target.value)} />);
  }
}
