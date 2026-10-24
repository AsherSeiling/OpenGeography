// Properties pane for a layout element (target { kind: "layoutElement" }).
import { Button, Checkbox, Dropdown, Field, Input, Option, Textarea } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { crsName, loadCatalog } from "../crs/crs";
import { useLayouts } from "../store/layout";
import { pickFrameCrs, setFrameFromMapView } from "./actions";
import type { LayoutElement, MapFrame, TextElement } from "./types";

/** A number field that keeps what the user types until it parses. */
function Num({ label, value, onChange, min, step }: { label: string; value: number; onChange: (v: number) => void; min?: number; step?: number }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText((t) => (Number(t) === value ? t : String(value))), [value]);
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
          if (d.value.trim() !== "" && Number.isFinite(v) && (min === undefined || v >= min)) onChange(v);
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
        <div className="props-grid">
          <Num label="X (mm)" value={el.x} step={0.5} onChange={(x) => set({ x }, "x")} />
          <Num label="Y (mm)" value={el.y} step={0.5} onChange={(y) => set({ y }, "y")} />
          <Num label="Width (mm)" value={el.w} min={1} step={0.5} onChange={(w) => set({ w }, "w")} />
          <Num label="Height (mm)" value={el.h} min={1} step={0.5} onChange={(h) => set({ h }, "h")} />
          <Num label="Rotation (°)" value={el.rotation} onChange={(rotation) => set({ rotation }, "rotation")} />
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
        <Num label="Centre X" value={el.center[0]} onChange={(v) => set({ center: [v, el.center[1]] }, "cx")} />
        <Num label="Centre Y" value={el.center[1]} onChange={(v) => set({ center: [el.center[0], v] }, "cy")} />
      </div>
      <Button size="small" onClick={() => void setFrameFromMapView(el.id)}>Set from Map View</Button>
      <Num label="Border width (mm, 0 = none)" value={el.border?.width ?? 0} min={0} step={0.05} onChange={(w) => set({ border: w > 0 ? { width: w, color: el.border?.color ?? "#000000" } : null }, "border")} />
    </>
  );
}

function TextFields({ el, set }: { el: TextElement; set: Set }) {
  const fonts: [TextElement["font"], string][] = [["sans", "Regular"], ["sans-bold", "Bold"], ["sans-italic", "Italic"]];
  const aligns: TextElement["align"][] = ["left", "center", "right"];
  return (
    <>
      <Field label="Text (tokens: {layout_name} {project} {date} {scale} {crs})" size="small">
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
