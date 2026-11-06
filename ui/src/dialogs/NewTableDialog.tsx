// New Table (UR-14): a table with no map shapes, typed fields chosen here, rows added in the Attribute Table.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, Option } from "@fluentui/react-components";
import { useState } from "react";
import { api, errorText } from "../ipc/api";
import { useApp } from "../store/store";
import { showPane } from "../panes/panes";

export const TABLE_FIELD_TYPES: [string, string][] = [["text", "Text"], ["integer", "Whole number"], ["double", "Number with decimals"], ["date", "Date"], ["boolean", "Yes / no"]];

/** Why the field list can't make a table, or null. Mirrors the engine's checks so the button says so before a round trip. */
export function tableProblem(name: string, fields: { name: string; type: string }[]): string | null {
  if (!name.trim()) return "Give the table a name.";
  if (!fields.length) return "Add at least one field.";
  const seen = new Set<string>();
  for (const f of fields) {
    const n = f.name.trim();
    if (!n) return "A field has no name.";
    if (/^(geom|__fid|__g|__x0|__y0|__sz)$/i.test(n)) return `'${n}' is reserved. Choose another field name.`;
    if (seen.has(n.toLowerCase())) return `Two fields are named '${n}'. Field names must differ.`;
    seen.add(n.toLowerCase());
  }
  return null;
}

export function NewTableDialog({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("New table");
  const [fields, setFields] = useState([{ name: "name", type: "text" }]);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const problem = tableProblem(name, fields);
  const set = (i: number, patch: Partial<(typeof fields)[number]>) => setFields(fields.map((f, j) => (j === i ? { ...f, ...patch } : f)));
  const create = async () => {
    setBusy(true);
    try {
      const info = await api.createTable(name.trim(), fields.map((f) => [f.name.trim(), f.type] as [string, string]));
      const s = useApp.getState();
      s.addLayers([info]);
      s.setActiveLayer(info.id);
      s.setStatus(`Created the table ${info.name}. Add rows with Add row in the Attribute Table.`);
      showPane("table");
      onClose();
    } catch (e) {
      setErr(errorText(e));
      setBusy(false);
    }
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ maxWidth: 520 }}>
        <DialogBody>
          <DialogTitle>New Table</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">A table has rows and fields but no shapes on the map. Choose its fields now; you can add more later and fill in the rows in the Attribute Table.</div>
            <Field label="Table name">
              <Input aria-label="Table name" value={name} onChange={(_, d) => setName(d.value)} />
            </Field>
            <div className="classes" data-testid="new-table-fields">
              {fields.map((f, i) => (
                <div key={i} className="class-row">
                  <Input size="small" className="grow" aria-label={`Field ${i + 1} name`} placeholder="Field name" value={f.name} onChange={(_, d) => set(i, { name: d.value })} />
                  <Dropdown size="small" aria-label={`Field ${i + 1} type`} value={TABLE_FIELD_TYPES.find((t) => t[0] === f.type)![1]} selectedOptions={[f.type]} onOptionSelect={(_, d) => set(i, { type: d.optionValue! })} style={{ minWidth: 150 }}>
                    {TABLE_FIELD_TYPES.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
                  </Dropdown>
                  <Button size="small" appearance="subtle" aria-label={`Remove field ${i + 1}`} disabled={fields.length === 1} onClick={() => setFields(fields.filter((_, j) => j !== i))}>✕</Button>
                </div>
              ))}
            </div>
            <div className="btn-row"><Button size="small" onClick={() => setFields([...fields, { name: "", type: "text" }])}>Add field</Button></div>
            {(err || (problem && fields.some((f) => f.name.trim()))) && <div className="field-error">{err ?? problem}</div>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!!problem || busy} onClick={create}>Create table</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
