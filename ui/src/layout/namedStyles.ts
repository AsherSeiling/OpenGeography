// Named text styles (add-on 9): "Title", "Legend body" and so on. A named style is a look (size, weight, slant, colour, font) that is
// applied to a Text element or to a text slot of another element, and that can be changed once for everything using it.
//
// The page file keeps only the look each element ended up with (the engine reads `size`, `font`, `color` and `textStyles`), so an element
// "uses" a style when its look equals the style's. Changing a style rewrites every element that matches the old look. Definitions are kept
// per layout in this browser profile (localStorage), the page itself is not changed by them. Pure helpers here, so they are unit-tested.
import { create } from "zustand";
import { withSlotStyle } from "./textStyles";
import { PAPER, type Layout, type LayoutElement, type TextElement, type TextStyle } from "./types";

export interface NamedStyle {
  name: string;
  /** Points, 1 to 400. */
  size: number;
  bold?: boolean;
  italic?: boolean;
  color?: string;
  family?: string;
}

export const DEFAULT_STYLES: NamedStyle[] = [
  { name: "Title", size: 21, bold: true, color: PAPER.black },
  { name: "Subtitle", size: 12, color: PAPER.dark },
  { name: "Body", size: 9, color: PAPER.black },
  { name: "Legend body", size: 8, color: PAPER.black },
  { name: "Small print", size: 6.5, color: PAPER.mid },
];

const same = (a: unknown, b: unknown) => (a ?? undefined) === (b ?? undefined);
const lc = (c: string | undefined) => c?.toLowerCase();

/** The look of a Text element, as a style. */
export function textLook(el: TextElement): Omit<NamedStyle, "name"> {
  return { size: el.size, bold: el.font === "sans-bold", italic: el.font === "sans-italic", color: lc(el.color), family: el.family };
}

export function textMatches(el: TextElement, s: NamedStyle): boolean {
  const l = textLook(el);
  return same(l.size, s.size) && !!l.bold === !!s.bold && !!l.italic === !!s.italic && same(l.color, lc(s.color) ?? PAPER.black) && same(l.family, s.family);
}

/** The element fields that give a Text element this style. */
export function textPatch(s: NamedStyle): Record<string, unknown> {
  return { size: s.size, font: s.bold ? "sans-bold" : s.italic ? "sans-italic" : "sans", color: s.color ?? PAPER.black, family: s.family };
}

export function slotMatches(st: TextStyle | undefined, s: NamedStyle): boolean {
  return !!st && same(st.size, s.size) && !!st.bold === !!s.bold && !!st.italic === !!s.italic && same(lc(st.color), lc(s.color)) && same(st.family, s.family);
}

/** The element field patch (`textStyles`) that gives one slot this style, keeping the slot's other settings (underline, halo, spacing). */
export function slotPatch(el: LayoutElement, slot: string, s: NamedStyle): Record<string, unknown> {
  return { textStyles: withSlotStyle(el.textStyles, slot, { size: s.size, bold: s.bold ?? false, italic: s.italic ?? false, color: s.color, family: s.family }) };
}

export interface Use {
  id: string;
  /** Unset for a Text element's own look. */
  slot?: string;
}

/** Everything in the layout that has exactly this look now. */
export function usesOf(layout: Layout, s: NamedStyle): Use[] {
  const out: Use[] = [];
  for (const e of layout.elements) {
    if (e.kind === "text" && textMatches(e, s)) out.push({ id: e.id });
    for (const [slot, st] of Object.entries(e.textStyles ?? {})) if (e.kind !== "text" || slot !== "text.text") if (slotMatches(st, s)) out.push({ id: e.id, slot });
  }
  return out;
}

/** Edits (element id → fields) that move everything using `from` to `to`. */
export function restyle(layout: Layout, from: NamedStyle, to: NamedStyle): Record<string, Record<string, unknown>> {
  const out: Record<string, Record<string, unknown>> = {};
  for (const u of usesOf(layout, from)) {
    const e = layout.elements.find((x) => x.id === u.id)!;
    const base = out[u.id] ? { ...e, ...out[u.id] } : e;
    out[u.id] = { ...out[u.id], ...(u.slot ? slotPatch(base as LayoutElement, u.slot, to) : textPatch(to)) };
  }
  return out;
}

/** A size box's text as a size, or null when it is not 1 to 400. */
export function parseStyleSize(t: string): number | null {
  const v = Number(t.trim());
  return t.trim() !== "" && Number.isFinite(v) && v >= 1 && v <= 400 ? v : null;
}

// ---- the definitions of one layout, kept in this browser profile

const KEY = (layoutId: string) => `gis.textStyles.${layoutId}`;

export function loadStyles(layoutId: string): NamedStyle[] {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY(layoutId)) ?? "null");
    if (Array.isArray(raw)) {
      const ok = raw.filter((s): s is NamedStyle => !!s && typeof s.name === "string" && typeof s.size === "number" && s.size >= 1 && s.size <= 400);
      if (ok.length) return ok;
    }
  } catch {
    /* storage can be unavailable or hold something else */
  }
  return DEFAULT_STYLES;
}

interface StylesState {
  byLayout: Record<string, NamedStyle[]>;
  get: (layoutId: string) => NamedStyle[];
  set: (layoutId: string, styles: NamedStyle[]) => void;
}

export const useNamedStyles = create<StylesState>((set, get) => ({
  byLayout: {},
  get: (id) => get().byLayout[id] ?? loadStyles(id),
  set: (id, styles) => {
    try {
      localStorage.setItem(KEY(id), JSON.stringify(styles));
    } catch {
      /* the styles still apply for this session */
    }
    set((s) => ({ byLayout: { ...s.byLayout, [id]: styles } }));
  },
}));
