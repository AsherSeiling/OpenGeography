// UX13: favourite and recent tools on the ribbon.
import { beforeEach, describe, expect, it } from "vitest";
import { useToolMenu } from "../nodes/toolMenuStore";
import { useApp } from "../store/store";
import { useModel } from "../store/model";
import { addToolFromRibbon, knownTools, RIBBON_FAVOURITES } from "./favourites";

const specs = {
  buffer: { name: "Buffer", role: "tool" },
  clip: { name: "Clip", role: "tool" },
  input_layer: { name: "Layer Input", role: "data" },
  layer_output: { name: "Layer Output", role: "output" },
};

describe("knownTools", () => {
  it("keeps order, drops tools that no longer exist, data and output plumbing, and repeats", () => {
    expect(knownTools(["clip", "gone", "input_layer", "layer_output", "clip", "buffer"], specs)).toEqual([{ id: "clip", name: "Clip" }, { id: "buffer", name: "Buffer" }]);
  });
  it("caps the count", () => {
    expect(knownTools(["buffer", "clip"], specs, 1)).toHaveLength(1);
    expect(RIBBON_FAVOURITES).toBeGreaterThan(0);
  });
});

describe("addToolFromRibbon", () => {
  beforeEach(() => useModel.setState({ layer: null }));
  it("says why when there is no layer tree to add to, and does not count it as recent", async () => {
    useToolMenu.setState({ recent: [] });
    await addToolFromRibbon("buffer");
    expect(useApp.getState().status).toMatch(/Select a layer first/);
    expect(useToolMenu.getState().recent).toEqual([]);
  });
});
