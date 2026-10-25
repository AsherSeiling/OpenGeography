import { beforeEach, describe, expect, it } from "vitest";
import { useNav } from "./navStore";

beforeEach(() => useNav.setState({ points: [], done: false, rays: [], fix: null, triangle: [], known: [], measureMode: "distance", navPanel: null, pick: null }));

describe("measure sketch", () => {
  it("accumulates points until the measurement is finished, then starts over", () => {
    const n = () => useNav.getState();
    n().addPoint([0, 0]);
    n().addPoint([1, 1]);
    expect(n().points).toEqual([[0, 0], [1, 1]]);
    n().set({ done: true });
    n().addPoint([5, 5]);
    expect(n().points).toEqual([[5, 5]]);
    expect(n().done).toBe(false);
  });

  it("clearMeasure leaves the land-nav sketch alone and vice versa", () => {
    const n = () => useNav.getState();
    n().addPoint([0, 0]);
    n().set({ fix: [1, 2], rays: [[[0, 0], [1, 1]]], known: [[3, 4]], triangle: [[0, 0], [1, 0], [0, 1]] });
    n().clearMeasure();
    expect(n().points).toEqual([]);
    expect(n().fix).toEqual([1, 2]);
    n().addPoint([9, 9]);
    n().clearNav();
    expect(n()).toMatchObject({ rays: [], fix: null, triangle: [], known: [] });
    expect(n().points).toEqual([[9, 9]]);
  });

  it("defaults are sensible", () => {
    expect(useNav.getState()).toMatchObject({ measureMode: "distance", north: "true", angleUnit: "deg" });
  });
});
