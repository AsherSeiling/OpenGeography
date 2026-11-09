// Georeference window (GR1, GR2). NOT YET RUN: written while e2e was on hold (2026-11-09); run it before merging.
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateSync } from "node:zlib";
import { test, expect, gis } from "./fixtures";

/** A 600 x 400 RGB PNG with no georeferencing. */
function writePng(path: string) {
  const w = 600, h = 400;
  const rows: Buffer[] = [];
  for (let y = 0; y < h; y++) {
    const row = Buffer.alloc(1 + w * 3);
    for (let x = 0; x < w; x++) row.set([(x * 255) / w, (y * 255) / h, 160], 1 + x * 3);
    rows.push(row);
  }
  const crcTable = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
  const crc = (b: Buffer) => { let c = 0xffffffff; for (const v of b) c = crcTable[(c ^ v) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const chunk = (t: string, d: Buffer) => { const len = Buffer.alloc(4); len.writeUInt32BE(d.length); const td = Buffer.concat([Buffer.from(t), d]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([len, td, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  writeFileSync(path, Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(Buffer.concat(rows))), chunk("IEND", Buffer.alloc(0))]));
}

test("GR-E1 georeference an image: live errors, check point, preview, new layer, source untouched, set saved", async ({ app }) => {
  const dir = mkdtempSync(join(tmpdir(), "gr-"));
  const src = join(dir, "scan.png");
  writePng(src);
  const before = readFileSync(src);
  await gis(app, async (g, p) => {
    const info = await g.api.georefImageInfo(p);
    g.useGeoref.getState().setImage({ path: p, info, sourceLayer: null, label: "scan.png" });
    g.useGeoref.getState().show();
  }, src);
  const win = app.getByTestId("georef");
  await expect(win).toBeVisible();
  await expect(win.getByTestId("gr-image-map")).toBeVisible();

  // Nothing is fitted until the coordinate system is chosen: said in words.
  await expect(win.getByTestId("gr-rms")).toContainText("Choose the coordinate system");

  await gis(app, (g) =>
    g.useGeoref.getState().edit((s: any) => ({
      ...s,
      targetCrs: "EPSG:26914",
      points: [[0, 0], [600, 0], [0, 400], [600, 400], [300, 200]].map(([p, l], i) => ({ id: i + 1, pixel: p, line: l, x: 600000 + 10 * p, y: 3900000 - 10 * l, enabled: true })),
    })),
  );
  await expect(win.getByTestId("gr-ref-map")).toBeVisible();
  await expect(win.getByTestId("gr-rms")).toContainText("RMS 0 m");

  // One wrong point inflates RMS; turning it off restores it and labels it a check point with its own error.
  await gis(app, (g) => g.useGeoref.getState().edit((s: any) => ({ ...s, points: s.points.map((p: any) => (p.id === 5 ? { ...p, x: p.x + 30 } : p)) })));
  await expect(win.getByTestId("gr-rms")).not.toContainText("RMS 0 m");
  await win.getByRole("checkbox", { name: "Use point 5 in the fit" }).uncheck();
  await expect(win.getByTestId("gr-rms")).toContainText("RMS 0 m");
  await expect(win.getByTestId("gr-rms")).toContainText("Check points");
  await expect(win.getByTestId("gr-table")).toContainText("check");

  // The spline warns that its enabled points show zero error.
  await win.getByText("1st-order polynomial").click();
  await app.getByRole("option", { name: /Thin plate spline/ }).click();
  await expect(win.getByText(/passes exactly through every enabled control point/)).toBeVisible();

  // Writes a new layer in the target CRS; the image and its folder are untouched.
  await win.getByTestId("gr-run").click();
  await expect.poll(() => gis(app, (g) => Object.values(g.useApp.getState().layers).map((l: any) => `${l.name}|${l.crs}`))).toContain("scan (georeferenced)|EPSG:26914");
  expect(readFileSync(src).equals(before)).toBe(true);
  expect(readdirSync(dir)).toEqual(["scan.png"]);

  // Save the set in the project.
  await win.getByRole("button", { name: "Save set" }).click();
  await expect.poll(() => gis(app, async (g) => (await g.api.listControlSets()).map((s: any) => `${s.name}|${s.points.length}`))).toEqual(["Control points 1|5"]);
});

test("GR-E2 an imported file that names no coordinate system asks for one", async ({ app }) => {
  const dir = mkdtempSync(join(tmpdir(), "gr-"));
  const file = join(dir, "pts.csv");
  writeFileSync(file, "id,pixel,line,x,y,enabled\n1,0,0,600000,3900000,1\n2,100,0,601000,3900000,1\n3,0,100,600000,3899000,1\n");
  const parsed = await gis(app, (g, p) => g.api.readControlFile(p), file);
  expect(parsed.crs).toBeNull();
  expect(parsed.points).toHaveLength(3);
  const qgis = join(dir, "q.points");
  writeFileSync(qgis, "#CRS: EPSG:4326\nmapX,mapY,sourceX,sourceY,enable,dX,dY,residual\n-100,40,10,-20,1,0,0,0\n");
  const q = await gis(app, (g, p) => g.api.readControlFile(p), qgis);
  expect(q).toMatchObject({ format: "qgis", crs: "EPSG:4326" });
  expect(q.points[0]).toMatchObject({ pixel: 10, line: 20 });
});
