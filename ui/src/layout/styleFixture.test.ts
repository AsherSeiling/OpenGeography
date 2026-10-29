// Drift guard: the engine's LayerStyle mirror (crates/engine/src/layout/model.rs)
// deserializes crates/engine/tests/fixtures/layer_style.json and checks that it
// has exactly these keys. If LayerStyle changes, both tests fail until updated.
import { describe, expect, it } from "vitest";
import type { LayerStyle } from "../store/store";
import { defaultStyle } from "../store/store";
import type { LayerInfo } from "../ipc/api";
import fixtureText from "../../../crates/engine/tests/fixtures/layer_style.json?raw";

const ALL_KEYS: Record<keyof LayerStyle, true> = {
  kind: true,
  fill: true,
  stroke: true,
  strokeWidth: true,
  opacity: true,
  radius: true,
  field: true,
  classes: true,
  breaks: true,
  colors: true,
  label: true,
  raster: true,
  effect: true,
};

describe("LayerStyle engine mirror", () => {
  const fixture = JSON.parse(fixtureText);

  it("fixture has every LayerStyle key", () => {
    expect(Object.keys(fixture).sort()).toEqual(Object.keys(ALL_KEYS).sort());
  });

  it("defaultStyle only uses keys the engine knows", () => {
    const s = defaultStyle({ geomType: "Polygon" } as LayerInfo);
    for (const k of Object.keys(s)) expect(Object.keys(fixture)).toContain(k);
  });
});
