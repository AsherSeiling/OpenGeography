// Split by Attribute (B6): one new layer, or one file, per value of a field. Shows what will be made before anything is written.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option, Radio, RadioGroup } from "@fluentui/react-components";
import { open } from "../ipc/dialogs";
import { useEffect, useState } from "react";
import { runSplit } from "../actions";
import { api, errorText, IN_TAURI, SPLIT_CONFIRM_ABOVE, SPLIT_MAX_OUTPUTS, type SplitGroup, type SplitTarget } from "../ipc/api";
import { displayInfo, useApp } from "../store/store";

/** What the dialog tells the user about the number of outputs; `ok` is false when Split must stay disabled. */
export function splitVerdict(groups: SplitGroup[] | null, confirmed: boolean): { ok: boolean; message: string | null } {
  if (!groups) return { ok: false, message: null };
  if (groups.length === 0) return { ok: false, message: "This layer has no rows to split." };
  if (groups.length > SPLIT_MAX_OUTPUTS) return { ok: false, message: `That field has ${groups.length.toLocaleString()} different values; the limit is ${SPLIT_MAX_OUTPUTS.toLocaleString()}. Pick a field with fewer values.` };
  if (groups.length > SPLIT_CONFIRM_ABOVE && !confirmed) return { ok: false, message: `This will make ${groups.length.toLocaleString()} outputs. Tick the box to continue.` };
  return { ok: true, message: null };
}

export function SplitDialog({ owner, onClose }: { owner: string; onClose: () => void }) {
  const info = useApp((s) => displayInfo(s, owner));
  const base = useApp((s) => s.layers[owner]);
  const [field, setField] = useState<string>("");
  const [groups, setGroups] = useState<SplitGroup[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [kind, setKind] = useState<"layers" | "files">("layers");
  const [dir, setDir] = useState("");
  const [format, setFormat] = useState<"GPKG" | "ESRI Shapefile">("GPKG");
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!field || !info) return;
    let live = true;
    setGroups(null);
    setErr(null);
    setConfirmed(false);
    api.splitGroups(info.id, field).then((g) => live && setGroups(g), (e) => live && setErr(errorText(e)));
    return () => {
      live = false;
    };
  }, [field, info?.id]);

  if (!info || !base) return null;
  const verdict = splitVerdict(groups, confirmed);
  const needsDir = kind === "files" && !dir.trim();
  const go = async () => {
    setBusy(true);
    const target: SplitTarget = kind === "layers" ? { kind: "layers" } : { kind: "files", dir: dir.trim(), format };
    const ok = await runSplit(owner, field, target, confirmed);
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Split by Attribute — {base.name}</DialogTitle>
          <DialogContent className="stack">
            <Field label="Split by the values of" size="small">
              <Dropdown size="small" placeholder="Choose a field" value={field} selectedOptions={field ? [field] : []} onOptionSelect={(_, d) => setField(d.optionValue ?? "")}>
                {info.fields.map((f) => <Option key={f.name} value={f.name}>{f.name}</Option>)}
              </Dropdown>
            </Field>
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
            {groups && groups.length > 0 && (
              <div className="muted small" data-testid="split-groups">
                {groups.length.toLocaleString()} value{groups.length === 1 ? "" : "s"}, {groups.reduce((n, g) => n + g.rows, 0).toLocaleString()} rows
                {base.definitionQuery ? " (after the definition query)" : ""}: {groups.slice(0, 6).map((g) => `${g.value} (${g.rows.toLocaleString()})`).join(", ")}
                {groups.length > 6 ? ", …" : ""}
              </div>
            )}
            <RadioGroup value={kind} onChange={(_, d) => setKind(d.value as never)}>
              <Radio value="layers" label="One new layer per value (added to the map)" />
              <Radio value="files" label="One file per value (written to a folder)" />
            </RadioGroup>
            {kind === "files" && (
              <>
                <Field label="Folder" size="small">
                  <div className="btn-row">
                    <Input className="grow" size="small" value={dir} placeholder="Folder to write into" onChange={(_, d) => setDir(d.value)} />
                    {IN_TAURI && (
                      <Button size="small" onClick={async () => { const p = await open({ directory: true, title: "Folder for the split files" }); if (typeof p === "string") setDir(p); }}>
                        Browse…
                      </Button>
                    )}
                  </div>
                </Field>
                <Field label="Format" size="small">
                  <Dropdown size="small" value={format === "GPKG" ? "GeoPackage (.gpkg)" : "Shapefile (.shp)"} selectedOptions={[format]} onOptionSelect={(_, d) => setFormat(d.optionValue as never)}>
                    <Option value="GPKG">GeoPackage (.gpkg)</Option>
                    <Option value="ESRI Shapefile">Shapefile (.shp)</Option>
                  </Dropdown>
                </Field>
                <div className="muted small">File names are the values made safe for any system. Existing files are never overwritten: a number is added instead. The coordinate system is written with each file.</div>
              </>
            )}
            {groups && groups.length > SPLIT_CONFIRM_ABOVE && groups.length <= SPLIT_MAX_OUTPUTS && (
              <Checkbox label={`Yes, make ${groups.length.toLocaleString()} outputs`} checked={confirmed} onChange={(_, d) => setConfirmed(!!d.checked)} />
            )}
            {verdict.message && <MessageBar intent={groups && groups.length > SPLIT_MAX_OUTPUTS ? "error" : "warning"}><MessageBarBody>{verdict.message}</MessageBarBody></MessageBar>}
            <div className="muted small">Method after QGIS native:splitvectorlayer (reimplemented). Each output keeps the coordinate system and sources of the input.</div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!verdict.ok || needsDir || busy} onClick={() => void go()}>{busy ? "Splitting…" : "Split"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
