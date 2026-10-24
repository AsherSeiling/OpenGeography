import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  api: { saveLayout: vi.fn(async () => {}), deleteLayout: vi.fn(async () => {}), listLayouts: vi.fn(async () => []) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));

import { api } from "../ipc/api";
import { resized } from "../layout/LayoutView";
import { newLayoutDoc, roundScaleUp, type MapFrame, type TextElement } from "../layout/types";
import { addMapFrame, addText, beginStep, flushLayoutSaves, useLayouts } from "./layout";
import { useApp } from "./store";

const L = useLayouts;
const doc = () => L.getState().doc!;
const text = (id: string) => doc().elements.find((e) => e.id === id) as TextElement;

beforeEach(() => {
  L.getState().loadAll([newLayoutDoc("a", "A"), newLayoutDoc("b", "B", "A4", "portrait")]);
  L.getState().open("a");
  useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" }, dirty: false });
  vi.clearAllMocks();
});

describe("layout store", () => {
  it("inserts a map frame in the map CRS, and refuses without one", () => {
    const id = addMapFrame()!;
    const f = doc().elements.find((e) => e.id === id) as MapFrame;
    expect(f.crs).toBe("EPSG:26915");
    expect(f.w).toBeCloseTo(279.4 - 2 * 12.7);
    expect(L.getState().selected).toEqual([id]);
    useApp.setState({ mapCrs: null });
    expect(addMapFrame()).toBeNull();
    expect(useApp.getState().status).toMatch(/Set a coordinate system for the map frame/);
    expect(doc().elements).toHaveLength(1);
  });

  it("move, resize, undo and redo", () => {
    const id = addText("Title")!;
    const start = { x: text(id).x, y: text(id).y, w: text(id).w };
    L.getState().update(id, { x: start.x + 10, y: start.y + 5 });
    L.getState().update(id, { w: 100 });
    expect(text(id)).toMatchObject({ x: start.x + 10, y: start.y + 5, w: 100 });
    L.getState().undo();
    expect(text(id).w).toBe(start.w);
    L.getState().undo();
    expect(text(id).x).toBe(start.x);
    L.getState().redo();
    expect(text(id).x).toBe(start.x + 10);
    L.getState().undo();
    L.getState().undo(); // undoes the insert
    expect(doc().elements).toHaveLength(0);
  });

  it("a drag is one undo step", () => {
    const id = addText()!;
    const x0 = text(id).x;
    const end = beginStep();
    for (let i = 1; i <= 5; i++) L.getState().update(id, { x: x0 + i });
    end();
    expect(text(id).x).toBe(x0 + 5);
    L.getState().undo();
    expect(text(id).x).toBe(x0);
  });

  it("typing in one setting is one undo step", () => {
    const id = addText("a")!;
    for (const t of ["ab", "abc", "abcd"]) L.getState().update(id, { text: t }, "text");
    L.getState().undo();
    expect(text(id).text).toBe("a");
  });

  it("undo history is per layout", () => {
    const id = addText()!;
    L.getState().open("b");
    expect(doc().elements).toHaveLength(0);
    expect(L.temporal.getState().pastStates).toHaveLength(0); // B has no history
    addText();
    L.getState().open("a");
    expect(doc().elements.map((e) => e.id)).toEqual([id]);
    L.getState().undo(); // A's own insert, not B's
    expect(doc().elements).toHaveLength(0);
    L.getState().open("b");
    expect(doc().elements).toHaveLength(1);
    L.getState().undo();
    expect(doc().elements).toHaveLength(0);
  });

  it("removes elements and keeps locked ones out of nothing else", () => {
    const a = addText()!;
    const b = addText()!;
    L.getState().removeElements([a]);
    expect(doc().elements.map((e) => e.id)).toEqual([b]);
    L.getState().undo();
    expect(doc().elements).toHaveLength(2);
  });

  it("saves edits to the engine (debounced) and marks the project dirty", async () => {
    addText();
    expect(useApp.getState().dirty).toBe(true);
    expect(api.saveLayout).not.toHaveBeenCalled();
    await flushLayoutSaves();
    expect(api.saveLayout).toHaveBeenCalledTimes(1);
    expect((api.saveLayout as ReturnType<typeof vi.fn>).mock.calls[0][0]).toMatchObject({ id: "a" });
  });

  it("duplicates, renames and deletes", () => {
    const copy = L.getState().duplicate("a")!;
    expect(copy.name).toBe("A 2");
    L.getState().rename(copy.id, "Copy");
    expect(L.getState().list().map((l) => l.name)).toEqual(["A", "B", "Copy"]);
    L.getState().remove(copy.id);
    expect(api.deleteLayout).toHaveBeenCalledWith(copy.id);
    expect(L.getState().list()).toHaveLength(2);
  });
});

describe("helpers", () => {
  it("resizes from any handle with a minimum size", () => {
    const b = { x: 10, y: 10, w: 50, h: 40 };
    expect(resized(b, "se", 5, 5)).toEqual({ x: 10, y: 10, w: 55, h: 45 });
    expect(resized(b, "nw", 5, 5)).toEqual({ x: 15, y: 15, w: 45, h: 35 });
    expect(resized(b, "w", 100, 0)).toMatchObject({ w: 2, x: 58 });
  });

  it("rounds a scale up to a standard one", () => {
    expect(roundScaleUp(23100)).toBe(24000);
    expect(roundScaleUp(24000)).toBe(24000);
    expect(roundScaleUp(1_100_000)).toBe(2_000_000);
  });
});
