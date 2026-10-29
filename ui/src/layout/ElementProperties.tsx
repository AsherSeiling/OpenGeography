// Properties pane for a layout element (target { kind: "layoutElement" }).
import { Button, Checkbox, Dropdown, Field, Input, Option, Textarea } from "@fluentui/react-components";
import { FontPicker } from "../panes/LabelSettings";
import { useEffect, useState } from "react";
import { crsName, loadCatalog } from "../crs/crs";
import { useLayouts } from "../store/layout";
import { displayInfo, useApp } from "../store/store";
import { withStyleOverride } from "./labelEdits";
import { useLegendRows, withOverride } from "./legendRows";
import { choosePicture, choosePictureFor, pickFrameCrs, setFrameFromMapView } from "./actions";
import { rounded, type GridSpec, type LayoutElement, type LegendElement, type MapFrame, type NorthArrowElement, type PictureElement, type ScaleBarElement, type ShapeElement, type SourcesElement, type TextElement } from "./types";

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
  const title = { map_frame: "Map Frame", text: "Text", legend: "Legend", scale_bar: "Scale Bar", north_arrow: "North Arrow", sources: "Sources", shape: "Shape", picture: "Picture" }[el.kind];
  return (
    <div className="props">
      <div className="props-title">{title} <span className="muted">({el.id})</span></div>
      <div className="props-body stack">
        {el.kind === "map_frame" && <FrameFields el={el} set={set} layout={layout} />}
        {el.kind === "text" && <TextFields el={el} set={set} />}
        {el.kind === "legend" && <LegendFields el={el} set={set} layout={layout} />}
        {el.kind === "scale_bar" && <ScaleBarFields el={el} set={set} />}
        {el.kind === "north_arrow" && <NorthArrowFields el={el} set={set} />}
        {el.kind === "sources" && <SourcesFields el={el} set={set} />}
        {el.kind === "shape" && <ShapeFields el={el} set={set} />}
        {el.kind === "picture" && <PictureFields el={el} set={set} />}
        <div className="props-grid">
          <Num label="X (mm)" value={el.x} step={0.5} decimals={1} onChange={(x) => set({ x }, "x")} />
          <Num label="Y (mm)" value={el.y} step={0.5} decimals={1} onChange={(y) => set({ y }, "y")} />
          <Num label="Width (mm)" value={el.w} min={1} step={0.5} decimals={1} onChange={(w) => set({ w }, "w")} />
          <Num label="Height (mm)" value={el.h} min={1} step={0.5} decimals={1} onChange={(h) => set({ h }, "h")} />
          <Num label="Rotation (°)" value={el.rotation} decimals={2} onChange={(rotation) => set({ rotation }, "rotation")} />
        </div>
        <DecorFields el={el} set={set} />
        <Checkbox label="Locked" checked={el.locked} onChange={(_, d) => set({ locked: !!d.checked })} />
        <Checkbox label="Visible" checked={el.visible} onChange={(_, d) => set({ visible: !!d.checked })} />
      </div>
    </div>
  );
}

type Set = (patch: Record<string, unknown>, burst?: string) => void;

function FrameFields({ el, set, layout }: { el: MapFrame; set: Set; layout: string }) {
  return (
    <>
      <Field label="Coordinate system" size="small">
        <div className="btn-row">
          <span className="grow">{el.crs ? crsName(el.crs) : "Not set"}</span>
          <Button size="small" onClick={() => pickFrameCrs(el.id)}>Change…</Button>
        </div>
      </Field>
      <Num label="Scale 1 :" value={el.scale} min={1} step={1000} onChange={(scale) => set({ scale }, "scale")} />
      <OverviewFields el={el} set={set} layout={layout} />
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

function LegendFields({ el, set, layout }: { el: LegendElement; set: Set; layout: string }) {
  const rows = useLegendRows((s) => s.byLayout[layout])?.filter((r) => r.legend === el.id) ?? [];
  const edit = (key: string, patch: { label?: string; hidden?: boolean }) => set({ overrides: withOverride(el.overrides, key, patch) });
  const custom = el.custom ?? [];
  return (
    <>
      <Field label="Title" size="small">
        <Input size="small" value={el.title} onChange={(_, d) => set({ title: d.value }, "title")} />
      </Field>
      <Num label="Columns" value={el.columns || 1} min={1} max={6} step={1} onChange={(columns) => set({ columns: Math.round(columns) }, "columns")} />
      <Num label="Entry text size (pt, 0 = 8)" value={el.textSize ?? 0} min={0} max={24} step={0.5} decimals={1} onChange={(textSize) => set({ textSize }, "textSize")} />
      <Choice label="Area swatch shape" value={el.patch || "square"} options={[["square", "Square"], ["rounded", "Rounded"], ["circle", "Circle"]]} onPick={(patch) => set({ patch })} />
      <details open>
        <summary>Entries ({rows.length})</summary>
        <div className="props-hint">Untick to hide an entry (a layer's name hides all its entries); type to rename.</div>
        <div className="stack">
          {rows.map((r) => (
            <div key={r.key} className="class-row" style={{ paddingLeft: r.layerRow ? 0 : 16 }}>
              <Checkbox checked={!(el.overrides?.[r.key]?.hidden)} onChange={(_, d) => edit(r.key, { hidden: !d.checked })} aria-label={`Show ${r.label}`} />
              <Input size="small" value={el.overrides?.[r.key]?.label ?? ""} placeholder={r.label} onChange={(_, d) => edit(r.key, { label: d.value })} aria-label={`Name of ${r.label}`} />
            </div>
          ))}
          {!rows.length && <div className="props-hint">Entries appear once the preview has drawn.</div>}
        </div>
      </details>
      <details>
        <summary>Extra entries ({custom.length})</summary>
        <div className="stack">
          {custom.map((c, i) => (
            <div key={i} className="class-row">
              <input type="color" value={/^#[0-9a-f]{6}$/i.test(c.color) ? c.color : "#888888"} aria-label={`Colour of extra entry ${i + 1}`} onChange={(e) => set({ custom: custom.map((x, j) => (j === i ? { ...x, color: e.target.value } : x)) })} />
              <Input size="small" value={c.label} aria-label={`Text of extra entry ${i + 1}`} onChange={(_, d) => set({ custom: custom.map((x, j) => (j === i ? { ...x, label: d.value } : x)) })} />
              <Button size="small" appearance="subtle" aria-label={`Remove extra entry ${i + 1}`} onClick={() => set({ custom: custom.filter((_, j) => j !== i) })}>✕</Button>
            </div>
          ))}
          <Button size="small" onClick={() => set({ custom: [...custom, { label: "New entry", color: "#888888" }] })}>Add entry</Button>
        </div>
      </details>
    </>
  );
}

/** Overview/inset map: show where another frame looks. */
function OverviewFields({ el, set, layout }: { el: MapFrame; set: Set; layout: string }) {
  const all = useLayouts((s) => (s.id === layout ? s.doc : s.docs[layout])?.elements);
  const others = (all ?? []).filter((e): e is MapFrame => e.kind === "map_frame" && e.id !== el.id);
  if (!others.length) return null;
  const ind = el.indicator;
  return (
    <details open={!!ind}>
      <summary>Overview map (show another frame's extent)</summary>
      <div className="stack">
        <Choice label="Show the extent of" value={ind?.frame ?? ""} options={[["", "Nothing"], ...others.map((o): [string, string] => [o.id, `Frame ${o.id}`])]} onPick={(frame) => set({ indicator: frame ? { ...(ind ?? {}), frame } : null })} />
        {ind && (
          <>
            <ColorInput label="Outline colour" value={ind.color ?? "#d95f02"} onChange={(color) => set({ indicator: { ...ind, color } }, "indColor")} />
            <Num label="Outline width (mm)" value={ind.width ?? 0.6} min={0.05} step={0.05} decimals={2} onChange={(width) => set({ indicator: { ...ind, width } }, "indWidth")} />
            <Num label="Fill opacity (0–1)" value={ind.fillOpacity ?? 0} min={0} max={1} step={0.05} decimals={2} onChange={(fillOpacity) => set({ indicator: { ...ind, fillOpacity } }, "indFill")} />
          </>
        )}
      </div>
    </details>
  );
}

function ScaleBarFields({ el, set }: { el: ScaleBarElement; set: Set }) {
  const units: [ScaleBarElement["units"], string][] = [["km", "Kilometres"], ["mi", "Miles"], ["m", "Metres"], ["ft", "Feet"]];
  const styles: [NonNullable<ScaleBarElement["style"]>, string][] = [["alternating", "Alternating blocks"], ["hollow", "Hollow blocks"], ["line", "Line with ticks"], ["ticks", "Line with tall ticks"], ["double", "Double (two units)"]];
  return (
    <>
      <Choice label="Style" value={el.style || "alternating"} options={styles} onPick={(style) => set({ style })} />
      <Choice label="Units" value={el.units || "km"} options={units} onPick={(units) => set({ units })} />
      {el.style === "double" && <Choice label="Second bar units" value={el.units2 || "mi"} options={units} onPick={(units2) => set({ units2 })} />}
      <Num label="Segments" value={el.segments || 4} min={1} max={10} step={1} onChange={(segments) => set({ segments: Math.round(segments) })} />
      <ColorInput label="Colour" value={el.color || "#000000"} onChange={(color) => set({ color }, "color")} />
      <Checkbox label="Also print the ratio (1:N) under the bar" checked={!!el.ratio} onChange={(_, d) => set({ ratio: d.checked ? true : undefined })} />
      <Checkbox label="Also print the sentence (1 cm on the map = …)" checked={!!el.verbal} onChange={(_, d) => set({ verbal: d.checked ? true : undefined })} />
    </>
  );
}

function NorthArrowFields({ el, set }: { el: NorthArrowElement; set: Set }) {
  const styles: [NorthArrowElement["style"], string][] = [["simple", "Simple kite"], ["compass", "Compass star"], ["rose", "Compass rose (N E S W)"], ["arrow", "Arrow"], ["triangle", "Triangle"], ["half", "Half arrow (topographic)"], ["circle", "Arrow in circle"], ["custom", "Custom picture"]];
  return (
    <>
      <Choice label="Style" value={el.style || "simple"} options={styles} onPick={(style) => set({ style })} />
      <Choice label="Points to" value={el.north || "true"} options={[["true", "True north (follows meridian convergence)"], ["grid", "Grid north (map's up)"]]} onPick={(north) => set({ north })} />
      {el.style === "custom" ? (
        <>
          <Button onClick={() => void choosePictureFor(el.id)}>{el.source ? "Replace picture…" : "Choose picture…"}</Button>
          <div className="props-hint">The top of the picture must point north.</div>
        </>
      ) : (
        <ColorInput label="Colour" value={el.color || "#000000"} onChange={(color) => set({ color }, "color")} />
      )}
    </>
  );
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
      <FontPicker value={el.family} onChange={(family) => set({ family })} />
      <Field label="Style" size="small">
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

type SetFn = (patch: Record<string, unknown>, burst?: string) => void;

function ColorInput({ label, value, onChange }: { label: string; value: string; onChange: (c: string) => void }) {
  return (
    <Field label={label} size="small">
      <input type="color" value={/^#[0-9a-f]{6}$/i.test(value) ? value : "#000000"} onChange={(e) => onChange(e.target.value)} aria-label={label} />
    </Field>
  );
}

function ShapeFields({ el, set }: { el: ShapeElement; set: SetFn }) {
  const line = el.shape === "line" || el.shape === "arrow";
  return (
    <>
      <Field label="Shape" size="small">
        <Dropdown value={el.shape} selectedOptions={[el.shape]} onOptionSelect={(_, d) => set({ shape: d.optionValue })}>
          {["rectangle", "ellipse", "triangle", "line", "arrow"].map((k) => (
            <Option key={k} value={k}>
              {k}
            </Option>
          ))}
        </Dropdown>
      </Field>
      {!line && (
        <>
          <Checkbox label="Filled" checked={el.fill !== "none"} onChange={(_, d) => set({ fill: d.checked ? "#ffffff" : "none" })} />
          {el.fill !== "none" && <ColorInput label="Fill" value={el.fill} onChange={(fill) => set({ fill }, "fill")} />}
          {el.fill !== "none" && <Num label="Fill opacity (0–1)" value={el.fillOpacity ?? 1} min={0} max={1} step={0.05} decimals={2} onChange={(fillOpacity) => set({ fillOpacity }, "fillOpacity")} />}
        </>
      )}
      <ColorInput label={line ? "Colour" : "Outline"} value={el.stroke} onChange={(stroke) => set({ stroke }, "stroke")} />
      <Num label="Line width (mm)" value={el.strokeWidth} min={0} step={0.05} decimals={2} onChange={(strokeWidth) => set({ strokeWidth }, "strokeWidth")} />
      <Field label="Line style" size="small">
        <Dropdown value={el.dash || "solid"} selectedOptions={[el.dash || "solid"]} onOptionSelect={(_, d) => set({ dash: d.optionValue })}>
          {["solid", "dashed", "dotted"].map((k) => (
            <Option key={k} value={k}>
              {k}
            </Option>
          ))}
        </Dropdown>
      </Field>
      {el.shape === "rectangle" && <Num label="Corner radius (mm)" value={el.cornerRadius ?? 0} min={0} step={0.5} decimals={1} onChange={(cornerRadius) => set({ cornerRadius }, "cornerRadius")} />}
    </>
  );
}

function PictureFields({ el, set }: { el: PictureElement; set: SetFn }) {
  return (
    <>
      <Button onClick={() => void choosePicture(el.id)}>{el.source ? "Replace picture…" : "Choose picture…"}</Button>
      <div className="props-hint">PNG, JPEG or SVG; stored inside the layout, so the project stays self-contained.</div>
      <Field label="Fit" size="small">
        <Dropdown value={el.fit || "contain"} selectedOptions={[el.fit || "contain"]} onOptionSelect={(_, d) => set({ fit: d.optionValue })}>
          {[["contain", "Whole picture inside"], ["cover", "Fill the frame (crop)"], ["stretch", "Stretch"]].map(([k, v]) => (
            <Option key={k} value={k} text={v}>
              {v}
            </Option>
          ))}
        </Dropdown>
      </Field>
      <Num label="Opacity (0–1)" value={el.opacity ?? 1} min={0} max={1} step={0.05} decimals={2} onChange={(opacity) => set({ opacity }, "opacity")} />
    </>
  );
}

/** Background, frame and drop shadow, for any element. */
function DecorFields({ el, set }: { el: LayoutElement; set: SetFn }) {
  const d = el.decor ?? {};
  const patch = (p: Record<string, unknown>) => set({ decor: { ...d, ...p } }, "decor");
  return (
    <details>
      <summary>Background, frame and shadow</summary>
      <div className="stack">
        <Checkbox label="Background" checked={!!d.background} onChange={(_, c) => patch({ background: c.checked ? "#ffffff" : null })} />
        {d.background && <ColorInput label="Background colour" value={d.background} onChange={(background) => patch({ background })} />}
        <Checkbox label="Frame" checked={(d.frameWidth ?? 0) > 0} onChange={(_, c) => patch({ frameWidth: c.checked ? 0.35 : 0, frameColor: d.frameColor ?? "#000000" })} />
        {(d.frameWidth ?? 0) > 0 && (
          <>
            <ColorInput label="Frame colour" value={d.frameColor ?? "#000000"} onChange={(frameColor) => patch({ frameColor })} />
            <Num label="Frame width (mm)" value={d.frameWidth ?? 0.35} min={0.05} step={0.05} decimals={2} onChange={(frameWidth) => patch({ frameWidth })} />
          </>
        )}
        <Num label="Corner radius (mm)" value={d.cornerRadius ?? 0} min={0} step={0.5} decimals={1} onChange={(cornerRadius) => patch({ cornerRadius })} />
        <Checkbox label="Drop shadow" checked={!!d.shadow} onChange={(_, c) => patch({ shadow: !!c.checked, shadowOffset: d.shadowOffset ?? 1.5 })} />
        {d.shadow && <Num label="Shadow offset (mm)" value={d.shadowOffset ?? 1.5} min={0} step={0.25} decimals={2} onChange={(shadowOffset) => patch({ shadowOffset })} />}
      </div>
    </details>
  );
}
