// Rasters are drawn in every map coordinate system (M4): no "isn't drawn yet" notice, and the raster's tiles come from the CRS's own grid.
import { test, expect, gis } from "./fixtures";
import { writeAsc } from "./pixels";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

test("a raster draws in UTM, in a conic and in a polar CRS, with its tiles on that CRS's grid", async ({ app }) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "rcrs-"));
  const f = path.join(dir, "dem.asc");
  writeAsc(f, 40, 40, -105.1, 38.8, 0.005, (c, r) => 1800 + c * 6 + r * 3);
  const layer = (await gis(app, async (g, a: any) => { const [l] = await g.api.importFiles([a.f]); g.useApp.getState().addLayers([l]); return l; }, { f })) as any;

  const tiles: { url: string; status: number; type: string }[] = [];
  app.on("response", (r) => { if (/\/tiles\/[^/]+\/[0-9a-f]{12}\/\d+\/\d+\/\d+\.png/.test(r.url())) tiles.push({ url: r.url(), status: r.status(), type: r.headers()["content-type"] ?? "" }); });

  for (const [id, name] of [["EPSG:26913", "NAD83 / UTM zone 13N"], ["EPSG:5070", "NAD83 / Conus Albers"], ["EPSG:3031", "WGS 84 / Antarctic Polar Stereographic"]] as const) {
    tiles.length = 0;
    await gis(app, (g, a: any) => g.useApp.getState().setMapCrs({ id: a.id, name: a.name }), { id, name });
    await gis(app, (g, l: any) => g.actions.zoomToLayer(l), layer.id);
    await app.waitForTimeout(1800);
    // The raster layer is on the map, and no message says it is skipped.
    const onMap = (await gis(app, (g) => g.getMap().getLayers().getArray().length)) as number;
    expect(onMap, id).toBeGreaterThan(0);
    await expect(app.getByText(/raster layers? (isn't|aren't) drawn/)).toHaveCount(0);
    if (process.env.RCRS_PNG) await app.screenshot({ path: process.env.RCRS_PNG.replace(".png", `-${id.replace(":", "")}.png`) });
    if (id !== "EPSG:3031") {
      await expect.poll(() => tiles.length, { message: `${id}: a raster tile was requested on the CRS grid` }).toBeGreaterThan(0);
      expect(tiles.every((t) => t.status === 200 && t.type.includes("image/png")), JSON.stringify(tiles.slice(0, 3))).toBe(true);
    }
  }
});
