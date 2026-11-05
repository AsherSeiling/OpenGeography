/**
 * The user's colour choices (docs/UI_REDESIGN_PLAN.md §2a): state, token derivation and storage.
 * Port of applyTheme() in docs/design/ui-redesign-mockup.html. Light or dark stays in `useApp.dark`; everything else is here.
 * Choices are written as inline properties on :root, so tokens.css stays the default and every consumer follows live.
 * The tables (palettes, swatches, family defaults) live in palettes.json: they are the data of this feature, not chrome code.
 */
import { create } from "zustand";
import { useApp } from "../store/store";
import { linkColour, readable } from "./colorMath";
import tables from "./palettes.json";

export type PaletteName = keyof typeof tables.palettes;
export type Mode = "light" | "dark";
export type FamilyName = keyof typeof tables.families;

export interface Appearance {
  palette: PaletteName;
  accent: string;
  hilite: string;
  fam: Record<FamilyName, string>;
}

export const PALETTES = tables.palettes;
export const PALETTE_NAMES = Object.keys(PALETTES) as PaletteName[];
export const ACCENTS = tables.accents as [string, string][];
export const HILITES = tables.highlights as [string, string][];
export const FAMILIES = Object.keys(tables.families) as FamilyName[];
export const FAMILY_LABEL: Record<FamilyName, string> = {
  Input: "Layer inputs",
  Vector: "Vector",
  Raster: "Raster",
  Hydrology: "Hydrology",
  Tables: "Tables",
  Coordinate: "Coordinate systems",
  Analysis: "Analysis",
  Styling: "Styling",
};

export const defaultAppearance = (): Appearance => ({
  palette: "Glacier",
  accent: ACCENTS[0][1],
  hilite: HILITES[0][1],
  fam: { ...tables.families },
});

const HEX6 = /^#[0-9a-f]{6}$/i;
const colour = (v: unknown, fallback: string): string => (typeof v === "string" && HEX6.test(v) ? v.toUpperCase() : fallback);

/** Every token the choices change, for one mode. Text on any chosen colour is computed; accent text is shifted until AA. */
export function deriveTokens(a: Appearance, mode: Mode): Record<string, string> {
  const dark = mode === "dark";
  const P: Record<string, string> = PALETTES[a.palette][mode];
  const link = linkColour(a.accent, dark, P["surface-0"], P.text);
  const out: Record<string, string> = {
    ...P,
    accent: a.accent,
    "sel-bg": a.hilite,
    "sel-text": readable(a.hilite),
    link,
    focus: dark ? link : P.text,
    ...tables.statusColours[mode],
  };
  for (const k of FAMILIES) {
    out[`fam-${k}`] = a.fam[k];
    out[`fam-${k}-text`] = readable(a.fam[k]);
  }
  return out;
}

/** The part of `document.documentElement` that `applyAppearance` needs; tests pass a stand-in. */
export interface RootLike {
  style: { setProperty(name: string, value: string): void };
  dataset: Record<string, string | undefined>;
}

export function applyAppearance(a: Appearance, mode: Mode, root: RootLike = document.documentElement): void {
  root.dataset.theme = mode;
  const t = deriveTokens(a, mode);
  for (const k of Object.keys(t)) root.style.setProperty(`--${k}`, t[k]);
}

// ---------- Storage: per user, never in the project ----------
export const STORAGE_KEY = "gis.appearance.v1";

/** Saved choices, field by field; anything missing or malformed falls back to the default. */
export function parseAppearance(raw: string | null): { appearance: Appearance; dark: boolean | null } {
  const d = defaultAppearance();
  let v: any = null;
  try {
    v = raw ? JSON.parse(raw) : null;
  } catch {
    v = null;
  }
  if (!v || typeof v !== "object") return { appearance: d, dark: null };
  const fam = { ...d.fam };
  for (const k of FAMILIES) fam[k] = colour(v.fam?.[k], d.fam[k]);
  // Saved before the default went green (no `v`) and still on the old default pair: that is not a choice, so it follows the new default.
  const oldDefault = v.v === undefined && colour(v.accent, "") === tables.formerDefault.accent && colour(v.hilite, "") === tables.formerDefault.hilite;
  return {
    appearance: {
      palette: PALETTE_NAMES.includes(v.palette) ? v.palette : d.palette,
      accent: oldDefault ? d.accent : colour(v.accent, d.accent),
      hilite: oldDefault ? d.hilite : colour(v.hilite, d.hilite),
      fam,
    },
    dark: v.mode === "dark" ? true : v.mode === "light" ? false : null,
  };
}

export function saveAppearance(a: Appearance, dark: boolean, storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  try {
    storage?.setItem(STORAGE_KEY, JSON.stringify({ ...a, v: 2, mode: dark ? "dark" : "light" }));
  } catch {
    /* private mode or full: the choice still applies for this session */
  }
}

function safeStorage(): Storage | undefined {
  try {
    return localStorage;
  } catch {
    return undefined;
  }
}

// ---------- Live state ----------
interface AppearanceState extends Appearance {
  open: boolean;
}

export const useAppearance = create<AppearanceState>(() => ({ ...defaultAppearance(), open: false }));

const current = (): Appearance => {
  const { palette, accent, hilite, fam } = useAppearance.getState();
  return { palette, accent, hilite, fam };
};

function rederive() {
  const dark = useApp.getState().dark;
  applyAppearance(current(), dark ? "dark" : "light");
  saveAppearance(current(), dark);
}

/** Changes one or more choices; colours are validated, so a bad value is ignored. */
export function setAppearance(p: Partial<Omit<Appearance, "fam">> & { fam?: Partial<Appearance["fam"]> }): void {
  const s = useAppearance.getState();
  useAppearance.setState({
    palette: p.palette && PALETTE_NAMES.includes(p.palette) ? p.palette : s.palette,
    accent: p.accent ? colour(p.accent, s.accent) : s.accent,
    hilite: p.hilite ? colour(p.hilite, s.hilite) : s.hilite,
    fam: Object.fromEntries(FAMILIES.map((k) => [k, colour(p.fam?.[k], s.fam[k])])) as Appearance["fam"],
  });
  rederive();
}

/** Back to the default colours; light or dark stays as it is. */
export function resetAppearance(): void {
  useAppearance.setState(defaultAppearance());
  rederive();
}

export const openAppearance = () => useAppearance.setState({ open: true });
export const closeAppearance = () => useAppearance.setState({ open: false });
export const toggleAppearance = () => useAppearance.setState((s) => ({ open: !s.open }));
/** Hook: is the Colours panel open? For the View › Colours button's pressed state. */
export const useAppearanceOpen = (): boolean => useAppearance((s) => s.open);
export const isAppearanceOpen = useAppearanceOpen;

let started = false;
/** Call once before first paint: loads the saved choices, applies them, and follows light/dark from `useApp.dark`. */
export function initAppearance(): void {
  if (started) return;
  started = true;
  const saved = parseAppearance(safeStorage()?.getItem(STORAGE_KEY) ?? null);
  useAppearance.setState(saved.appearance);
  if (saved.dark !== null && saved.dark !== useApp.getState().dark) useApp.setState({ dark: saved.dark });
  rederive();
  let last = useApp.getState().dark;
  useApp.subscribe((s) => {
    if (s.dark === last) return;
    last = s.dark;
    rederive();
  });
}
