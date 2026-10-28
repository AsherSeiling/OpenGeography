import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  api: { saveLayout: vi.fn(async () => {}), deleteLayout: vi.fn(async () => {}), listLayouts: vi.fn(async () => []) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));

import { addText, useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { alignPatches, reorder } from "./arrange";
import { DPI_DEFAULT, parseDpi } from "./exportDpi";
import { newLayoutDoc, type LayoutElement } from "./types";

const el = (id: string, x: number, y: number, w = 20, h = 10, locked = false): LayoutElement => ({ kind: "text", id, x, y, w, h, rotation: 0, locked, visible: true, text: id, font: "sans", size: 10, align: "left", color: "#000" }) as LayoutElement;
const ids = (e: LayoutElement[]) => e.map((x) => x.id);
const page = { width: 200, height: 100, margin: 10 };

describe("Arrange: stacking order (C3.0)", () => {
  const four = [el("a", 0, 0), el("b", 0, 0), el("c", 0, 0), el("d", 0, 0)];
  it("front and back keep the moved ones in their own order", () => {
    expect(ids(reorder(four, ["b", "a"], "front"))).toEqual(["c", "d", "a", "b"]);
    expect(ids(reorder(four, ["d", "c"], "back"))).toEqual(["c", "d", "a", "b"]);
  });
  it("forward and backward move one place, past unselected elements only", () => {
    expect(ids(reorder(four, ["a"], "forward"))).toEqual(["b", "a", "c", "d"]);
    expect(ids(reorder(four, ["a", "b"], "forward"))).toEqual(["c", "a", "b", "d"]);
    expect(ids(reorder(four, ["d"], "forward"))).toEqual(["a", "b", "c", "d"]); // already on top
    expect(ids(reorder(four, ["c"], "backward"))).toEqual(["a", "c", "b", "d"]);
    expect(ids(reorder(four, ["a"], "backward"))).toEqual(["a", "b", "c", "d"]);
  });
  it("nothing or everything selected changes nothing, and the input is not changed", () => {
    expect(reorder(four, [], "front")).toBe(four);
    expect(reorder(four, ["a", "b", "c", "d"], "back")).toBe(four);
    expect(ids(four)).toEqual(["a", "b", "c", "d"]);
  });
});

describe("Arrange: align (C3.0)", () => {
  const els = [el("a", 10, 10, 20, 10), el("b", 50, 30, 40, 20), el("c", 120, 5, 10, 10, true)];
  it("two or more align to their common box; locked ones stay put", () => {
    expect(alignPatches(els, ["a", "b", "c"], "left", page)).toEqual({ b: { x: 10 } });
    expect(alignPatches(els, ["a", "b"], "right", page)).toEqual({ a: { x: 70 } });
    expect(alignPatches(els, ["a", "b"], "center", page)).toEqual({ a: { x: 40 }, b: { x: 30 } });
    expect(alignPatches(els, ["a", "b"], "top", page)).toEqual({ b: { y: 10 } });
    expect(alignPatches(els, ["a", "b"], "bottom", page)).toEqual({ a: { y: 40 } });
    expect(alignPatches(els, ["a", "b"], "middle", page)).toEqual({ a: { y: 25 }, b: { y: 20 } });
  });
  it("one element aligns to the page inside its margins", () => {
    expect(alignPatches(els, ["a"], "right", page)).toEqual({ a: { x: 170 } });
    expect(alignPatches(els, ["a"], "bottom", page)).toEqual({ a: { y: 80 } });
    expect(alignPatches(els, ["a"], "center", page)).toEqual({ a: { x: 90 } });
  });
  it("a locked or unknown selection gives nothing, and already-aligned elements are not touched", () => {
    expect(alignPatches(els, ["c"], "left", page)).toEqual({});
    expect(alignPatches(els, ["zzz"], "left", page)).toEqual({});
    expect(alignPatches([el("a", 10, 10)], ["a"], "left", { width: 200, height: 100, margin: 10 })).toEqual({});
  });
});

describe("Arrange in the store: one undo step", () => {
  beforeEach(() => {
    useLayouts.getState().loadAll([newLayoutDoc("a", "A")]);
    useLayouts.getState().open("a");
    useApp.setState({ dirty: false });
  });
  it("order and align undo and redo as one step each, and keep the selection", () => {
    const s = useLayouts.getState;
    const t1 = addText()!;
    const t2 = addText()!;
    s().update(t1, { x: 20, y: 30 });
    s().update(t2, { x: 60, y: 50 });
    s().select([t1, t2]);
    s().align("left");
    expect(s().doc!.elements.map((e) => e.x)).toEqual([20, 20]);
    s().undo();
    expect(s().doc!.elements.map((e) => e.x)).toEqual([20, 60]);
    s().redo();
    expect(s().doc!.elements.map((e) => e.x)).toEqual([20, 20]);
    s().select([t1]);
    s().arrange("front");
    expect(s().doc!.elements.map((e) => e.id)).toEqual([t2, t1]);
    s().undo();
    expect(s().doc!.elements.map((e) => e.id)).toEqual([t1, t2]);
    expect(s().selected).toEqual([t1]);
  });
  it("with nothing selected nothing happens", () => {
    const s = useLayouts.getState;
    addText();
    s().select([]);
    const before = s().doc;
    s().arrange("back");
    s().align("right");
    expect(s().doc).toBe(before);
  });
});

describe("Export resolution box (C3.0)", () => {
  it("accepts whole numbers from 72 to 600 and nothing else", () => {
    expect(parseDpi("300")).toBe(300);
    expect(parseDpi(" 150 ")).toBe(150);
    expect(parseDpi("72")).toBe(72);
    expect(parseDpi("600")).toBe(600);
    expect(parseDpi("71")).toBeNull();
    expect(parseDpi("601")).toBeNull();
    expect(parseDpi("")).toBeNull();
    expect(parseDpi("abc")).toBeNull();
    expect(parseDpi("30x")).toBeNull();
    expect(DPI_DEFAULT).toBe(300);
  });
});
