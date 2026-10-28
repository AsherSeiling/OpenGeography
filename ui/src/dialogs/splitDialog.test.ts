import { describe, expect, it } from "vitest";
import { splitVerdict } from "./SplitDialog";

const groups = (n: number) => Array.from({ length: n }, (_, i) => ({ value: `v${i}`, rows: 1 }));

describe("Split by Attribute dialog (B6)", () => {
  it("waits for the counts, then allows a normal split", () => {
    expect(splitVerdict(null, false)).toEqual({ ok: false, message: null });
    expect(splitVerdict(groups(12), false)).toEqual({ ok: true, message: null });
    expect(splitVerdict(groups(200), false).ok).toBe(true);
  });
  it("asks for a confirmation above 200 outputs and says how many", () => {
    const v = splitVerdict(groups(201), false);
    expect(v.ok).toBe(false);
    expect(v.message).toContain("201 outputs");
    expect(splitVerdict(groups(201), true)).toEqual({ ok: true, message: null });
  });
  it("refuses more than 1000 outputs even when confirmed, and an empty layer", () => {
    const v = splitVerdict(groups(1001), true);
    expect(v.ok).toBe(false);
    expect(v.message).toContain("limit is 1,000");
    expect(splitVerdict([], false).ok).toBe(false);
  });
});
