// Properties of the selected statistics card: its inputs (layer or table, "selected features only") and its options, drawn from the tool's
// own ToolSpec, the same definition a node of that tool uses. Changing anything clears the card's result: the old numbers belong to the old settings.
import { Checkbox, Dropdown, Field, Input, Option, Textarea } from "@fluentui/react-components";
import { useShallow } from "zustand/react/shallow";
import type { ToolParam } from "../ipc/api";
import { useRun } from "../store/model";
import { useApp } from "../store/store";
import { projectLayerInfos } from "../store/maps";
import { useStats } from "./store";
import type { CardInput } from "./types";

export function StatCardProperties({ sheetId, id }: { sheetId: string; id: string }) {
  const card = useStats((s) => s.docs[sheetId]?.cards.find((c) => c.id === id));
  const spec = useRun((s) => (card ? s.specs[card.tool] : undefined));
  const layers = useApp(useShallow((s) => projectLayerInfos(s).filter((l) => l && !l.temporary && l.geomType !== "Raster")));
  const allLayers = useApp((s) => s.layers);
  const selection = useApp((s) => s.selection);
  if (!card) return <div className="empty-hint pad">That card is no longer on the sheet.</div>;
  if (!spec) return <div className="empty-hint pad">The tool '{card.tool}' is not available in this version of GIS Studio.</div>;
  const edit = useStats.getState().editCard;
  const setInput = (port: string, v: CardInput) => edit(sheetId, id, { inputs: { ...card.inputs, [port]: v } });
  const setOption = (pid: string, v: unknown) => edit(sheetId, id, { options: { ...card.options, [pid]: v } });
  // Fields offered for a field option: those of the layer on the port it reads.
  const fieldsOf = (port: string | null) => (port ? allLayers[card.inputs[port]?.layer ?? ""]?.fields ?? [] : []);
  return (
    <div className="props">
      <div className="props-title">{spec.name}</div>
      <div className="props-body stack">
        <div className="muted">{spec.description}</div>
        <Field label="Card title (optional)">
          <Input size="small" value={card.title ?? ""} placeholder={spec.name} onChange={(_, d) => edit(sheetId, id, { title: d.value || null })} />
        </Field>
        {spec.inputs.map((port) => {
          const input = card.inputs[port.id];
          const sel = input ? (selection[input.layer] ?? []) : [];
          return (
            <div key={port.id} className="stat-props-field">
              <Field label={port.label}>
                <Dropdown
                  size="small"
                  value={input ? (allLayers[input.layer]?.name ?? "(removed)") : ""}
                  placeholder="Choose a layer or table"
                  onOptionSelect={(_, d) => d.optionValue && setInput(port.id, { layer: d.optionValue, fids: null })}
                >
                  {layers.map((l) => (
                    <Option key={l.id} value={l.id}>
                      {l.name}
                    </Option>
                  ))}
                </Dropdown>
              </Field>
              {input?.layer && (
                <Checkbox
                  label={input.fids ? `Selected features only (${input.fids.length.toLocaleString()} fixed when ticked)` : `Selected features only (${sel.length.toLocaleString()} selected on the map now)`}
                  checked={!!input.fids}
                  disabled={!input.fids && sel.length === 0}
                  onChange={(_, d) => setInput(port.id, { layer: input.layer, fids: d.checked ? [...sel] : null })}
                />
              )}
            </div>
          );
        })}
        {spec.params
          .filter((p) => !p.hidden)
          .map((p) => (
            <OptionField key={p.id} param={p} value={card.options[p.id]} fields={fieldsOf(p.fieldOf)} onChange={(v) => setOption(p.id, v)} />
          ))}
        {card.result && <div className="muted">Changing a setting clears this card's result.</div>}
      </div>
    </div>
  );
}

function OptionField({ param: p, value, fields, onChange }: { param: ToolParam; value: unknown; fields: { name: string }[]; onChange: (v: unknown) => void }) {
  const label = `${p.label}${p.optional ? " (optional)" : ""}`;
  switch (p.kind) {
    case "number":
      return (
        <Field label={label} hint={p.help || undefined}>
          <Input size="small" type="number" value={value === undefined || value === null ? "" : String(value)} onChange={(_, d) => onChange(d.value === "" ? null : Number(d.value))} />
        </Field>
      );
    case "bool":
      return <Checkbox label={p.label} checked={!!value} onChange={(_, d) => onChange(!!d.checked)} />;
    case "enum":
      return (
        <Field label={label}>
          <Dropdown size="small" value={p.options.find(([v]) => v === value)?.[1] ?? ""} onOptionSelect={(_, d) => onChange(d.optionValue)}>
            {p.options.map(([v, l]) => (
              <Option key={v} value={v}>
                {l}
              </Option>
            ))}
          </Dropdown>
        </Field>
      );
    case "field":
      return (
        <Field label={label} hint={fields.length ? undefined : "Choose the layer first to list its fields"}>
          <Dropdown size="small" value={String(value ?? "")} placeholder={p.optional ? "(none)" : "Choose a field"} onOptionSelect={(_, d) => onChange(d.optionValue || null)}>
            {p.optional && <Option value="">(none)</Option>}
            {fields.map((f) => (
              <Option key={f.name} value={f.name}>
                {f.name}
              </Option>
            ))}
          </Dropdown>
        </Field>
      );
    case "expression":
      return (
        <Field label={label} hint={p.help || undefined}>
          <Textarea value={String(value ?? "")} onChange={(_, d) => onChange(d.value)} rows={3} className="mono" />
        </Field>
      );
    default:
      // Colours, coordinate systems, queries and save paths are not offered by any statistics tool so far; a plain text box keeps such a tool usable.
      return (
        <Field label={label} hint={p.help || undefined}>
          <Input size="small" value={String(value ?? "")} onChange={(_, d) => onChange(d.value)} />
        </Field>
      );
  }
}
