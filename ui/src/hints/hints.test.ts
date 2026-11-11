// UX15: first-run tips show only while the place is empty, until dismissed, and can be turned off.
import { beforeEach, describe, expect, it } from "vitest";
import { HINTS, hintVisible, useHints } from "./hints";

beforeEach(() => useHints.getState().reset());

describe("hintVisible", () => {
  const base = { showHints: undefined as boolean | undefined, dismissed: [] as ("nodeEditor" | "layout")[], id: "nodeEditor" as const, empty: true };
  it("shows by default in an empty place", () => {
    expect(hintVisible(base)).toBe(true);
    expect(hintVisible({ ...base, showHints: true })).toBe(true);
  });
  it("hides when tips are off, dismissed, the place has content, or a test driver runs the page", () => {
    expect(hintVisible({ ...base, showHints: false })).toBe(false);
    expect(hintVisible({ ...base, dismissed: ["nodeEditor"] })).toBe(false);
    expect(hintVisible({ ...base, empty: false })).toBe(false);
    expect(hintVisible({ ...base, automated: true })).toBe(false);
  });
  it("dismissing one tip leaves the other", () => {
    expect(hintVisible({ ...base, dismissed: ["nodeEditor"], id: "layout" })).toBe(true);
  });
});

describe("the dismissed list", () => {
  it("remembers a dismissal once, and reset brings the tips back", () => {
    useHints.getState().dismiss("layout");
    useHints.getState().dismiss("layout");
    expect(useHints.getState().dismissed).toEqual(["layout"]);
    useHints.getState().reset();
    expect(useHints.getState().dismissed).toEqual([]);
  });
  it("every tip has a title and text", () => {
    for (const h of Object.values(HINTS)) expect(h.title && h.text.length > 20).toBeTruthy();
  });
});
