// Layer Properties: a floating window (like ArcGIS Pro / QGIS) with a
// section list on the left. Non-modal, draggable by its title bar and
// resizable; changes apply immediately.
import { Button, Checkbox, Field, Input } from "@fluentui/react-components";
import {
  ArrowExportRegular,
  CalculatorRegular,
  DeleteRegular,
  DismissRegular,
  DocumentRegular,
  EditRegular,
  FilterRegular,
  GlobeRegular,
  InfoRegular,
  PaintBrushRegular,
  TableAddRegular,
  TableRegular,
} from "@fluentui/react-icons";
import { useEffect, useRef, useState, type ReactElement } from "react";
import { create } from "zustand";
import { exportLayer, useDialog } from "../actions";
import { api, errorText } from "../ipc/api";
import { CrsInfo, DefinitionQuery, Symbology } from "../panes/PropertiesPane";
import { useApp } from "../store/store";

export type PropsSection = "general" | "source" | "symbology" | "query" | "fields" | "crs";

export const useLayerProps = create<{ layer: string | null; section: PropsSection; open: (layer: string, section?: PropsSection) => void; close: () => void }>((set) => ({
  layer: null,
  section: "general",
  open: (layer, section = "general") => set({ layer, section }),
  close: () => set({ layer: null }),
}));

/** Open a layer's Properties window (optionally at a section). */
export function openLayerProperties(layer: string, section?: PropsSection) {
  useLayerProps.getState().open(layer, section);
}

const SECTIONS: { id: PropsSection; label: string; icon: ReactElement; vector?: boolean }[] = [
  { id: "general", label: "General", icon: <InfoRegular /> },
  { id: "source", label: "Source", icon: <DocumentRegular /> },
  { id: "symbology", label: "Symbology", icon: <PaintBrushRegular />, vector: true },
  { id: "query", label: "Definition Query", icon: <FilterRegular /> },
  { id: "fields", label: "Fields", icon: <TableRegular /> },
  { id: "crs", label: "Coordinate System", icon: <GlobeRegular />, vector: true },
];

export function LayerPropertiesWindow() {
  const { layer, section, close } = useLayerProps();
  const info = useApp((s) => (layer ? s.layers[layer] : undefined));
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const dragging = useRef<{ x0: number; y0: number; px: number; py: number } | null>(null);

  useEffect(() => setPos({ x: 0, y: 0 }), [layer]);
  useEffect(() => {
    if (layer && !info) close(); // layer was removed
  }, [layer, info]);
  if (!layer || !info) return null;
  const isTable = info.geomType === "Table";

  const startDrag = (e: React.PointerEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    dragging.current = { x0: e.clientX, y0: e.clientY, px: pos.x, py: pos.y };
    const mv = (ev: PointerEvent) => {
      const d = dragging.current!;
      setPos({ x: d.px + ev.clientX - d.x0, y: d.py + ev.clientY - d.y0 });
    };
    const up = () => {
      dragging.current = null;
      window.removeEventListener("pointermove", mv, true);
      window.removeEventListener("pointerup", up, true);
    };
    window.addEventListener("pointermove", mv, true);
    window.addEventListener("pointerup", up, true);
  };

  return (
    <div className="lp-window" style={{ transform: `translate(calc(-50% + ${pos.x}px), calc(-50% + ${pos.y}px))` }} role="dialog" aria-label={`Layer Properties: ${info.name}`}>
      <div className="lp-title" onPointerDown={startDrag}>
        <span>Layer Properties: {info.name}</span>
        <button className="lp-x" onClick={close} title="Close" aria-label="Close">
          <DismissRegular />
        </button>
      </div>
      <div className="lp-body">
        <nav className="lp-nav">
          {SECTIONS.filter((s) => !(s.vector && isTable)).map((s) => (
            <button key={s.id} className={`lp-nav-item ${section === s.id ? "on" : ""}`} onClick={() => useLayerProps.setState({ section: s.id })}>
              {s.icon}
              <span>{s.label}</span>
              {s.id === "query" && info.definitionQuery && <span className="lp-dot" title="Definition query active" />}
            </button>
          ))}
        </nav>
        <div className="lp-content">
          {section === "general" && <General id={layer} />}
          {section === "source" && <Source id={layer} />}
          {section === "symbology" && <Symbology id={layer} />}
          {section === "query" && <DefinitionQuery id={layer} />}
          {section === "fields" && <Fields id={layer} />}
          {section === "crs" && <CrsInfo id={layer} />}
        </div>
      </div>
      <div className="lp-foot">
        <span className="muted small">Changes apply immediately.</span>
        <Button appearance="primary" onClick={close}>Close</Button>
      </div>
    </div>
  );
}

function General({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const visible = useApp((s) => !!s.visible[id]);
  const [name, setName] = useState(info.name);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => setName(info.name), [info.name]);
  const rename = async () => {
    if (!name.trim() || name.trim() === info.name) return;
    try {
      useApp.getState().updateLayer(await api.renameLayer(id, name.trim()));
    } catch (e) {
      setErr(errorText(e));
    }
  };
  return (
    <div className="stack">
      <Field label="Name" validationMessage={err ?? undefined} validationState={err ? "error" : "none"}>
        <Input value={name} onChange={(_, d) => setName(d.value)} onBlur={rename} onKeyDown={(e) => e.key === "Enter" && rename()} />
      </Field>
      {info.geomType !== "Table" && <Checkbox label="Visible on the map" checked={visible} onChange={(_, d) => useApp.getState().setVisible(id, !!d.checked)} />}
      <dl className="kv">
        <dt>Type</dt>
        <dd>{info.geomType === "Table" ? "Table (no geometry)" : `${info.geomType} features`}</dd>
        <dt>Features</dt>
        <dd>
          {info.definitionQuery ? `${(info.queryCount ?? 0).toLocaleString()} of ${info.featureCount.toLocaleString()} (definition query)` : info.featureCount.toLocaleString()}
        </dd>
        <dt>Fields</dt>
        <dd>{info.fields.length}</dd>
        <dt>Coordinate system</dt>
        <dd>{info.crsName ?? info.crs ?? "Not defined"}</dd>
      </dl>
      <div className="btn-row">
        <Button size="small" icon={<ArrowExportRegular />} onClick={() => exportLayer(id)}>Export Features…</Button>
      </div>
    </div>
  );
}

function Source({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const e = info.extent;
  const ne = info.nativeExtent;
  return (
    <dl className="kv">
      <dt>Source</dt>
      <dd className="wrap">{info.source ?? (info.temporary ? "Model preview" : "Created in GIS Studio")}</dd>
      <dt>Stored as</dt>
      <dd className="mono">{info.table} (project database)</dd>
      <dt>Layer id</dt>
      <dd className="mono">{info.id}</dd>
      {e && (
        <>
          <dt>Extent (lon/lat)</dt>
          <dd className="mono">
            W {e[0].toFixed(5)} S {e[1].toFixed(5)}
            <br />E {e[2].toFixed(5)} N {e[3].toFixed(5)}
          </dd>
        </>
      )}
      {ne && (
        <>
          <dt>Extent (native)</dt>
          <dd className="mono">
            {ne[0].toFixed(3)}, {ne[1].toFixed(3)}
            <br />
            {ne[2].toFixed(3)}, {ne[3].toFixed(3)}
          </dd>
        </>
      )}
      {info.overviews.length > 0 && (
        <>
          <dt>Map overviews</dt>
          <dd>Zoom levels {info.overviews.join(", ")} (for fast zoomed-out drawing)</dd>
        </>
      )}
      {info.warnings.length > 0 && (
        <>
          <dt>Warnings</dt>
          <dd>{info.warnings.join(" ")}</dd>
        </>
      )}
    </dl>
  );
}

function Fields({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const showsResult = useApp((s) => !!s.results[id] && !s.showOriginal[id]);
  const dlg = useDialog.getState().show;
  const del = async (f: string) => {
    if (!confirm(`Delete field "${f}" from ${info.name}? This can't be undone.`)) return;
    try {
      useApp.getState().updateLayer(await api.deleteField(id, f));
    } catch (e) {
      useApp.getState().setStatus(errorText(e));
    }
  };
  return (
    <div className="stack">
      {showsResult && <div className="muted small">The map shows this layer's node result; field edits apply to the original data.</div>}
      <div className="btn-row">
        <Button size="small" icon={<TableAddRegular />} onClick={() => dlg({ kind: "addField", layer: id })}>Add Field</Button>
        <Button size="small" icon={<CalculatorRegular />} onClick={() => dlg({ kind: "calcField", layer: id, owner: id })}>Calculate</Button>
      </div>
      <table className="lp-fields">
        <thead>
          <tr>
            <th>Name</th>
            <th>Type</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {info.fields.map((f) => (
            <tr key={f.name}>
              <td>{f.name}</td>
              <td className="muted">{f.type.toLowerCase()}</td>
              <td className="lp-field-actions">
                <button title="Rename" onClick={() => dlg({ kind: "renameField", layer: id, field: f.name })}>
                  <EditRegular />
                </button>
                <button title="Delete" onClick={() => del(f.name)}>
                  <DeleteRegular />
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
