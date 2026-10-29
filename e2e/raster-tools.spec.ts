import { test, expect, gis } from "./fixtures";

test("E1/E2 the raster foundation tools are in the tool catalog with their ports and parameters", async ({ app }) => {
  const specs = await gis(app, (g) => g.api.toolSpecs());
  const by = (id: string) => specs.find((s: any) => (s.spec ?? s).id === id);
  const want: [string, number, string[]][] = [
    ["raster_calc", 1, ["expr"]],
    ["raster_calc2", 2, ["expr"]],
    ["reclassify", 1, ["band", "rules"]],
    ["nodata", 1, ["mode", "value", "distance", "smoothing"]],
    ["select_bands", 1, ["bands"]],
    ["resample_raster", 1, ["cell", "resampling"]],
    ["align_raster", 2, ["resampling"]],
    ["mosaic_rasters", 2, ["resampling"]],
    ["rasterize", 1, ["field", "cell"]],
  ];
  for (const [id, ports, params] of want) {
    const s = by(id);
    expect(s, id).toBeTruthy();
    expect(s.category).toMatch(/^Raster › /);
    expect(s.inputs.length, id).toBe(ports);
    expect(s.params.map((p: any) => p.id), id).toEqual(params);
  }
  expect(by("reclassify").params.find((p: any) => p.id === "rules").kind).toBe("expression");
  expect(by("rasterize").inputs[0].kind).toBe("vector");
  expect(by("raster_calc").output).toBe("raster");
});
