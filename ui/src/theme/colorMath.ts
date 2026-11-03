/** Colour maths shared by the token derivation and its tests. Same formulas as `applyTheme()` in docs/design/ui-redesign-mockup.html. */

export const INK = "#06222B";

export const rgb = (h: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)) as [number, number, number];

export const hex = (a: number[]): string =>
  "#" +
  a
    .map((v) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase();

export const mix = (a: string, b: string, t: number): string => {
  const x = rgb(a);
  const y = rgb(b);
  return hex(x.map((v, i) => v + (y[i] - v) * t));
};

export const lum = (h: string): number => {
  const [r, g, b] = rgb(h).map((v) => {
    v /= 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

/** WCAG contrast ratio of two #RRGGBB colours. */
export const ratio = (a: string, b: string): number => {
  const x = lum(a);
  const y = lum(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};

/** Text on any chosen colour picks itself: the first of ink, white or black that reaches 4.5:1. */
export const readable = (c: string): string => [INK, "#FFFFFF", "#000000"].find((t) => ratio(c, t) >= 4.5) ?? "#000000";

/** Moves `c` toward `toward` in 5 % steps until it reaches 4.5:1 on `on`. */
export const untilAA = (c: string, toward: string, on: string): string => {
  let v = c;
  for (let t = 0.05; ratio(v, on) < 4.5 && t <= 1; t += 0.05) v = mix(c, toward, t);
  return v;
};

/** Blue text colour derived from the accent, AA on `surface0`; falls back to `text` when no shift reaches AA. */
export function linkColour(accent: string, dark: boolean, surface0: string, text: string): string {
  const [ar, ag, ab] = rgb(accent);
  const blueish = ab > ar && ab >= ag * 0.9;
  const link = dark ? untilAA(accent, "#FFFFFF", surface0) : untilAA(accent, blueish ? "#00488F" : mix(accent, "#000000", 0.8), surface0);
  return ratio(link, surface0) >= 4.5 ? link : text;
}
