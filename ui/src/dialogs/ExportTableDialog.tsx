// Export Table: a point layer's attributes plus its coordinates (X/Y or lon/lat) as CSV or Parquet, in a coordinate system the user picks.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, Option, Radio, RadioGroup } from "@fluentui/react-components";
import { save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { runExportTable } from "../actions";
import { crsName } from "../crs/crs";
import { IN_TAURI } from "../ipc/api";
import { displayInfo, useApp } from "../store/store";
import { CrsPicker } from "./Dialogs";

export function ExportTableDialog({ owner, onClose }: { owner: string; onClose: () => void }) {
  const info = useApp((s) => displayInfo(s, owner));
  const base = useApp((s) => s.layers[owner]);
  const nsel = useApp((s) => s.selection[owner]?.length ?? 0);
  const [which, setWhich] = useState<"all" | "selected">(nsel > 0 ? "selected" : "all");
  const [format, setFormat] = useState<"csv" | "parquet">("csv");
  const [crs, setCrs] = useState<string | null>(null);
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);
  if (!info || !base) return null;
  const ext = format === "csv" ? "csv" : "parquet";
  const fixedPath = path.trim() && !path.trim().toLowerCase().endsWith(`.${ext}`) ? `${path.trim()}.${ext}` : path.trim();
  const ready = !!crs && !!fixedPath && !busy;
  const go = async () => {
    setBusy(true);
    const ok = await runExportTable(owner, { path: fixedPath, format, crs: crs!, fids: which === "selected" ? (useApp.getState().selection[owner] ?? []) : null });
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Export Table — {base.name}</DialogTitle>
          <DialogContent className="stack">
            <RadioGroup value={which} onChange={(_, d) => setWhich(d.value as never)}>
              <Radio value="all" label={`All points (${info.featureCount.toLocaleString()}${base.definitionQuery ? ", after the definition query" : ""})`} />
              <Radio value="selected" disabled={nsel === 0} label={`Selected points only (${nsel.toLocaleString()})`} />
            </RadioGroup>
            <Field label="Coordinate system of the X/Y columns" size="small" required hint={crs ? `Chosen: ${crsName(crs)}` : "Choose one; nothing is assumed."}>
              <CrsPicker value={crs} onChange={(id) => setCrs(id)} />
            </Field>
            {info.crs && (
              <div className="btn-row">
                <Button size="small" onClick={() => setCrs(info.crs!)} data-testid="export-table-layer-crs">Use the layer's ({info.crsName ?? info.crs})</Button>
                <Button size="small" onClick={() => setCrs("EPSG:4326")}>WGS 84 lon/lat</Button>
              </div>
            )}
            <Field label="Format" size="small">
              <Dropdown size="small" value={format === "csv" ? "CSV (.csv)" : "Parquet (.parquet)"} selectedOptions={[format]} onOptionSelect={(_, d) => setFormat(d.optionValue as never)}>
                <Option value="csv">CSV (.csv)</Option>
                <Option value="parquet">Parquet (.parquet)</Option>
              </Dropdown>
            </Field>
            <Field label="Output file" size="small">
              <div className="btn-row">
                <Input className="grow" size="small" value={path} placeholder={`/path/to/${base.name}.${ext}`} onChange={(_, d) => setPath(d.value)} data-testid="export-table-path" />
                {IN_TAURI && (
                  <Button size="small" onClick={async () => { const p = await save({ title: `Export table of ${base.name}`, defaultPath: `${base.name}.${ext}`, filters: [{ name: format.toUpperCase(), extensions: [ext] }] }); if (p) setPath(p); }}>
                    Browse…
                  </Button>
                )}
              </div>
            </Field>
            <div className="muted small">
              Geographic systems write <b>lon</b>/<b>lat</b> in degrees; projected systems write <b>x</b>/<b>y</b> in the system's units. A note named <i>file</i>.crs.txt next to the file states the system, units and datum. A source file is never overwritten.
            </div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!ready} onClick={() => void go()}>{busy ? "Exporting…" : "Export"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
