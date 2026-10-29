import { describe, expect, it } from "vitest";
import { withOverride } from "./legendRows";

describe("legend overrides", () => {
  it("adds, merges and clears overrides", () => {
    let o = withOverride(undefined, "L1", { label: "Roads" });
    expect(o).toEqual({ L1: { label: "Roads" } });
    o = withOverride(o, "L1", { hidden: true });
    expect(o).toEqual({ L1: { label: "Roads", hidden: true } });
    o = withOverride(o, "L1", { hidden: false });
    expect(o).toEqual({ L1: { label: "Roads" } });
    o = withOverride(o, "L1", { label: "  " });
    expect(o).toEqual({});
  });
  it("leaves other keys alone", () => {
    const o = withOverride({ A: { hidden: true } }, "B", { label: "x" });
    expect(o).toEqual({ A: { hidden: true }, B: { label: "x" } });
  });
});
