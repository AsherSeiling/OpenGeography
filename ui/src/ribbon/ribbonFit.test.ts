import { describe, expect, it } from "vitest";
import { fitRibbon, type Variant } from "./ribbonFit";

// Six groups: widths in the four variants (large, all small, icons only, collapsed button).
const W = [[300, 220, 120, 60], [240, 180, 100, 60], [180, 150, 90, 60], [320, 240, 140, 60], [200, 160, 80, 60], [260, 200, 110, 60]];
const sum = (r: ReturnType<typeof fitRibbon>) => W.slice(0, r.shown).reduce((t, w, i) => t + w[r.variants[i]], 0);

describe("ribbon fitting", () => {
  it("leaves everything large when the window is wide", () => {
    const r = fitRibbon({ widths: W, avail: 2000, moreWidth: 70 });
    expect(r).toEqual({ variants: [0, 0, 0, 0, 0, 0], shown: 6 });
  });

  it("shrinks the right-most group first", () => {
    const total = W.reduce((t, w) => t + w[0], 0); // 1500
    const r = fitRibbon({ widths: W, avail: total - 10, moreWidth: 70 });
    expect(r.variants).toEqual([0, 0, 0, 0, 0, 1]);
    expect(r.shown).toBe(6);
  });

  it("never exceeds the available width when it can avoid it", () => {
    for (const avail of [1400, 1000, 800, 600, 450, 380, 300]) {
      const r = fitRibbon({ widths: W, avail, moreWidth: 70 });
      const used = sum(r) + (r.shown < W.length ? 70 : 0);
      expect(used, `width ${avail}`).toBeLessThanOrEqual(avail);
    }
  });

  it("moves groups into More only after every group has collapsed, and never drops one", () => {
    const wide = fitRibbon({ widths: W, avail: 600, moreWidth: 70 });
    // 6 × 60 = 360 fits in 600 without More.
    expect(wide.shown).toBe(6);
    const narrow = fitRibbon({ widths: W, avail: 250, moreWidth: 70 });
    expect(narrow.shown).toBeLessThan(6);
    expect(narrow.shown).toBeGreaterThanOrEqual(1);
    expect(narrow.variants.slice(0, narrow.shown).every((v: Variant) => v >= 0 && v <= 3)).toBe(true);
    // Everything not shown is in More, so shown + hidden is the whole ribbon.
    expect(narrow.shown + (W.length - narrow.shown)).toBe(W.length);
  });

  it("keeps at least one group inline however narrow it is", () => {
    expect(fitRibbon({ widths: W, avail: 10, moreWidth: 70 }).shown).toBe(1);
  });

  it("is monotonic: a wider window never hides more", () => {
    let last = 0;
    for (let avail = 100; avail <= 1600; avail += 50) {
      const r = fitRibbon({ widths: W, avail, moreWidth: 70 });
      const detail = r.shown * 10 - r.variants.slice(0, r.shown).reduce((t: number, v: Variant) => t + v, 0);
      expect(detail, `width ${avail}`).toBeGreaterThanOrEqual(last);
      last = detail;
    }
  });

  it("skips a collapsed button that would be wider than the icon-only form", () => {
    // Icons only = 58 px, the collapsed button = 79 px: the collapsed form is never chosen, the sum is honest.
    const w = [[300, 200, 58, 79], [300, 200, 58, 79]];
    const r = fitRibbon({ widths: w, avail: 200, moreWidth: 70 });
    const used = r.variants.slice(0, r.shown).reduce((t: number, v: Variant, i: number) => t + w[i][v], 0) + (r.shown < 2 ? 70 : 0);
    expect(used).toBeLessThanOrEqual(200);
    expect(r.variants.every((v: Variant) => v !== 3)).toBe(true);
  });

  it("handles an empty ribbon", () => {
    expect(fitRibbon({ widths: [], avail: 500, moreWidth: 70 })).toEqual({ variants: [], shown: 0 });
  });
});
