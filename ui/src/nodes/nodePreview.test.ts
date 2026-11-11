import { describe, expect, it } from "vitest";
import { previewBlocked, previewNeedsRun } from "./nodePreview";

describe("previewNeedsRun", () => {
  it("runs when never run, failed, or changed since", () => {
    expect(previewNeedsRun(undefined, false, undefined, "a")).toBe(true);
    expect(previewNeedsRun("error", false, "a", "a")).toBe(true);
    expect(previewNeedsRun("done", true, "a", "b")).toBe(true);
    expect(previewNeedsRun("done", false, "a", "a")).toBe(true);
  });
  it("reuses a finished, unchanged result", () => {
    expect(previewNeedsRun("done", true, "a", "a")).toBe(false);
    expect(previewNeedsRun("cached", true, "a", "a")).toBe(false);
  });
});

describe("previewBlocked", () => {
  it("allows vector and table outputs", () => {
    expect(previewBlocked("__node_1", "vector")).toBeNull();
  });
  it("says why a raster or a step without output can't be shown", () => {
    expect(previewBlocked("/tmp/x.tif", "raster")).toMatch(/Raster/);
    expect(previewBlocked(null, null)).toMatch(/no output/);
  });
});
