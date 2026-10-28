import { describe, expect, it } from "vitest";
import { rulerTicks } from "./Rulers";

const majors = (t: ReturnType<typeof rulerTicks>) => t.filter((x) => x.major).map((x) => x.mm);

describe("layout rulers (C3.0)", () => {
  it("at 2 px per mm labels every 50 mm with minor ticks every 10", () => {
    const t = rulerTicks(0, 280, 2);
    expect(majors(t)).toEqual([0, 50, 100, 150, 200, 250]);
    expect(t.filter((x) => !x.major).slice(0, 4).map((x) => x.mm)).toEqual([10, 20, 30, 40]);
  });
  it("zooming in refines the labels, zooming out thins them, always at least 60 px apart", () => {
    for (const z of [0.3, 0.5, 1, 2, 4, 8, 12]) {
      const m = majors(rulerTicks(0, 300, z));
      if (m.length > 1) expect((m[1] - m[0]) * z).toBeGreaterThanOrEqual(60 - 1e-9);
    }
    expect(majors(rulerTicks(0, 30, 12))).toEqual([0, 5, 10, 15, 20, 25, 30]); // 60 px per 5 mm
    expect(majors(rulerTicks(0, 3000, 0.3)).slice(0, 3)).toEqual([0, 200, 400]);
  });
  it("never labels -0", () => {
    for (const z of [0.3, 1, 2, 12]) expect(rulerTicks(-0.2, 100, z).every((t) => !Object.is(t.mm, -0))).toBe(true);
  });
  it("covers a scrolled range, including negative positions left of the page", () => {
    const t = rulerTicks(-30, 30, 2);
    expect(t[0].mm).toBeLessThanOrEqual(-29);
    expect(t.some((x) => x.mm === 0 && x.major)).toBe(true);
    expect(majors(t)).toEqual([0]); // the 50 mm labels: only 0 lies in -30…30
  });
  it("bad input gives no ticks", () => {
    expect(rulerTicks(0, 10, 0)).toEqual([]);
    expect(rulerTicks(10, 0, 2)).toEqual([]);
    expect(rulerTicks(NaN, 10, 2)).toEqual([]);
  });
  it("stays bounded", () => {
    expect(rulerTicks(0, 1e9, 12).length).toBeLessThanOrEqual(2000);
  });
});
