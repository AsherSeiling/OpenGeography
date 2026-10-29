// Properties pane for a layout element (target { kind: "layoutElement" }).
import { Button, Checkbox, Dropdown, Field, Input, Option, Textarea } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { crsName, loadCatalog } from "../crs/crs";
import { useLayouts } from "../store/layout";
import { displayInfo, useApp } from "../store/store";
import { withStyleOverride } from "./labelEdits";
import { pickFrameCrs, setFrameFromMapView } from "./actions";
import { rounded, type GridSpec, type LayoutElement, type LegendElement, type MapFrame, type NorthArrowElement, type ScaleBarElement, type SourcesElement, type TextElement } from "./types";

/** A number field that keeps what the user types until it parses. */
/** `decimals` rounds what is shown (0.1 mm for sizes and positions, 0.01 for CRS coordinates); the stored value changes only when the user types. */
export function Num({ label, value, onChange, min, max, step, decimals }: { label: string; value: number; onChange: (v: number) => void; min?: number; max?: number; step?: number; decimals?: number }) {
  const show = (v: number) => String(decimals === undefined ? v : rounded(v, decimals));
  const [text, setText] = useState(show(value));
  useEffect(() => setText((t) => (Number(t) === value ? t : show(value))), [value]);
  return (
    <Field label={label} size="small">
      <Input
        size="small"
        type="number"
        value={text}
        step={step}
        onChange={(_, d) => {
          setText(d.value);
          const v = Number(d.value);
          if (d.value.trim() !== "" && Number.isFinite(v) && (min === undefined || v >= min) && (max === undefined || v <= max)) onChange(v);
        }}
      />
    </Field>
  );
}

export function LayoutElementProperties({ layout, id }: { layout: string; id: string }) {
  const el = useLayouts((s) => (s.id === layout ? s.doc : s.docs[layout])?.elements.find((e) => e.id === id));
  const set = (patch: Record<string, unknown>, burst?: string) => useLayouts.getState().update(id, patch, burst);
  useEffect(() => void loadCatalog().catch(() => {}), []);
  if (!el) return <div className="empty-hint pad">Element not found.</div>;
  const title = { map_frame: "Map Frame", text: "Text", legend: "Legend", scale_bar: "Scale Bar", north_arrow: "North Arrow", sources: "Sources" }[el.kind];
  return (
    <div className="props">
      <div className="props-title">{title} <span className="muted">({el.id})</span></div>
      <div className="props-body stack">
        {el.kind === "map_frame" && <FrameFields el={el} set={set} />}
        {el.kind === "text" && <TextFields el={el} set={set} />}
        {el.kind === "legend" && <LegendFields el={el} set={set} />}
        {el.kind === "scale_bar" && <ScaleBarFields el={el} set={set} />}
        {el.kind === "north_arrow" && <NorthArrowFields el={el} set={set} />}
        {el.kind === "sources" && <SourcesFields el={el} set={set} />}
        <div className="props-grid">
          <Num label="X (mm)" value={el.x} step={0.5} decimals={1} onChange={(x) => set({ x }, "x")} />
          <Num label="Y (mm)" value={el.y} step={0.5} decimals={1} onChange={(y) => set({ y }, "y")} />
          <Num label="Width (mm)" value={el.w} min={1} step={0.5} decimals={1} onChange={(w) => set({ w }, "w")} />
          <Num label="Height (mm)" value={el.h} min={1} step={0.5} decimals={1} onChange={(h) => set({ h }, "h")} />
          <Num label="Rotation (°)" value={el.rotation} decimals={2} onChange={(rotation) => set({ rotation }, "rotation")} />
        </div>
        <Checkbox label="Locked" checked={el.locked} onChange={(_, d) => set({ locked: !!d.checked })} />
        <Checkbox label="Visible" checked={el.visible} onChange={(_, d) => set({ visible: !!d.checked })} />
      </div>
    </div>
  );
}

type Set = (patch: Record<string, unknown>, burst?: string) => void;

function FrameFields({ el, set }: { el: MapFrame; set: Set }) {
  return (
    <>
      <Field label="Coordinate system" size="small">
        <div className="btn-row">
          <span className="grow">{el.crs ? crsName(el.crs) : "Not set"}</span>
          <Button size="small" onClick={() => pickFrameCrs(el.id)}>Change…</Button>
        </div>
      </Field>
      <Num label="Scale 1 :" value={el.scale} min={1} step={1000} onChange={(scale) => set({ scale }, "scale")} />
      <div className="props-grid">
        <Num label="Centre X" value={el.center?.[0] ?? 0} decimals={2} onChange={(v) => set({ center: [v, el.center?.[1] ?? 0] }, "cx")} />
        <Num label="Centre Y" value={el.center?.[1] ?? 0} decimals={2} onChange={(v) => set({ center: [el.center?.[0] ?? 0, v] }, "cy")} />
      </div>
      <Button size="small" onClick={() => void setFrameFromMapView(el.id)}>Set from Map View</Button>
      <Field label="Basemap" size="small">
        <Dropdown size="small" value={BASEMAPS.find((b) => b[0] === el.basemap)?.[1] ?? "None"} selectedOptions={[el.basemap]} onOptionSelect={(_, d) => set({ basemap: d.optionValue })}>
          {BASEMAPS.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
          <Option value="osm" disabled title="OpenStreetMap's tile policy forbids bulk downloads, so it can't be used in a print layout.">OpenStreetMap (not for print)</Option>
        </Dropdown>
      </Field>
      <GridFields label="Graticule (lat/lon lines)" unit="degrees" value={el.graticule} onChange={(graticule) => set({ graticule }, "graticule")} />
      <GridFields label="Grid (map coordinates)" unit="CRS units" value={el.grid} onChange={(grid) => set({ grid }, "grid")} />
      <SeriesFollow el={el} set={set} />
      <FrameLabels el={el} set={set} />
      <Num label="Border width (mm, 0 = none)" value={el.border?.width ?? 0} min={0} step={0.05} decimals={2} onChange={(w) => set({ border: w > 0 ? { width: w, color: el.border?.color ?? "#000000" } : null }, "border")} />
    </>
  );
}

/** Whether this frame follows the layout's map series (only offered when the layout has one). */
function SeriesFollow({ el, set }: { el: MapFrame; set: Set }) {
  const has = useLayouts((s) => !!s.doc?.series);
  if (!has && !el.followsSeries) return null;
  return <Checkbox label="Follows the map series" checked={!!el.followsSeries} onChange={(_, d) => set({ followsSeries: d.checked ? true : undefined })} />;
}

/** Labels in this layout: per layer, follow the map's label setting or show or hide them here (L3). Edits to single labels are made by clicking a label in the page. */
function FrameLabels({ el, set }: { el: MapFrame; set: Set }) {
  const order = useApp((s) => s.order);
  const layers = useApp((s) => s.layers);
  const results = useApp((s) => s.results);
  const showOriginal = useApp((s) => s.showOriginal);
  const styles = useApp((s) => s.styles);
  const ids = (el.layers === "map" ? order : el.layers.filter((l) => l.visible).map((l) => l.id)).filter((id) => {
    const g = displayInfo({ layers, results, showOriginal }, id)?.geomType;
    return g === "Point" || g === "Line" || g === "Polygon" || g === "Mixed";
  });
  if (!ids.length) return null;
  return (
    <Field label="Labels in this layout" size="small">
      <div className="stack">
        {ids.map((id) => {
          const info = displayInfo({ layers, results, showOriginal }, id)!;
          const ov = el.labelOverrides?.[id];
          const patch = ov?.style;
          const mapLabel = styles[id]?.label;
          const mode = patch?.enabled === undefined ? "map" : patch.enabled ? "show" : "hide";
          const shown = mode === "show" || (mode === "map" && !!mapLabel?.enabled);
          const field = patch?.field ?? mapLabel?.field ?? info.fields[0]?.name ?? "";
          const edit = (p: Parameters<typeof withStyleOverride>[2]) => set({ labelOverrides: withStyleOverride(el.labelOverrides, id, p) }, `labels:${id}`);
          return (
            <div key={id} className="stack">
              <div className="btn-row">
                <span className="grow">{info.name}</span>
                <Dropdown size="small" value={{ map: "Map setting", show: "Show", hide: "Hide" }[mode]} selectedOptions={[mode]} onOptionSelect={(_, d) => edit(d.optionValue === "map" ? { enabled: undefined } : { enabled: d.optionValue === "show", ...(d.optionValue === "show" ? { field } : {}) })}>
                  <Option value="map">Map setting</Option>
                  <Option value="show">Show</Option>
                  <Option value="hide">Hide</Option>
                </Dropdown>
              </div>
              {shown && (
                <div className="props-grid">
                  <Field label="Field" size="small">
                    <Dropdown size="small" value={field} selectedOptions={[field]} onOptionSelect={(_, d) => edit({ field: d.optionValue })}>
                      {info.fields.map((f) => <Option key={f.name} value={f.name}>{f.name}</Option>)}
                    </Dropdown>
                  </Field>
                  <Num label="Size (pt)" value={patch?.size ?? mapLabel?.size ?? 9} min={1} step={0.5} decimals={1} onChange={(size) => edit({ size })} />
                  <Field label="Text color" size="small">
                    <input type="color" value={patch?.color ?? mapLabel?.color ?? "#222222"} onChange={(e) => edit({ color: e.target.value })} />
                  </Field>
                </div>
              )}
              {ov && <Button size="small" onClick={() => set({ labelOverrides: (() => { const { [id]: _drop, ...rest } = el.labelOverrides ?? {}; return Object.keys(rest).length ? rest : undefined; })() })}>Reset labels of {info.name} to the map's</Button>}
            </div>
          );
        })}
      </div>
    </Field>
  );
}

const BASEMAPS: [MapFrame["basemap"], string][] = [["none", "None"], ["usgsTopo", "USGS Topo"], ["usgsImagery", "USGS Imagery"], ["usgsRelief", "USGS Shaded Relief"]];

/** Graticule/grid: off, or on with an interval ("auto" or a number) and edge labels. */
function GridFields({ label, unit, value, onChange }: { label: string; unit: string; value: GridSpec | null; onChange: (v: GridSpec | null) => void }) {
  const [text, setText] = useState(value ? String(value.interval) : "auto");
  useEffect(() => setText((t) => (value && (value.interval === "auto" ? t.trim() === "auto" : Number(t) === value.interval) ? t : value ? String(value.interval) : t)), [value?.interval]);
  return (
    <>
      <Checkbox label={label} checked={!!value} onChange={(_, d) => onChange(d.checked ? { interval: "auto", labels: true } : null)} />
      {value && (
        <div className="props-grid">
          <Field label={`Interval (${unit})`} size="small">
            <Input
              size="small"
              value={text}
              onChange={(_, d) => {
                setText(d.value);
                const t = d.value.trim();
                const v = Number(t);
                if (t.toLowerCase() === "auto") onChange({ ...value, interval: "auto" });
                else if (t !== "" && Number.isFinite(v) && v > 0) onChange({ ...value, interval: v });
              }}
            />
          </Field>
          <Checkbox label="Labels" checked={value.labels} onChange={(_, d) => onChange({ ...value, labels: !!d.checked })} />
        </div>
      )}
    </>
  );
}

function Choice<T extends string>({ label, value, options, onPick }: { label: string; value: T; options: [T, string][]; onPick: (v: T) => void }) {
  return (
    <Field label={label} size="small">
      <Dropdown size="small" value={options.find((o) => o[0] === value)?.[1]} selectedOptions={[value]} onOptionSelect={(_, d) => onPick(d.optionValue as T)}>
        {options.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
      </Dropdown>
    </Field>
  );
}

function LegendFields({ el, set }: { el: LegendElement; set: Set }) {
  return (
    <>
      <Field label="Title" size="small">
        <Input size="small" value={el.title} onChange={(_, d) => set({ title: d.value }, "title")} />
      </Field>
      <Num label="Columns" value={el.columns || 1} min={1} max={6} step={1} onChange={(columns) => set({ columns: Math.round(columns) }, "columns")} />
    </>
  );
}

function ScaleBarFields({ el, set }: { el: ScaleBarElement; set: Set }) {
  const units: [ScaleBarElement["units"], string][] = [["km", "Kilometres"], ["mi", "Miles"], ["m", "Metres"], ["ft", "Feet"]];
  return (
    <>
      <Choice label="Units" value={el.units || "km"} options={units} onPick={(units) => set({ units })} />
      <Num label="Segments" value={el.segments || 4} min={1} max={10} step={1} onChange={(segments) => set({ segments: Math.round(segments) })} />
      <Checkbox label="Also print the ratio (1:N) under the bar" checked={!!el.ratio} onChange={(_, d) => set({ ratio: d.checked ? true : undefined })} />
    </>
  );
}

function NorthArrowFields({ el, set }: { el: NorthArrowElement; set: Set }) {
  return <Choice label="Style" value={el.style || "simple"} options={[["simple", "Simple"], ["compass", "Compass"]]} onPick={(style) => set({ style })} />;
}

function SourcesFields({ el, set }: { el: SourcesElement; set: Set }) {
  return <Num label="Text size (pt)" value={el.size || 6} min={1} step={0.5} decimals={1} onChange={(size) => set({ size }, "size")} />;
}

function TextFields({ el, set }: { el: TextElement; set: Set }) {
  const fonts: [TextElement["font"], string][] = [["sans", "Regular"], ["sans-bold", "Bold"], ["sans-italic", "Italic"]];
  const aligns: TextElement["align"][] = ["left", "center", "right"];
  return (
    <>
      <Field label="Text (tokens: {layout_name} {project} {date} {scale} {crs} {datum} {units})" size="small">
        <Textarea size="small" value={el.text} rows={3} onChange={(_, d) => set({ text: d.value }, "text")} />
      </Field>
      <Field label="Font" size="small">
        <Dropdown size="small" value={fonts.find((f) => f[0] === el.font)?.[1]} selectedOptions={[el.font]} onOptionSelect={(_, d) => set({ font: d.optionValue })}>
          {fonts.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
        </Dropdown>
      </Field>
      <Num label="Size (pt)" value={el.size} min={1} step={1} onChange={(size) => set({ size }, "size")} />
      <Field label="Align" size="small">
        <Dropdown size="small" value={el.align} selectedOptions={[el.align]} onOptionSelect={(_, d) => set({ align: d.optionValue })}>
          {aligns.map((a) => <Option key={a} value={a}>{a}</Option>)}
        </Dropdown>
      </Field>
      <Field label="Colour" size="small">
        <input type="color" value={el.color} onChange={(e) => set({ color: e.target.value }, "color")} />
      </Field>
    </>
  );
}

export type { LayoutElement };
