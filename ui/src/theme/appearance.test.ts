import { beforeEach, describe, expect, it } from "vitest";
import {
  ACCENTS, FAMILIES, HILITES, PALETTE_NAMES, PALETTES, STORAGE_KEY, applyAppearance, defaultAppearance, deriveTokens, initAppearance, parseAppearance,
  resetAppearance, saveAppearance, setAppearance, useAppearance, type Appearance, type Mode, type RootLike,
} from "./appearance";
import { ratio } from "./colorMath";
import { useApp } from "../store/store";

// Seeded, so a failure reproduces.
let seed = 20261103;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);
const randomColour = () => "#" + [0, 1, 2].map(() => Math.floor(rnd() * 256).toString(16).padStart(2, "0")).join("").toUpperCase();

const MODES: Mode[] = ["light", "dark"];
const SURFACES = ["surface-0", "surface-1", "surface-2"];

function check(a: Appearance, mode: Mode, label: string) {
  const t = deriveTokens(a, mode);
  const low = (fg: string, bg: string, what: string) => expect(ratio(t[fg] ?? fg, t[bg] ?? bg), `${label} ${mode}: ${what}`).toBeGreaterThanOrEqual(4.5);
  for (const s of SURFACES) {
    low("text", s, `text on ${s}`);
    low("text-2", s, `text-2 on ${s}`);
    low("link", s, `link on ${s}`);
  }
  low("text", "canvas", "text on canvas");
  low("sel-text", "sel-bg", "text on highlight");
  for (const k of FAMILIES) low(`fam-${k}-text`, `fam-${k}`, `text on ${k}`);
}

const colours = [...ACCENTS.map((c) => c[1]), ...HILITES.map((c) => c[1]), ...Array.from({ length: 48 }, randomColour), "#000000", "#FFFFFF", "#808080"];

describe("readability is enforced", () => {
  for (const palette of PALETTE_NAMES)
    for (const mode of MODES)
      it(`${palette} ${mode}: defaults and ${colours.length} chosen colours stay AA`, () => {
        check({ ...defaultAppearance(), palette }, mode, "defaults");
        for (const c of colours) {
          const fam = Object.fromEntries(FAMILIES.map((k) => [k, c])) as Appearance["fam"];
          check({ palette, accent: c, hilite: c, fam }, mode, c);
        }
      });

  it("palette text is AA on every surface", () => {
    for (const p of PALETTE_NAMES) for (const m of MODES) for (const s of SURFACES) for (const k of ["text", "text-2"]) expect(ratio((PALETTES[p][m] as Record<string, string>)[k], (PALETTES[p][m] as Record<string, string>)[s])).toBeGreaterThanOrEqual(4.5);
  });

  it("a blue-ish accent keeps a blue link; the default light link matches tokens.css", () => {
    expect(deriveTokens(defaultAppearance(), "light").link).toBe("#0869A9");
    const [, g, b] = deriveTokens({ ...defaultAppearance(), accent: "#3B9BFF" }, "light").link.match(/\w\w/g)!.map((x) => parseInt(x, 16));
    expect(b).toBeGreaterThan(g);
  });
});

function fakeRoot(): RootLike & { props: Record<string, string> } {
  const props: Record<string, string> = {};
  return { props, dataset: {}, style: { setProperty: (k, v) => void (props[k] = v) } };
}

describe("applying", () => {
  it("writes the theme and every derived token as inline properties", () => {
    const r = fakeRoot();
    applyAppearance({ ...defaultAppearance(), accent: "#FF8A6B" }, "dark", r);
    expect(r.dataset.theme).toBe("dark");
    expect(r.props["--accent"]).toBe("#FF8A6B");
    expect(r.props["--surface-0"]).toBe(PALETTES.Glacier.dark["surface-0"]);
    expect(r.props["--fam-Input-text"]).toBeDefined();
  });
});

class MemStorage {
  data = new Map<string, string>();
  getItem(k: string) { return this.data.get(k) ?? null; }
  setItem(k: string, v: string) { this.data.set(k, v); }
}

describe("storage", () => {
  const custom: Appearance = { palette: "Paper", accent: "#9B8CFF", hilite: "#FFE680", fam: { ...defaultAppearance().fam, Raster: "#112233" } };

  it("saves and reloads", () => {
    const s = new MemStorage();
    saveAppearance(custom, true, s as unknown as Storage);
    const back = parseAppearance(s.getItem(STORAGE_KEY));
    expect(back.appearance).toEqual(custom);
    expect(back.dark).toBe(true);
  });

  it("falls back to defaults for missing or corrupt data", () => {
    const d = defaultAppearance();
    for (const raw of [null, "", "{not json", "42", "null", '"x"', "[]"]) expect(parseAppearance(raw).appearance, String(raw)).toEqual(d);
    expect(parseAppearance("{}").dark).toBeNull();
  });

  it("repairs bad fields one by one", () => {
    const p = parseAppearance(JSON.stringify({ palette: "Neon", accent: "red", hilite: "#a5ecff", fam: { Vector: 5, Raster: "#abcdef", Nope: "#000000" }, mode: "dark" }));
    expect(p.appearance.palette).toBe("Glacier");
    expect(p.appearance.accent).toBe(defaultAppearance().accent);
    expect(p.appearance.hilite).toBe("#A5ECFF");
    expect(p.appearance.fam.Vector).toBe(defaultAppearance().fam.Vector);
    expect(p.appearance.fam.Raster).toBe("#ABCDEF");
    expect(p.dark).toBe(true);
  });

  it("a storage that throws does not throw", () => {
    const bad = { setItem: () => { throw new Error("full"); } };
    expect(() => saveAppearance(custom, false, bad as unknown as Storage)).not.toThrow();
  });
});

describe("live state", () => {
  beforeEach(() => {
    const store = new MemStorage();
    (globalThis as any).localStorage = store;
    (globalThis as any).document = { documentElement: fakeRoot() };
    useApp.setState({ dark: false });
    useAppearance.setState({ ...defaultAppearance(), open: false });
  });

  it("changes apply, save, and survive a reload; Reset restores defaults but keeps the mode", () => {
    setAppearance({ accent: "#19C9A6", palette: "Wireframe" });
    const root = (document as any).documentElement;
    expect(root.props["--accent"]).toBe("#19C9A6");
    const saved = parseAppearance(localStorage.getItem(STORAGE_KEY));
    expect(saved.appearance.accent).toBe("#19C9A6");
    expect(saved.appearance.palette).toBe("Wireframe");

    useAppearance.setState(defaultAppearance()); // what a fresh page starts with
    initAppearance();
    expect(useAppearance.getState().accent).toBe("#19C9A6");
    expect(root.props["--accent"]).toBe("#19C9A6");

    useApp.setState({ dark: true }); // light/dark follows useApp.dark
    expect(root.dataset.theme).toBe("dark");
    expect(root.props["--surface-0"]).toBe(PALETTES.Wireframe.dark["surface-0"]);

    resetAppearance();
    expect(useAppearance.getState().accent).toBe(defaultAppearance().accent);
    expect(useAppearance.getState().palette).toBe("Glacier");
    expect(useApp.getState().dark).toBe(true);
    expect(root.props["--accent"]).toBe(defaultAppearance().accent);
  });

  it("ignores an invalid colour", () => {
    setAppearance({ accent: "not-a-colour" });
    expect(useAppearance.getState().accent).toBe(defaultAppearance().accent);
  });
});
