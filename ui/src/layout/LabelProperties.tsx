// Properties of one selected label in a layout (L3): its text, size, rotation, position and visibility. Edits are saved in
// this layout only, on the map frame, and undo like any layout edit.
import { Button, Checkbox, Field, Input } from "@fluentui/react-components";
import { useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { Num } from "./ElementProperties";
import { pinned, withFeatureEdit } from "./labelEdits";
import { useLabelHits } from "./labelHits";
import type { FeatureLabel, MapFrame } from "./types";

export function LayoutLabelProperties({ layout, frame, layer, fid }: { layout: string; frame: string; layer: string; fid: number }) {
  const el = useLayouts((s) => (s.id === layout ? s.doc : s.docs[layout])?.elements.find((e) => e.id === frame && e.kind === "map_frame")) as MapFrame | undefined;
  const placed = useLabelHits((s) => s.byLayout[layout]?.find((l) => l.frame === frame && l.layer === layer && l.fid === fid));
  const layerName = useApp((s) => s.layers[layer]?.name ?? layer);
  const labelStyle = useApp((s) => s.styles[layer]?.label);
  if (!el) return <div className="empty-hint pad">This label's map frame no longer exists.</div>;
  const edit: FeatureLabel = el.labelOverrides?.[layer]?.features?.[String(fid)] ?? {};
  const set = (patch: FeatureLabel | null) => useLayouts.getState().update(frame, { labelOverrides: withFeatureEdit(el.labelOverrides, layer, fid, patch) }, patch ? `label:${layer}:${fid}` : undefined);
  const size = edit.sizePt ?? labelStyle?.size ?? 9;
  return (
    <div className="props">
      <div className="props-title">Label <span className="muted">({layerName}, feature {fid})</span></div>
      <div className="props-body stack">
        <Field label="Text (empty = the field's value)" size="small">
          <Input size="small" value={edit.text ?? ""} placeholder={placed && !edit.text ? placed.text : ""} onChange={(_, d) => set({ text: d.value })} />
        </Field>
        <div className="props-grid">
          <Num label="Size (pt)" value={size} min={1} step={0.5} decimals={1} onChange={(sizePt) => set({ sizePt })} />
          <Num label="Rotation (°)" value={edit.rotation ?? placed?.rotation ?? 0} decimals={1} onChange={(rotation) => set({ rotation })} />
          <Num label="Move right (mm)" value={edit.dxMm ?? 0} step={0.5} decimals={1} onChange={(dxMm) => set({ dxMm })} />
          <Num label="Move down (mm)" value={edit.dyMm ?? 0} step={0.5} decimals={1} onChange={(dyMm) => set({ dyMm })} />
        </div>
        <Checkbox label="Hide this label" checked={!!edit.hidden} onChange={(_, d) => set({ hidden: d.checked ? true : undefined })} />
        <div className="muted">{pinned(edit) ? "Edited: this label is placed first and other labels avoid it." : edit.hidden ? "Hidden: it will not come back automatically." : "Automatic: placed where it fits."}</div>
        <Button size="small" disabled={!Object.keys(edit).length} onClick={() => set(null)}>Reset to automatic</Button>
      </div>
    </div>
  );
}
