// The Text section of a layout element's Properties: one entry per text slot the element draws (the list comes from the engine,
// `layoutTextSlots`), each with size in points, font, bold, italic, underline, colour, letter spacing and halo.
import { Button, Checkbox, Field, Input } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { FontPicker } from "../panes/LabelSettings";
import { api, type TextSlot } from "../ipc/api";
import { parseSize, resetSlot, withSlotStyle } from "./textStyles";
import type { LayoutElement, TextStyle } from "./types";

let cache: TextSlot[] | null = null;
let pending: Promise<TextSlot[]> | null = null;
/** The engine's slot list, fetched once. */
export function loadSlots(): Promise<TextSlot[]> {
  if (cache) return Promise.resolve(cache);
  pending ??= api.layoutTextSlots().then((s) => (cache = s)).catch(() => { pending = null; return [] as TextSlot[]; });
  return pending;
}

type SetFn = (patch: Record<string, unknown>, burst?: string) => void;

/** The size box: empty = the default, shown as the placeholder. */
function SizeBox({ value, placeholder, onChange, label }: { value: number | undefined; placeholder: string; onChange: (v: number | undefined) => void; label: string }) {
  const [text, setText] = useState(value === undefined ? "" : String(value));
  useEffect(() => setText((t) => (parseSize(t) === value ? t : value === undefined ? "" : String(value))), [value]);
  return (
    <Field label={label} size="small">
      <Input
        size="small"
        type="number"
        aria-label={label}
        value={text}
        placeholder={placeholder}
        step={0.5}
        onChange={(_, d) => {
          setText(d.value);
          const p = parseSize(d.value);
          if (p !== null) onChange(p);
        }}
      />
    </Field>
  );
}

/** The base size of a slot when the element keeps its own size field (`size`, `textSize`): what the slot uses unless it is overridden. */
function baseSize(el: LayoutElement, slot: TextSlot): number {
  const own = (el as { size?: number; textSize?: number }).size ?? (el as { textSize?: number }).textSize;
  if (slot.legacySize && typeof own === "number" && own > 0) {
    return slot.id === "legend.title" || slot.id === "chart.title" ? own * (slot.id === "chart.title" ? 1.2 : 1.25) : own;
  }
  return slot.defaultPt;
}

export function TextSection({ el, set }: { el: LayoutElement; set: SetFn }) {
  const [slots, setSlots] = useState<TextSlot[]>(cache ?? []);
  useEffect(() => { loadSlots().then(setSlots); }, []);
  // The Text element keeps its own size, font and colour controls (they are its one slot).
  const mine = slots.filter((s) => s.kind === el.kind && !(s.id === "text.text"));
  if (!mine.length) return null;
  const change = (slot: string, patch: Partial<Record<keyof TextStyle, unknown>>, burst?: string) => set({ textStyles: withSlotStyle(el.textStyles, slot, patch) }, burst ?? `text:${slot}`);
  return (
    <div className="stack" data-testid="text-slots">
      {mine.map((s) => {
        const st = el.textStyles?.[s.id] ?? {};
        const def = baseSize(el, s);
        const effective = st.size ?? def;
        return (
          <details key={s.id} className="text-slot" data-slot={s.id} open={mine.length <= 2 || !!el.textStyles?.[s.id]}>
            <summary>
              {s.label} <span className="muted">· {s.defaultPt === 0 && st.size === undefined ? "auto size" : `${Math.round(effective * 10) / 10} pt`}</span>
            </summary>
            <div className="stack">
              <SizeBox label={`${s.label} size (pt)`} value={st.size} placeholder={s.defaultPt === 0 ? "auto" : String(Math.round(def * 10) / 10)} onChange={(size) => change(s.id, { size })} />
              <FontPicker value={st.family} onChange={(family) => change(s.id, { family })} />
              <div className="row2">
                <Checkbox label="Bold" checked={st.bold ?? s.bold} onChange={(_, d) => change(s.id, { bold: !!d.checked })} />
                <Checkbox label="Italic" checked={!!st.italic} onChange={(_, d) => change(s.id, { italic: !!d.checked })} />
                <Checkbox label="Underline" checked={!!st.underline} onChange={(_, d) => change(s.id, { underline: !!d.checked })} />
              </div>
              <Field label="Colour" size="small">
                <input type="color" aria-label={`${s.label} colour`} value={st.color ?? "#000000"} onChange={(e) => change(s.id, { color: e.target.value }, `text:${s.id}:color`)} />
              </Field>
              <div className="row2">
                <Field label="Letter spacing (mm)" size="small">
                  <Input size="small" type="number" step={0.1} value={st.letterSpacing === undefined ? "" : String(st.letterSpacing)} onChange={(_, d) => change(s.id, { letterSpacing: d.value === "" ? undefined : Number(d.value) }, `text:${s.id}:ls`)} />
                </Field>
                <Field label="Halo width (mm)" size="small">
                  <Input size="small" type="number" step={0.1} min={0} value={st.haloWidth === undefined ? "" : String(st.haloWidth)} onChange={(_, d) => change(s.id, { haloWidth: d.value === "" ? undefined : Number(d.value), haloColor: st.haloColor ?? "#ffffff" }, `text:${s.id}:halo`)} />
                </Field>
              </div>
              {st.haloWidth !== undefined && (
                <Field label="Halo colour" size="small">
                  <input type="color" aria-label={`${s.label} halo colour`} value={st.haloColor ?? "#ffffff"} onChange={(e) => change(s.id, { haloColor: e.target.value }, `text:${s.id}:haloc`)} />
                </Field>
              )}
              <Button size="small" appearance="subtle" disabled={!el.textStyles?.[s.id]} onClick={() => set({ textStyles: resetSlot(el.textStyles, s.id) })}>
                Reset {s.label.toLowerCase()}
              </Button>
            </div>
          </details>
        );
      })}
    </div>
  );
}
