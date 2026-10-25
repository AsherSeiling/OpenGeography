// P13: the map view is OpenLayers only; no source file may import MapLibre.
import { describe, expect, it } from "vitest";

const sources = import.meta.glob("../**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const pkg = import.meta.glob("../../../package.json", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

describe("no MapLibre", () => {
  it("no source file imports maplibre", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(50);
    const bad = Object.entries(sources)
      .filter(([f, text]) => !f.endsWith("noMaplibre.test.ts") && /from\s+["']maplibre|require\(["']maplibre|import\(["']maplibre/i.test(text))
      .map(([f]) => f);
    expect(bad).toEqual([]);
  });
  it("package.json has no maplibre dependency", () => {
    const text = Object.values(pkg)[0];
    expect(text).toBeTruthy();
    const j = JSON.parse(text);
    expect(Object.keys({ ...j.dependencies, ...j.devDependencies }).filter((k) => /maplibre/i.test(k))).toEqual([]);
  });
});
