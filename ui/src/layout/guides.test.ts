import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  api: { saveLayout: vi.fn(async () => {}), deleteLayout: vi.fn(async () => {}), listLayouts: vi.fn(async () => []) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));

import { addGuide, clearGuides, nextGuidePos, useLayouts } from "../store/layout";
import { newLayoutDoc } from "./types";

const L = useLayouts;
beforeEach(() => {
  L.getState().loadAll([newLayoutDoc("a", "A")]);
  L.getState().open("a");
});

describe("UR-6 guides added from the ribbon use the layout's own guide list", () => {
  it("adds a vertical and a horizontal guide at the middle of the page", () => {
    const { width, height } = L.getState().doc!.page;
    addGuide("x");
    addGuide("y");
    expect(L.getState().doc!.guides).toEqual([{ axis: "x", pos: Math.round(width / 2) }, { axis: "y", pos: Math.round(height / 2) }]);
  });

  it("a second click makes a second guide, 10 mm along, instead of stacking on the first", () => {
    const mid = Math.round(L.getState().doc!.page.width / 2);
    addGuide("x");
    addGuide("x");
    expect(L.getState().doc!.guides!.map((g) => g.pos)).toEqual([mid, mid + 10]);
    expect(nextGuidePos([{ axis: "x", pos: 5 }], "y", L.getState().doc!.page)).toBe(Math.round(L.getState().doc!.page.height / 2));
  });

  it("is one undo step, and Clear Guides is too", () => {
    addGuide("x");
    addGuide("y");
    L.getState().undo();
    expect(L.getState().doc!.guides).toHaveLength(1);
    clearGuides();
    expect(L.getState().doc!.guides).toEqual([]);
    L.getState().undo();
    expect(L.getState().doc!.guides).toHaveLength(1);
  });
});
