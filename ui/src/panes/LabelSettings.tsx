// The Labels section of a layer's Properties (L1): which field to show, how it looks, and the scales it shows at.
import { Accordion, AccordionHeader, AccordionItem, AccordionPanel, Checkbox, Combobox, Dropdown, Field, Input, Option, Slider } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { matchFonts, useFonts } from "../map/fonts";
import { displayInfo, defaultLabel, useApp, type LabelPosition, type LabelStyle, type LayerStyle } from "../store/store";

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

const POSITIONS: [LabelPosition, string][] = [
  ["aboveLeft", "Above left"], ["above", "Above"], ["aboveRight", "Above right"],
  ["left", "Left"], ["center", "Centre"], ["right", "Right"],
  ["belowLeft", "Below left"], ["below", "Below"], ["belowRight", "Below right"],
];

/** A field picker with a "(none)" entry. */
function OptField({ label, value, fields, onChange, hint }: { label: string; value?: string; fields: string[]; onChange: (f: string | undefined) => void; hint?: string }) {
  return (
    <Field label={label} size="small" hint={hint}>
      <Dropdown size="small" value={value ?? "(none)"} selectedOptions={[value ?? ""]} onOptionSelect={(_, d) => onChange(d.optionValue || undefined)}>
        <Option value="" text="(none)">(none)</Option>
        {fields.map((f) => <Option key={f} value={f}>{f}</Option>)}
      </Dropdown>
    </Field>
  );
}

export function LabelSettings({ id }: { id: string }) {
  const info = useApp((s) => displayInfo(s, id));
  const style = useApp((s) => s.styles[id]);
  if (!info || !style || info.geomType === "Raster" || info.geomType === "Table") return null;
  const first = info.fields[0]?.name ?? "";
  const label = style.label;
  const set = (patch: Partial<LabelStyle>) => useApp.getState().setStyle(id, withLabel(style, patch, first));
  if (!info.fields.length) return <div className="muted">Labels need an attribute field; this layer has none.</div>;
  const names = info.fields.map((f) => f.name);
  const geom = info.geomType;
  return (
    <div className="stack">
      <Checkbox label="Label features" checked={!!label?.enabled} onChange={(_, d) => set({ enabled: !!d.checked })} />
      {label?.enabled && (
        <Accordion multiple collapsible defaultOpenItems={["text"]}>
          <AccordionItem value="text">
            <AccordionHeader size="small">Text</AccordionHeader>
            <AccordionPanel className="stack">
              <Field label="Label field" size="small">
                <Dropdown size="small" value={label.field} selectedOptions={[label.field]} onOptionSelect={(_, d) => set({ field: d.optionValue })}>
                  {names.map((f) => <Option key={f} value={f}>{f}</Option>)}
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
              <Field label="Text color" size="small">
                <input type="color" value={label.color} onChange={(e) => set({ color: e.target.value })} />
              </Field>
              <div className="row2">
                <Field label="Wrap at (characters)" size="small" hint="Break lines at spaces; empty = no wrap">
                  <Input size="small" type="number" min={1} value={label.wrapAt ? String(label.wrapAt) : ""} onChange={(_, d) => set({ wrapAt: d.value ? Math.max(1, Math.round(Number(d.value))) : undefined })} />
                </Field>
                <Field label="Multi-line alignment" size="small">
                  <Dropdown size="small" value={label.align ?? "center"} selectedOptions={[label.align ?? "center"]} onOptionSelect={(_, d) => set({ align: d.optionValue as never })}>
                    {["left", "center", "right"].map((a) => <Option key={a} value={a}>{a}</Option>)}
                  </Dropdown>
                </Field>
              </div>
            </AccordionPanel>
          </AccordionItem>

          <AccordionItem value="placement">
            <AccordionHeader size="small">Placement</AccordionHeader>
            <AccordionPanel className="stack">
              {geom === "Point" && (
                <Field label="Position around the point" size="small">
                  <div className="quadrants" role="radiogroup" aria-label="Position around the point">
                    {POSITIONS.map(([v, l]) => (
                      <button key={v} role="radio" aria-checked={(label.position ?? "right") === v} className={`quad ${(label.position ?? "right") === v ? "on" : ""}`} title={l} aria-label={l} onClick={() => set({ position: v })} />
                    ))}
                  </div>
                </Field>
              )}
              {geom === "Line" && (
                <Field label="Along the line" size="small" hint="Text follows the line">
                  <Dropdown size="small" value={label.linePos ?? "on"} selectedOptions={[label.linePos ?? "on"]} onOptionSelect={(_, d) => set({ linePos: d.optionValue as never })}>
                    <Option value="above">Above the line</Option>
                    <Option value="on">On the line</Option>
                    <Option value="below">Below the line</Option>
                  </Dropdown>
                </Field>
              )}
              {geom === "Polygon" && <div className="muted">Polygon labels are centred on a point inside the shape.</div>}
              {geom !== "Polygon" && (
                <Field label={`Distance from ${geom === "Point" ? "the symbol" : "the line"} ${label.distance ?? 0} px`} size="small">
                  <Slider min={0} max={30} step={1} value={label.distance ?? 0} onChange={(_, d) => set({ distance: d.value || undefined })} />
                </Field>
              )}
              <div className="row2">
                <Field label="Hide when zoomed out past 1:" size="small">
                  <Input size="small" placeholder="no limit" defaultValue={label.maxScale ? String(label.maxScale) : ""} key={`max${label.maxScale ?? ""}`} onBlur={(e) => set({ maxScale: parseScale(e.target.value) })} />
                </Field>
                <Field label="Hide when zoomed in past 1:" size="small">
                  <Input size="small" placeholder="no limit" defaultValue={label.minScale ? String(label.minScale) : ""} key={`min${label.minScale ?? ""}`} onBlur={(e) => set({ minScale: parseScale(e.target.value) })} />
                </Field>
              </div>
            </AccordionPanel>
          </AccordionItem>

          <AccordionItem value="look">
            <AccordionHeader size="small">Halo, background and shadow</AccordionHeader>
            <AccordionPanel className="stack">
              <div className="row2">
                <Field label="Halo color" size="small"><input type="color" value={label.haloColor} onChange={(e) => set({ haloColor: e.target.value })} /></Field>
                <Field label={`Halo width ${label.haloWidth} px (0 = none)`} size="small"><Slider min={0} max={4} step={0.5} value={label.haloWidth} onChange={(_, d) => set({ haloWidth: d.value })} /></Field>
              </div>
              <Checkbox label="Background box" checked={!!label.background} onChange={(_, d) => set({ background: d.checked ? { fill: "#ffffe0", stroke: "#999999", padding: 2 } : undefined })} />
              {label.background && (
                <div className="row2">
                  <Field label="Fill" size="small"><input type="color" value={label.background.fill} onChange={(e) => set({ background: { ...label.background!, fill: e.target.value } })} /></Field>
                  <Field label="Outline" size="small"><input type="color" value={label.background.stroke ?? "#999999"} onChange={(e) => set({ background: { ...label.background!, stroke: e.target.value } })} /></Field>
                  <Field label={`Padding ${label.background.padding} px`} size="small"><Slider min={0} max={10} step={1} value={label.background.padding} onChange={(_, d) => set({ background: { ...label.background!, padding: d.value } })} /></Field>
                </div>
              )}
              <Checkbox label="Drop shadow" checked={!!label.shadow} onChange={(_, d) => set({ shadow: d.checked ? { color: "#00000080", dx: 1.5, dy: 1.5 } : undefined })} />
              {label.shadow && (
                <div className="row2">
                  <Field label="Shadow color" size="small"><input type="color" value={label.shadow.color.slice(0, 7)} onChange={(e) => set({ shadow: { ...label.shadow!, color: e.target.value } })} /></Field>
                  <Field label={`Offset ${label.shadow.dx} px`} size="small"><Slider min={0} max={6} step={0.5} value={label.shadow.dx} onChange={(_, d) => set({ shadow: { ...label.shadow!, dx: d.value, dy: d.value } })} /></Field>
                </div>
              )}
            </AccordionPanel>
          </AccordionItem>

          <AccordionItem value="rendering">
            <AccordionHeader size="small">Rendering and data-defined</AccordionHeader>
            <AccordionPanel className="stack">
              <Field label={`Priority ${label.priority ?? 5} (0 lowest, 10 highest)`} size="small" hint="When labels collide the higher priority keeps its place">
                <Slider min={0} max={10} step={1} value={label.priority ?? 5} onChange={(_, d) => set({ priority: d.value === 5 ? undefined : d.value })} />
              </Field>
              <Checkbox label="Allow overlap (draw even where labels collide)" checked={!!label.allowOverlap} onChange={(_, d) => set({ allowOverlap: d.checked ? true : undefined })} />
              <OptField label="Rotation from field (degrees clockwise)" value={label.rotationField} fields={names} onChange={(rotationField) => set({ rotationField })} />
              <OptField label="Size from field (pt)" value={label.sizeField} fields={names} onChange={(sizeField) => set({ sizeField })} />
              <OptField label="Colour from field (#rrggbb)" value={label.colorField} fields={names} onChange={(colorField) => set({ colorField })} />
            </AccordionPanel>
          </AccordionItem>
        </Accordion>
      )}
      {label?.enabled && <div className="muted">Overlapping labels are hidden (higher priority first); zoom in to see more. Settings follow QGIS's Labels panel.</div>}
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
