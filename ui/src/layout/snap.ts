// Snapping in the Layout view (CG6): element edges and centres jump to guides, the page edges and margins, the page centre
// and the edges and centres of other elements, when closer than a few screen pixels. Pure functions.

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface SnapTargets {
  xs: number[];
  ys: number[];
}

export type Handle = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

/** Lines to snap to: guides, page edges, margins and centre, and the edges/centres of `others`. */
export function snapTargets(page: { width: number; height: number; margin: number }, guides: { axis: string; pos: number }[], others: Box[]): SnapTargets {
  const xs = [0, page.margin, page.width / 2, page.width - page.margin, page.width];
  const ys = [0, page.margin, page.height / 2, page.height - page.margin, page.height];
  for (const g of guides) (g.axis === "x" ? xs : ys).push(g.pos);
  for (const o of others) {
    xs.push(o.x, o.x + o.w / 2, o.x + o.w);
    ys.push(o.y, o.y + o.h / 2, o.y + o.h);
  }
  return { xs, ys };
}

/** The offset that brings the nearest of `edges` onto a target, if within `tol`; else 0. */
function nearest(edges: number[], targets: number[], tol: number): number {
  let best = 0;
  let bestAbs = tol + 1e-9;
  for (const e of edges) {
    for (const t of targets) {
      const d = t - e;
      if (Math.abs(d) < bestAbs) {
        bestAbs = Math.abs(d);
        best = d;
      }
    }
  }
  return bestAbs <= tol ? best : 0;
}

/** `dx`, `dy` moved so the box (moved by the raw offset) snaps: all of its edges and centre when moving, only the dragged edges when resizing. */
export function snapMove(box: Box, dx: number, dy: number, t: SnapTargets, tol: number, handle: Handle | null): { dx: number; dy: number } {
  const x0 = box.x + dx;
  const y0 = box.y + dy;
  const moving = handle === null;
  const ex: number[] = [];
  const ey: number[] = [];
  if (moving) {
    ex.push(x0, x0 + box.w / 2, x0 + box.w);
    ey.push(y0, y0 + box.h / 2, y0 + box.h);
  } else {
    if (handle.includes("w")) ex.push(x0);
    if (handle.includes("e")) ex.push(box.x + box.w + dx);
    if (handle.includes("n")) ey.push(y0);
    if (handle.includes("s")) ey.push(box.y + box.h + dy);
  }
  return { dx: dx + nearest(ex, t.xs, tol), dy: dy + nearest(ey, t.ys, tol) };
}
