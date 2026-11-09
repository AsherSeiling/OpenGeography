import { beforeEach, describe, expect, it, vi } from "vitest";

// getRenderPixel needs a real render event; here the pixel is returned as is.
vi.mock("ol/render", () => ({ getRenderPixel: (_e: unknown, p: number[]) => p }));

import { attachSwipe, chosenPresent, clampPos, useSwipe } from "./swipe";

beforeEach(() => useSwipe.setState({ byMap: {} }));

function fakeLayer() {
  const h: Record<string, ((e: unknown) => void)[]> = {};
  return {
    on: (t: string, f: (e: unknown) => void) => void (h[t] = [...(h[t] ?? []), f]),
    un: (t: string, f: (e: unknown) => void) => void (h[t] = (h[t] ?? []).filter((x) => x !== f)),
    fire: (t: string, e: unknown) => (h[t] ?? []).forEach((f) => f(e)),
    count: () => Object.values(h).reduce((n, a) => n + a.length, 0),
  };
}
function fakeCtx() {
  const calls: unknown[][] = [];
  const rec = (n: string) => (...a: unknown[]) => void calls.push([n, ...a]);
  return { calls, save: rec("save"), restore: rec("restore"), beginPath: rec("beginPath"), moveTo: rec("moveTo"), lineTo: rec("lineTo"), closePath: rec("closePath"), clip: rec("clip") };
}

describe("swipe state", () => {
  it("choosing layers turns it on at the middle; none chosen turns it off; the divider keeps its place", () => {
    const s = useSwipe.getState();
    s.setLayers("m", ["a"]);
    expect(useSwipe.getState().byMap.m).toEqual({ layers: ["a"], pos: 0.5 });
    useSwipe.getState().setPos("m", 0.25);
    useSwipe.getState().setLayers("m", ["a", "b"]);
    expect(useSwipe.getState().byMap.m.pos).toBe(0.25);
    useSwipe.getState().setLayers("m", []);
    expect(useSwipe.getState().byMap.m).toBeUndefined();
  });
  it("moving the divider on a map with no swipe does nothing; off is per map", () => {
    useSwipe.getState().setPos("x", 0.3);
    expect(useSwipe.getState().byMap).toEqual({});
    useSwipe.getState().setLayers("a", ["l"]);
    useSwipe.getState().setLayers("b", ["l"]);
    useSwipe.getState().off("a");
    expect(Object.keys(useSwipe.getState().byMap)).toEqual(["b"]);
  });
  it("keeps the divider inside the map and ignores nonsense", () => {
    expect(clampPos(-1)).toBe(0.02);
    expect(clampPos(5)).toBe(0.98);
    expect(clampPos(NaN)).toBe(0.5);
  });
  it("a layer that left the map drops out of the comparison", () => {
    expect(chosenPresent(["a", "b", "c"], ["c", "a"])).toEqual(["a", "c"]);
  });
});

describe("attachSwipe", () => {
  it("clips each chosen layer to the left of the divider and restores after, and reads the position at every draw", () => {
    const layer = fakeLayer();
    const render = vi.fn();
    let pos = 0.5;
    const map = { getSize: () => [400, 200], render };
    const off = attachSwipe(map as never, [layer] as never, () => pos);
    expect(render).toHaveBeenCalledTimes(1);
    const ctx = fakeCtx();
    layer.fire("prerender", { context: ctx });
    expect(ctx.calls.map((c) => c[0])).toEqual(["save", "beginPath", "moveTo", "lineTo", "lineTo", "lineTo", "closePath", "clip"]);
    const xs = ctx.calls.filter((c) => c[0] === "moveTo" || c[0] === "lineTo").map((c) => c[1]);
    expect(Math.max(...(xs as number[]))).toBe(200);
    layer.fire("postrender", { context: ctx });
    expect(ctx.calls.at(-1)![0]).toBe("restore");
    pos = 0.25;
    const ctx2 = fakeCtx();
    layer.fire("prerender", { context: ctx2 });
    expect(Math.max(...(ctx2.calls.filter((c) => c[0] === "lineTo").map((c) => c[1]) as number[]))).toBe(100);
    off();
    expect(layer.count()).toBe(0);
    expect(render).toHaveBeenCalledTimes(2);
  });
});
