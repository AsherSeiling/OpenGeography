// Mounted once: the Edit ribbon tab (while a session is open), the Z choice, the notices, and the edit dialogs (New Layer, Save / Discard /
// Cancel, resume or discard edits left by a crash).
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { api, errorText } from "../ipc/api";
import { getActiveCrs } from "../map/mapController";
import { registerContextualTab } from "../ribbon/tabs";
import { useApp } from "../store/store";
import { mapOf } from "../store/maps";
import { TABLE_FIELD_TYPES, tableProblem } from "../dialogs/NewTableDialog";
import { useDialog } from "../actions";
import { addByCoordinates, discardEdits, editRedo, editUndo, newLayer, offerResume, resumeEditing, saveEdits, startEditing, useEditDialogs } from "./editActions";
import { EDIT_GROUPS } from "../ribbon/tabsDef";
import { useEdit } from "./editStore";

/** The Z given to new vertices on a layer with Z values. Empty means none, which the engine stores as NaN. */
function ZChoice() {
  const st = useEdit((s) => s.status);
  const layerId = useEdit((s) => s.layerId);
  const z = useEdit((s) => s.zValue);
  const [text, setText] = useState(z === null ? "" : String(z));
  const layers = useApp((s) => s.layers);
  if (!layerId || !st?.layerHasZ) return null;
  const dems = Object.values(layers).filter((l) => l.raster && !l.raster.empty);
  const dem = st.zDem ? layers[st.zDem] : undefined;
  const apply = async () => {
    if (text === (z === null ? "" : String(z))) return; // unchanged: leaves a chosen DEM alone
    const v = text.trim() === "" ? null : Number(text);
    if (v !== null && !Number.isFinite(v)) return useApp.getState().setStatus("The Z value must be a number, or empty for none.");
    useEdit.getState().set({ zValue: v });
    useEdit.getState().bump(await api.editSetZ(layerId, v, null));
  };
  const pickDem = async (id: string | null) => {
    try {
      useEdit.getState().bump(await api.editSetZ(layerId, null, id));
    } catch (err) {
      useEdit.getState().set({ notice: errorText(err) });
    }
  };
  return (
    <div className="stack" style={{ padding: "0 8px", fontSize: 12 }} data-testid="edit-z">
      <label>
        New vertex Z{" "}
        <Input size="small" aria-label="New vertex Z" value={text} placeholder="none" onChange={(_, d) => setText(d.value)} onBlur={() => void apply()} style={{ width: 80 }} />
      </label>
      <label>
        or from a DEM{" "}
        <Dropdown size="small" aria-label="Z from a DEM" data-testid="edit-z-dem" value={dem?.name ?? "none"} selectedOptions={[st.zDem ?? ""]} onOptionSelect={(_, d) => void pickDem(d.optionValue ? d.optionValue : null)} style={{ minWidth: 120 }}>
          <Option value="">none</Option>
          {dems.map((l) => (
            <Option key={l.id} value={l.id}>{l.name}</Option>
          ))}
        </Dropdown>
      </label>
      <span className="muted" data-testid="edit-z-state">
        {dem ? `New vertices get Z from "${dem.name}" (its own values and units; none where it has no data)` : z === null ? "No Z: stored as NaN" : `New vertices get Z = ${z}`}
      </span>
    </div>
  );
}

function FinishDialog() {
  const open = useEditDialogs((s) => s.finish);
  const st = useEdit((s) => s.status);
  const [busy, setBusy] = useState(false);
  if (!open) return null;
  const n = st ? st.added + st.changed + st.deleted : 0;
  const close = () => useEditDialogs.getState().setFinish(false);
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && close()}>
      <DialogSurface style={{ maxWidth: 460 }}>
        <DialogBody>
          <DialogTitle>Finish editing</DialogTitle>
          <DialogContent>
            {n ? `${st!.added} added, ${st!.changed} changed, ${st!.deleted} deleted. Save them to the layer, or discard them?` : "There are no changes. Leave edit mode?"}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={close}>Cancel</Button>
            <Button disabled={busy} onClick={async () => { setBusy(true); if (await discardEdits()) close(); setBusy(false); }}>Discard edits</Button>
            <Button appearance="primary" disabled={busy} onClick={async () => { setBusy(true); if (await saveEdits()) close(); setBusy(false); }}>Save edits</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

const COORD_KINDS: [string, string, string][] = [
  ["xy", "x, y in the layer's coordinate system", "500000, 4000000"],
  ["lonlat", "Longitude, latitude (degrees)", "-99.25, 36.1"],
  ["usng", "USNG / MGRS reference", "14S MV 00000 00000"],
  ["bearing", "Bearing and distance (geodesic)", "500000, 4000000\n45, 120.5\n135, 80"],
];

function CoordsDialog() {
  const open = useEditDialogs((s) => s.coords);
  const [kind, setKind] = useState("xy");
  const [text, setText] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (!open) return null;
  const close = () => useEditDialogs.getState().setCoords(false);
  const k = COORD_KINDS.find((c) => c[0] === kind)!;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && close()}>
      <DialogSurface style={{ maxWidth: 520 }}>
        <DialogBody>
          <DialogTitle>Enter coordinates</DialogTitle>
          <DialogContent className="stack">
            <Field label="Coordinates are">
              <Dropdown aria-label="Coordinates are" value={k[1]} selectedOptions={[kind]} onOptionSelect={(_, d) => setKind(d.optionValue!)}>
                {COORD_KINDS.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
              </Dropdown>
            </Field>
            <Field label="One position per line" hint={kind === "bearing" ? "First line: start x, y in the layer's coordinate system. Then bearing (degrees clockwise from north) and distance (metres) per line, measured on the ellipsoid." : "A point layer takes one line; a line layer two or more; a polygon layer three or more."}>
              <textarea aria-label="Coordinates" className="mono" rows={6} value={text} placeholder={k[2]} onChange={(e) => setText(e.target.value)} style={{ width: "100%" }} />
            </Field>
            {err && <div className="field-error" role="alert">{err}</div>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={close}>Cancel</Button>
            <Button appearance="primary" disabled={busy || !text.trim()} onClick={async () => { setBusy(true); setErr(null); try { await addByCoordinates(kind as "xy", text); close(); } catch (e) { setErr(errorText(e)); } setBusy(false); }}>Add</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

function ResumeDialog() {
  const id = useEditDialogs((s) => s.resume);
  const layer = useApp((s) => (id ? s.layers[id] : undefined));
  if (!id) return null;
  return (
    <Dialog open>
      <DialogSurface style={{ maxWidth: 480 }}>
        <DialogBody>
          <DialogTitle>Edits were left unsaved</DialogTitle>
          <DialogContent>"{layer?.name ?? id}" has edits that were never saved (the program or project closed while editing). Resume them, or discard them?</DialogContent>
          <DialogActions>
            <Button onClick={async () => { await discardEdits(); useEditDialogs.getState().setResume(null); }}>Discard edits</Button>
            <Button appearance="primary" onClick={() => void resumeEditing(id)}>Resume editing</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

const GEOM: [string, string][] = [["point", "Points"], ["multipoint", "Multipoints"], ["line", "Lines"], ["polygon", "Polygons"]];

function NewLayerDialog() {
  const open = useEditDialogs((s) => s.newLayer);
  const [name, setName] = useState("New layer");
  const [geom, setGeom] = useState("point");
  const [crs, setCrs] = useState<{ id: string; name: string } | null>(null);
  const [fields, setFields] = useState<{ name: string; type: string }[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (open) {
      const m = getActiveCrs(useApp.getState().activeMapId);
      const doc = mapOf(useApp.getState(), useApp.getState().activeMapId);
      setCrs(m ? { id: m.id, name: doc?.crs?.name ?? m.id } : { id: "EPSG:3857", name: "Web Mercator" });
      setErr(null);
    }
  }, [open]);
  if (!open) return null;
  const close = () => useEditDialogs.getState().setNewLayer(false);
  const problem = !name.trim() ? "Give the layer a name." : !crs ? "Choose a coordinate system." : fields.length ? tableProblem(name, fields) : null;
  const create = async () => {
    setBusy(true);
    setErr(null);
    try {
      await newLayer(name.trim(), geom, crs!.id, fields.map((f) => [f.name.trim(), f.type] as [string, string]));
      close();
    } catch (e) {
      setErr(errorText(e));
    }
    setBusy(false);
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && close()}>
      <DialogSurface style={{ maxWidth: 540 }}>
        <DialogBody>
          <DialogTitle>New Layer</DialogTitle>
          <DialogContent className="stack">
            <Field label="Layer name"><Input aria-label="Layer name" value={name} onChange={(_, d) => setName(d.value)} /></Field>
            <Field label="Shape type">
              <Dropdown aria-label="Shape type" value={GEOM.find((g) => g[0] === geom)![1]} selectedOptions={[geom]} onOptionSelect={(_, d) => setGeom(d.optionValue!)}>
                {GEOM.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
              </Dropdown>
            </Field>
            <Field label="Coordinate system" hint="The map's by default. It is stored with the layer and never left blank.">
              <div className="btn-row">
                <span data-testid="new-layer-crs">{crs?.name ?? "—"} ({crs?.id})</span>
                <Button size="small" onClick={() => useDialog.getState().show({ kind: "crs", title: "Coordinate system of the new layer", initial: crs?.id, onPick: (id, n) => { setCrs({ id, name: n }); useDialog.getState().show(null); } })}>Change…</Button>
              </div>
            </Field>
            <div className="muted">An <code>id</code> field is added for you. Add other fields here; you can add more later in the Attribute Table.</div>
            <div className="classes">
              {fields.map((f, i) => (
                <div key={i} className="class-row">
                  <Input size="small" className="grow" aria-label={`Field ${i + 1} name`} placeholder="Field name" value={f.name} onChange={(_, d) => setFields(fields.map((x, j) => (j === i ? { ...x, name: d.value } : x)))} />
                  <Dropdown size="small" aria-label={`Field ${i + 1} type`} value={TABLE_FIELD_TYPES.find((t) => t[0] === f.type)![1]} selectedOptions={[f.type]} onOptionSelect={(_, d) => setFields(fields.map((x, j) => (j === i ? { ...x, type: d.optionValue! } : x)))} style={{ minWidth: 150 }}>
                    {TABLE_FIELD_TYPES.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
                  </Dropdown>
                  <Button size="small" appearance="subtle" aria-label={`Remove field ${i + 1}`} onClick={() => setFields(fields.filter((_, j) => j !== i))}>✕</Button>
                </div>
              ))}
            </div>
            <div className="btn-row"><Button size="small" onClick={() => setFields([...fields, { name: "", type: "text" }])}>Add field</Button></div>
            {(err || (problem && fields.some((f) => f.name.trim()))) && <div className="field-error">{err ?? problem}</div>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={close}>Cancel</Button>
            <Button appearance="primary" disabled={!!problem || busy} onClick={create}>Create layer</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

export function EditHost() {
  const layerId = useEdit((s) => s.layerId);
  const notice = useEdit((s) => s.notice);
  const layers = useApp((s) => s.layers);
  const name = useApp((s) => (layerId ? s.layers[layerId]?.name : undefined));
  // The Edit tab, while a session is open.
  useEffect(() => {
    if (!layerId) return;
    const off = registerContextualTab({ id: "Edit", band: "Editing", label: "Edit", visible: true, groups: [...EDIT_GROUPS, { label: "Z values", items: [{ custom: <ZChoice /> }] }] });
    return off;
  }, [layerId]);
  // Edits left by a crash or a closed project: offer to resume or discard.
  useEffect(() => {
    offerResume(layers);
  }, [layers]);
  // Undo / Redo keys inside the session (not while typing in a field).
  useEffect(() => {
    if (!layerId) return;
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "z" || (e.target as HTMLElement)?.closest?.("input, textarea, select, [contenteditable]")) return;
      e.preventDefault();
      e.stopPropagation();
      void (e.shiftKey ? editRedo() : editUndo());
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [layerId]);
  return (
    <>
      {layerId && (
        <div className="edit-banner" data-testid="edit-banner" role="status" style={{ position: "fixed", bottom: 34, left: "50%", transform: "translateX(-50%)", zIndex: 1000, maxWidth: 720 }}>
          <MessageBar intent={notice ? "warning" : "info"}>
            <MessageBarBody>{notice ?? `Editing ${name ?? layerId}. Changes stay pending until you Save.`}</MessageBarBody>
          </MessageBar>
        </div>
      )}
      <FinishDialog />
      <ResumeDialog />
      <NewLayerDialog />
      <CoordsDialog />
    </>
  );
}

export { startEditing };
