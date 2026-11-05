import { describe, expect, it } from "vitest";
import { pageMmAt, scrollToKeep, wheelZoomFactor } from "./view";

// A scroll area showing a page: the page's on-screen left edge is `origin - scroll`. Zooming changes the page's size, and the scroll
// offset is then moved by scrollToKeep. The page position under the pointer must not change.
function zoomOnce(o: { origin: number; scroll: number; z: number; to: number; cursor: number }) {
  const left = o.origin - o.scroll;
  const mm = pageMmAt(left, 0, o.z, o.cursor, 0);
  const leftAfterResize = o.origin - o.scroll; // the page starts at the same place, only bigger
  const { dx } = scrollToKeep(mm, leftAfterResize, 0, o.to, o.cursor, 0);
  const scroll = o.scroll + dx;
  const finalLeft = o.origin - scroll;
  return { before: mm.x, after: pageMmAt(finalLeft, 0, o.to, o.cursor, 0).x };
}

describe("UR-7 anchored zoom keeps the page position under the pointer", () => {
  it("zooming in and out around several pointer positions and scroll offsets", () => {
    for (const cursor of [100, 333.5, 760]) {
      for (const scroll of [0, 250, 1200]) {
        for (const [z, to] of [[3, 3.3], [3.3, 3], [2, 11.9], [11.9, 0.3]]) {
          const r = zoomOnce({ origin: 60, scroll, z, to, cursor });
          expect(r.after).toBeCloseTo(r.before, 9);
        }
      }
    }
  });

  it("a point at the pointer is the same millimetre before and after, for a known case", () => {
    // Page left edge at 40 px, 3 px/mm, pointer at 340 px: 100 mm. Zoom to 6 px/mm: the page grows from the same left edge, so
    // 100 mm would be at 640 px; scrolling 300 px right puts it back under the pointer.
    expect(pageMmAt(40, 0, 3, 340, 0).x).toBe(100);
    expect(scrollToKeep({ x: 100, y: 0 }, 40, 0, 6, 340, 0).dx).toBe(300);
  });

  it("a wheel notch is about 10 %, a small pinch delta a small step, and the direction follows the sign", () => {
    expect(wheelZoomFactor(-100)).toBeCloseTo(1.105, 2);
    expect(wheelZoomFactor(100)).toBeCloseTo(1 / 1.105, 2);
    expect(wheelZoomFactor(-2)).toBeCloseTo(1.008, 3);
    expect(wheelZoomFactor(0)).toBe(1);
    expect(wheelZoomFactor(-100000)).toBeCloseTo(wheelZoomFactor(-100), 9); // one event never jumps far
  });
});
