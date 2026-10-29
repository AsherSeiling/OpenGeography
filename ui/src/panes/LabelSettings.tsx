// The Labels section of a layer's Properties (L1): which field to show, how it looks, and the scales it shows at.
import { Checkbox, Combobox, Dropdown, Field, Input, Option, Slider } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { matchFonts, useFonts } from "../map/fonts";
import { displayInfo, defaultLabel, useApp, type LabelStyle, type LayerStyle } from "../store/store";

/** A new style with the label settings changed; turning labels on picks a field. */
export function withLabel(style: LayerStyle, patch: Partial<LabelStyle>, firstField: string): LayerStyle {
  const cur = style.label ?? { ...defaultLabel(firstField), enabled: false };
  return { ...style, label: { ...cur, ...patch, field: patch.field ?? (cur.field || firstField) } };
}

/** "" or a non-positive number clears a scale limit. */
export function parseScale(text: string): number | undefined {
  const v = Math.round(Number(text.replace(/[, ]/g, "").replace(/^1:/, "")));
  return Number.isFinite(v) && v > 0 ? v : undefined;
}

export function LabelSettings({ id }: { id: string }) {
  const info = useApp((s) => displayInfo(s, id));
  const style = useApp((s) => s.styles[id]);
  if (!info || !style || info.geomType === "Raster" || info.geomType === "Table") return null;
  const first = info.fields[0]?.name ?? "";
  const label = style.label;
  const set = (patch: Partial<LabelStyle>) => useApp.getState().setStyle(id, withLabel(style, patch, first));
  if (!info.fields.length) return <div className="muted">Labels need an attribute field; this layer has none.</div>;
  return (
    <div className="stack">
      <Checkbox label="Label features" checked={!!label?.enabled} onChange={(_, d) => set({ enabled: !!d.checked })} />
      {label?.enabled && (
        <>
          <Field label="Label field" size="small">
            <Dropdown size="small" value={label.field} selectedOptions={[label.field]} onOptionSelect={(_, d) => set({ field: d.optionValue })}>
              {info.fields.map((f) => <Option key={f.name} value={f.name}>{f.name}</Option>)}
            </Dropdown>
          </Field>
          <FontPicker value={label.fontFamily} onChange={(fontFamily) => set({ fontFamily })} />
          <div className="row2">
            <Checkbox label="Bold" checked={!!label.bold} onChange={(_, d) => set({ bold: !!d.checked })} />
            <Checkbox label="Italic" checked={!!label.italic} onChange={(_, d) => set({ italic: !!d.checked })} />
          </div>
          <Field label={`Size ${label.size} pt`} size="small">
            <Slider min={5} max={24} step={0.5} value={label.size} onChange={(_, d) => set({ size: d.value })} />
          </Field>
          <div className="row2">
            <Field label="Text color" size="small">
              <input type="color" value={label.color} onChange={(e) => set({ color: e.target.value })} />
            </Field>
            <Field label="Halo color" size="small">
              <input type="color" value={label.haloColor} onChange={(e) => set({ haloColor: e.target.value })} />
            </Field>
          </div>
          <Field label={`Halo width ${label.haloWidth} px (0 = none)`} size="small">
            <Slider min={0} max={4} step={0.5} value={label.haloWidth} onChange={(_, d) => set({ haloWidth: d.value })} />
          </Field>
          <div className="row2">
            <Field label="Hide when zoomed out past 1:" size="small">
              <Input size="small" placeholder="no limit" defaultValue={label.maxScale ? String(label.maxScale) : ""} key={`max${label.maxScale ?? ""}`} onBlur={(e) => set({ maxScale: parseScale(e.target.value) })} />
            </Field>
            <Field label="Hide when zoomed in past 1:" size="small">
              <Input size="small" placeholder="no limit" defaultValue={label.minScale ? String(label.minScale) : ""} key={`min${label.minScale ?? ""}`} onBlur={(e) => set({ minScale: parseScale(e.target.value) })} />
            </Field>
          </div>
          <div className="muted">Overlapping labels are hidden; zoom in to see more.</div>
        </>
      )}
    </div>
  );
}

/** Searchable list of the fonts installed on this computer, each name set in its own face. */
export function FontPicker({ value, onChange }: { value?: string; onChange: (family: string | undefined) => void }) {
  const fonts = useFonts((s) => s.fonts);
  const failed = useFonts((s) => s.failed);
  const [q, setQ] = useState<string | null>(null);
  useEffect(() => void useFonts.getState().load(), []);
  const list = matchFonts(fonts ?? [], q ?? "");
  return (
    <Field label="Font" size="small" hint={failed ? "The list of installed fonts could not be read; common fonts are shown. A font this computer lacks falls back to Noto Sans." : value ? undefined : "Noto Sans (built in)"}>
      <Combobox
        size="small"
        freeform
        placeholder="Noto Sans (built in)"
        value={q ?? value ?? ""}
        selectedOptions={value ? [value] : []}
        onChange={(e) => setQ(e.target.value)}
        onOptionSelect={(_, d) => { setQ(null); onChange(d.optionValue === "__default" ? undefined : d.optionValue); }}
        onBlur={() => setQ(null)}
        data-testid="label-font"
      >
        <Option value="__default" text="Noto Sans (built in)">Noto Sans (built in)</Option>
        {list.map((f) => <Option key={f.name} value={f.name} text={f.name}><span style={{ fontFamily: `"${f.name}"` }}>{f.name}</span></Option>)}
      </Combobox>
    </Field>
  );
}
