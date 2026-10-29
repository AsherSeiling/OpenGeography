import { describe, expect, it } from "vitest";
import { snapMove, snapTargets } from "./snap";

const page = { width: 280, height: 216, margin: 12.7 };
const t = snapTargets(page, [{ axis: "x", pos: 100 }, { axis: "y", pos: 50 }], [{ x: 150, y: 80, w: 20, h: 10 }]);

describe("snapping", () => {
  it("lists page, guide and element lines", () => {
    expect(t.xs).toEqual(expect.arrayContaining([0, 12.7, 140, 267.3, 280, 100, 150, 160, 170]));
    expect(t.ys).toEqual(expect.arrayContaining([0, 12.7, 108, 203.3, 216, 50, 80, 85, 90]));
  });
  it("snaps a moved box's left edge to a guide and its bottom to another element", () => {
    const box = { x: 90, y: 30, w: 20, h: 10 };
    const r = snapMove(box, 9.4, 9.7, t, 1, null); // left edge would be at 99.4, bottom at 49.7
    expect(r.dx).toBeCloseTo(10);
    expect(r.dy).toBeCloseTo(10);
  });
  it("does nothing beyond the tolerance", () => {
    expect(snapMove({ x: 20, y: 20, w: 10, h: 10 }, 3, 3, t, 1, null)).toEqual({ dx: 3, dy: 3 });
  });
  it("snaps the centre to the page centre", () => {
    const r = snapMove({ x: 100, y: 20, w: 80, h: 10 }, 0.4, 0, t, 1, null);
    expect(100 + r.dx + 40).toBeCloseTo(140);
  });
  it("snaps only the dragged edges when resizing", () => {
    // Dragging the east handle: the right edge snaps to the guide at 100; the left edge (at 60.4) is not considered.
    const r = snapMove({ x: 60, y: 20, w: 39.4, h: 10 }, 0.3, 0, t, 1, "e");
    expect(60 + 39.4 + r.dx).toBeCloseTo(100);
    const r2 = snapMove({ x: 60, y: 20, w: 40, h: 10 }, 0.4, 0, t, 1, "s");
    expect(r2.dx).toBe(0.4);
  });
});
