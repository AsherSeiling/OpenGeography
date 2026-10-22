// Field tools for the attribute table: Add Field, Calculate Field,
// Calculate Geometry, Rename Field.
import {
  Button,
  Checkbox,
  Combobox,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Dropdown,
  Field,
  Input,
  MessageBar,
  MessageBarBody,
  Option,
  Textarea,
} from "@fluentui/react-components";
import { useState } from "react";
import { api, errorText, type FieldType, type LayerInfo } from "../ipc/api";
import { useApp } from "../store/store";

const TYPES: [FieldType, string][] = [
  ["text", "Text"],
  ["integer", "Whole number (integer)"],
  ["double", "Decimal number (double)"],
  ["date", "Date"],
  ["boolean", "True / False"],
];

const quote = (f: string) => (/^[A-Za-z_][A-Za-z0-9_]*$/.test(f) ? f : `"${f}"`);

function useLayer(id: string): LayerInfo | undefined {
  return useApp((s) => s.layers[id] ?? Object.values(s.results).find((r) => r.id === id));
}

function Shell({ title, children, onClose, onOk, okLabel = "OK", busy, error }: { title: string; children: React.ReactNode; onClose: () => void; onOk: () => void; okLabel?: string; busy?: boolean; error?: string | null }) {
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{title}</DialogTitle>
          <DialogContent className="stack">
            {children}
            {error && (
              <MessageBar intent="error">
                <MessageBarBody>{error}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={busy} onClick={onOk}>{busy ? "Working…" : okLabel}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

/** Field picker that also accepts a new name (the field is then created). */
function TargetField({ info, value, onChange }: { info: LayerInfo; value: string; onChange: (v: string) => void }) {
  const exists = info.fields.some((f) => f.name === value);
  return (
    <Field label="Field" hint={value && !exists ? `"${value}" doesn't exist yet: it will be created.` : "Pick a field, or type a new name to create one."}>
      <Combobox freeform value={value} onChange={(e) => onChange(e.target.value)} onOptionSelect={(_, d) => onChange(d.optionValue ?? "")} placeholder="Field name">
        {info.fields.map((f) => (
          <Option key={f.name} value={f.name} text={f.name}>
            {f.name} <span className="muted">({f.type.toLowerCase()})</span>
          </Option>
        ))}
      </Combobox>
    </Field>
  );
}

function OnlySelected({ owner, value, onChange }: { owner: string; value: boolean; onChange: (v: boolean) => void }) {
  const n = useApp((s) => s.selection[owner]?.length ?? 0);
  return <Checkbox label={`Only selected rows (${n.toLocaleString()})`} disabled={n === 0} checked={value && n > 0} onChange={(_, d) => onChange(!!d.checked)} />;
}

export function AddFieldDialog({ layer, onClose }: { layer: string; onClose: () => void }) {
  const [name, setName] = useState("");
  const [ty, setTy] = useState<FieldType>("text");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const ok = async () => {
    setBusy(true);
    try {
      useApp.getState().updateLayer(await api.addField(layer, name.trim(), ty));
      useApp.getState().setStatus(`Added field ${name.trim()}`);
      onClose();
    } catch (e) {
      setErr(errorText(e));
    }
    setBusy(false);
  };
  return (
    <Shell title="Add Field" onClose={onClose} onOk={ok} okLabel="Add" busy={busy} error={err}>
      <Field label="Name">
        <Input value={name} onChange={(_, d) => setName(d.value)} autoFocus onKeyDown={(e) => e.key === "Enter" && ok()} placeholder="e.g. AREA_ACRES" />
      </Field>
      <Field label="Type">
        <Dropdown value={TYPES.find(([t]) => t === ty)![1]} onOptionSelect={(_, d) => setTy(d.optionValue as FieldType)}>
          {TYPES.map(([t, l]) => (
            <Option key={t} value={t}>{l}</Option>
          ))}
        </Dropdown>
      </Field>
    </Shell>
  );
}

const SNIPPETS: [string, string][] = [
  ["+ - * /", " + "],
  ["round(x, 2)", "round(, 2)"],
  ["upper(text)", "upper()"],
  ["a || ' ' || b", " || ' ' || "],
  ["CASE WHEN … THEN … ELSE … END", "CASE WHEN  THEN  ELSE  END"],
  ["coalesce(x, 0)", "coalesce(, 0)"],
  ["substr(text, 1, 5)", "substr(, 1, 5)"],
];

export function CalculateFieldDialog({ layer, owner, field, onClose }: { layer: string; owner: string; field?: string; onClose: () => void }) {
  const info = useLayer(layer);
  const [target, setTarget] = useState(field ?? "");
  const [expr, setExpr] = useState("");
  const [ty, setTy] = useState<FieldType | "auto">("auto");
  const [onlySel, setOnlySel] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (!info) return null;
  const isNew = target.trim() !== "" && !info.fields.some((f) => f.name === target.trim());
  const insert = (t: string) => setExpr((x) => (x && !x.endsWith(" ") ? `${x} ${t}` : `${x}${t}`));
  const ok = async () => {
    if (!target.trim() || !expr.trim()) {
      setErr("Choose a field and enter an expression.");
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      const fids = onlySel ? (useApp.getState().selection[owner] ?? []) : null;
      const r = await api.calculateField(layer, target.trim(), expr, isNew && ty !== "auto" ? ty : null, fids);
      useApp.getState().updateLayer(r.layer);
      useApp.getState().setStatus(`Calculated ${target.trim()} for ${r.count.toLocaleString()} row(s)`);
      onClose();
    } catch (e) {
      setErr(errorText(e));
    }
    setBusy(false);
  };
  return (
    <Shell title={`Calculate Field — ${info.name}`} onClose={onClose} onOk={ok} okLabel="Calculate" busy={busy} error={err}>
      <TargetField info={info} value={target} onChange={setTarget} />
      {isNew && (
        <Field label="New field type">
          <Dropdown value={ty === "auto" ? "Automatic (from the expression)" : TYPES.find(([t]) => t === ty)![1]} onOptionSelect={(_, d) => setTy(d.optionValue as FieldType | "auto")}>
            <Option value="auto">Automatic (from the expression)</Option>
            {TYPES.map(([t, l]) => (
              <Option key={t} value={t}>{l}</Option>
            ))}
          </Dropdown>
        </Field>
      )}
      <div>
        <div className="muted small">Fields (click to insert):</div>
        <div className="field-chips">
          {info.fields.map((f) => (
            <button key={f.name} className="chip" onClick={() => insert(quote(f.name))} title={f.type}>{f.name}</button>
          ))}
        </div>
        <div className="muted small">Helpers:</div>
        <div className="field-chips">
          {SNIPPETS.map(([label, text]) => (
            <button key={label} className="chip op" onClick={() => insert(text)}>{label}</button>
          ))}
        </div>
      </div>
      <Field label={`${target.trim() || "Field"} =`} hint="SQL expression. Text values go in single quotes: 'TX'.">
        <Textarea value={expr} onChange={(_, d) => setExpr(d.value)} rows={4} className="mono" placeholder="POP2020 / AREA_SQMI" />
      </Field>
      <OnlySelected owner={owner} value={onlySel} onChange={setOnlySel} />
    </Shell>
  );
}

const PROPS: [string, string][] = [
  ["area", "Area (geodesic)"],
  ["perimeter", "Perimeter (geodesic)"],
  ["length", "Length (geodesic)"],
  ["x", "Centroid longitude (x)"],
  ["y", "Centroid latitude (y)"],
];
const AREA: [string, string][] = [
  ["acres", "Acres"],
  ["sqmi", "Square miles"],
  ["sqkm", "Square kilometers"],
  ["ha", "Hectares"],
  ["sqm", "Square meters"],
  ["sqft", "Square feet"],
];
const LENGTH: [string, string][] = [
  ["mi", "Miles"],
  ["km", "Kilometers"],
  ["m", "Meters"],
  ["ft", "Feet (international)"],
  ["usft", "US survey feet"],
  ["yd", "Yards"],
];

export function CalculateGeometryDialog({ layer, owner, field, onClose }: { layer: string; owner: string; field?: string; onClose: () => void }) {
  const info = useLayer(layer);
  const isLine = info?.geomType === "Line";
  const [prop, setProp] = useState(isLine ? "length" : "area");
  const [areaU, setAreaU] = useState("acres");
  const [lenU, setLenU] = useState("mi");
  const defaultName = (p: string) => ({ area: "AREA", perimeter: "PERIMETER", length: "LENGTH", x: "CENTROID_X", y: "CENTROID_Y" })[p] ?? "VALUE";
  const [target, setTarget] = useState(field ?? defaultName(isLine ? "length" : "area"));
  const [onlySel, setOnlySel] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (!info) return null;
  const units = prop === "area" ? AREA : prop === "x" || prop === "y" ? null : LENGTH;
  const ok = async () => {
    if (!target.trim()) {
      setErr("Choose or name a field.");
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      const fids = onlySel ? (useApp.getState().selection[owner] ?? []) : null;
      const r = await api.calculateGeometry(layer, target.trim(), prop, areaU, lenU, fids);
      useApp.getState().updateLayer(r.layer);
      useApp.getState().setStatus(`Calculated ${target.trim()} for ${r.count.toLocaleString()} row(s)`);
      onClose();
    } catch (e) {
      setErr(errorText(e));
    }
    setBusy(false);
  };
  return (
    <Shell title={`Calculate Geometry — ${info.name}`} onClose={onClose} onOk={ok} okLabel="Calculate" busy={busy} error={err}>
      <Field label="Property">
        <Dropdown
          value={PROPS.find(([p]) => p === prop)![1]}
          onOptionSelect={(_, d) => {
            const p = d.optionValue!;
            if (!field && (target === defaultName(prop) || !target)) setTarget(defaultName(p));
            setProp(p);
          }}
        >
          {PROPS.map(([p, l]) => (
            <Option key={p} value={p}>{l}</Option>
          ))}
        </Dropdown>
      </Field>
      {units && (
        <Field label="Units">
          <Dropdown value={units.find(([u]) => u === (prop === "area" ? areaU : lenU))![1]} onOptionSelect={(_, d) => (prop === "area" ? setAreaU(d.optionValue!) : setLenU(d.optionValue!))}>
            {units.map(([u, l]) => (
              <Option key={u} value={u}>{l}</Option>
            ))}
          </Dropdown>
        </Field>
      )}
      <TargetField info={info} value={target} onChange={setTarget} />
      <div className="muted small">
        Measured on the Earth's ellipsoid, so results are true ground values in any coordinate system ({info.crsName ?? info.crs ?? "unknown"}).
      </div>
      <OnlySelected owner={owner} value={onlySel} onChange={setOnlySel} />
    </Shell>
  );
}

export function RenameFieldDialog({ layer, field, onClose }: { layer: string; field: string; onClose: () => void }) {
  const [name, setName] = useState(field);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const ok = async () => {
    setBusy(true);
    try {
      useApp.getState().updateLayer(await api.renameField(layer, field, name.trim()));
      onClose();
    } catch (e) {
      setErr(errorText(e));
    }
    setBusy(false);
  };
  return (
    <Shell title={`Rename Field "${field}"`} onClose={onClose} onOk={ok} okLabel="Rename" busy={busy} error={err}>
      <Input value={name} onChange={(_, d) => setName(d.value)} autoFocus onKeyDown={(e) => e.key === "Enter" && ok()} />
    </Shell>
  );
}
