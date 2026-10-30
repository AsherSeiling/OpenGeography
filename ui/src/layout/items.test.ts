import { describe, expect, it } from "vitest";
import { pointerMm, clickSelection, displayOrder, itemName, matchItems, moveInList, rangeIds } from "./items";
import type { LayoutElement } from "./types";

const el = (id: string, kind: string, extra: Record<string, unknown> = {}) => ({ id, kind, x: 0, y: 0, w: 10, h: 10, rotation: 0, locked: false, visible: true, ...extra }) as unknown as LayoutElement;
const list = [el("a", "map_frame"), el("b", "text", { text: "Hello  world" }), el("c", "legend", { name: "  Key  " }), el("d", "scale_bar")];
const ids = (l: LayoutElement[]) => l.map((e) => e.id).join("");

describe("Items catalog logic", () => {
  it("names: the user's name, the text of a text element, else kind and id", () => {
    expect(itemName(list[2])).toBe("Key");
    expect(itemName(list[1])).toBe("Hello world");
    expect(itemName(list[0])).toBe("Map frame a");
    expect(itemName(el("t", "text", { text: "x".repeat(50) })).length).toBe(28);
    expect(itemName(el("t", "text", { text: "  " }))).toBe("Text t");
  });
  it("shows the top of the stacking order first", () => {
    expect(ids(displayOrder(list))).toBe("dcba");
    expect(ids(list)).toBe("abcd");
  });
  it("searches names, kinds and ids", () => {
    expect(ids(matchItems(list, "key"))).toBe("c");
    expect(ids(matchItems(list, "SCALE"))).toBe("d");
    expect(ids(matchItems(list, "hello"))).toBe("b");
    expect(ids(matchItems(list, ""))).toBe("abcd");
    expect(ids(matchItems(list, "zzz"))).toBe("");
  });
  it("moves an element above or below another as the list shows it", () => {
    // List (top first) is d c b a. Drop "a" above "d" (the very top): a drawn last.
    expect(ids(moveInList(list, "a", "d", false))).toBe("bcda");
    // Drop "d" below "a" (the very bottom): d drawn first.
    expect(ids(moveInList(list, "d", "a", true))).toBe("dabc");
    // Drop "b" above "c": b drawn after c.
    expect(ids(moveInList(list, "b", "c", false))).toBe("acbd");
    // Dropping onto itself, or where it already is, changes nothing.
    expect(moveInList(list, "b", "b", false)).toBe(list);
    expect(moveInList(list, "b", "a", false)).toBe(list);
    expect(moveInList(list, "x", "a", false)).toBe(list);
  });
  it("selects ranges and toggles", () => {
    expect(rangeIds(list, "d", "b")).toEqual(["d", "c", "b"]);
    expect(rangeIds(list, "b", "d")).toEqual(["d", "c", "b"]);
    expect(rangeIds(list, "zz", "b")).toEqual(["b"]);
    expect(clickSelection(list, ["a"], "a", "c", { toggle: false, shift: false })).toEqual(["c"]);
    expect(clickSelection(list, ["a"], "a", "c", { toggle: true, shift: false })).toEqual(["a", "c"]);
    expect(clickSelection(list, ["a", "c"], "a", "c", { toggle: true, shift: false })).toEqual(["a"]);
    expect(clickSelection(list, ["d"], "d", "b", { toggle: false, shift: true })).toEqual(["d", "c", "b"]);
  });
  it("turns the pointer into page millimetres, null off the page", () => {
    const page = { width: 279.4, height: 215.9 };
    expect(pointerMm(110, 70, { left: 10, top: 20 }, 2, page)).toEqual({ x: 50, y: 25 });
    expect(pointerMm(5, 70, { left: 10, top: 20 }, 2, page)).toBeNull();
    expect(pointerMm(10 + 2 * 300, 70, { left: 10, top: 20 }, 2, page)).toBeNull();
    expect(pointerMm(11, 21, { left: 10, top: 20 }, 0, page)).toBeNull();
  });
});
