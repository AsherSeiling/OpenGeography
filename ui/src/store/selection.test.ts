import { describe, expect, it } from "vitest";
import { useApp } from "./store";

describe("selection methods", () => {
  it("new, add, remove and select-from-current", () => {
    const s = () => [...(useApp.getState().selection.L ?? [])].sort((a, b) => a - b);
    useApp.getState().setSelection("L", [1, 2, 3], "new");
    useApp.getState().setSelection("L", [3, 4], "add");
    expect(s()).toEqual([1, 2, 3, 4]);
    useApp.getState().setSelection("L", [2, 9], "remove");
    expect(s()).toEqual([1, 3, 4]);
    useApp.getState().setSelection("L", [3, 4, 5], "subset");
    expect(s()).toEqual([3, 4]);
    useApp.getState().setSelection("L", [7], "new");
    expect(s()).toEqual([7]);
  });
});
