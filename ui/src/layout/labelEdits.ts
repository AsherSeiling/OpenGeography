// Edits of labels inside one layout (L3): pure functions on a map frame's `labelOverrides`, so they are easy to test
// and every change is one `update` of the frame (one undo step).
import type { DroppedLabelEdit, FeatureLabel, LabelOverrides, LabelPatch } from "./types";

type Overrides = Record<string, LabelOverrides>;

const clean = <T extends object>(o: T): T | undefined => {
  const out = Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined && v !== null && v !== "")) as T;
  return Object.keys(out).length ? out : undefined;
};

/** `base` with one label's edit changed by `patch` (a key set to undefined is removed); `null` removes the label's edits ("Reset to automatic"). */
export function withFeatureEdit(base: Overrides | undefined, layer: string, fid: number, patch: FeatureLabel | null): Overrides | undefined {
  const cur = base?.[layer] ?? {};
  const features = { ...(cur.features ?? {}) };
  const next = patch === null ? undefined : clean({ ...(features[String(fid)] ?? {}), ...patch });
  if (next) features[String(fid)] = next;
  else delete features[String(fid)];
  return put(base, layer, { ...cur, features: Object.keys(features).length ? features : undefined });
}

/** `base` with this layout's label style for a layer changed by `patch`; `null` clears it (back to the map's setting). */
export function withStyleOverride(base: Overrides | undefined, layer: string, patch: LabelPatch | null): Overrides | undefined {
  const cur = base?.[layer] ?? {};
  return put(base, layer, { ...cur, style: patch === null ? undefined : clean({ ...(cur.style ?? {}), ...patch }) });
}

/** `base` without the edits the engine says point at features that are gone. */
export function withoutDropped(base: Overrides | undefined, frame: string, dropped: DroppedLabelEdit[]): Overrides | undefined {
  let out = base;
  for (const d of dropped) if (d.frame === frame) out = withFeatureEdit(out, d.layer, d.fid, null);
  return out;
}

function put(base: Overrides | undefined, layer: string, v: LabelOverrides): Overrides | undefined {
  const next = { ...(base ?? {}) };
  const cleaned = clean({ style: v.style, features: v.features });
  if (cleaned) next[layer] = cleaned;
  else delete next[layer];
  return Object.keys(next).length ? next : undefined;
}

/** True when an edit pins the label (moves, turns, resizes or re-words it). */
export const pinned = (e: FeatureLabel | undefined) => !!e && (e.dxMm !== undefined || e.dyMm !== undefined || e.rotation !== undefined || e.text !== undefined || e.sizePt !== undefined);

/** A drag of (dxMm, dyMm) added to what the label already has; drags under 0.2 mm count as a click. */
export function dragged(e: FeatureLabel | undefined, dxMm: number, dyMm: number): FeatureLabel | null {
  if (Math.hypot(dxMm, dyMm) < 0.2) return null;
  const r = (v: number) => Math.round(v * 100) / 100;
  return { dxMm: r((e?.dxMm ?? 0) + dxMm), dyMm: r((e?.dyMm ?? 0) + dyMm) };
}
