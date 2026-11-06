import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

// UR-15: the Export Features dialog's format list, coordinate system, field and clip choices, end to end (the engine side is in export_options.rs).
test("UR-15 Export Features: KML in WGS 84 with chosen fields; GPX is refused for polygons; the status says what changed", async ({ app }) => {
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-ur15-"));
  const poly = await addTestLayer(app, "polygons", 8, [-100, 30, -95, 35]);
  const pts = await addTestLayer(app, "points", 12, [-100, 30, -95, 35]);
  const nameOf = (l: any) => l.fields.map((f: any) => f.name) as string[];

  // Polygons: GPX is listed but cannot be chosen, and the reason is on screen.
  await gis(app, (g, id) => g.actions.exportLayer(id), poly.id);
  const dlg = app.getByRole("dialog");
  await dlg.getByRole("combobox", { name: "File format" }).click();
  await expect(app.getByRole("option", { name: /GPX for GPS devices/ })).toHaveAttribute("aria-disabled", "true");
  for (const f of ["Google Earth KML", "AutoCAD DXF", "FlatGeobuf", "GeoParquet", "CSV with the shape as WKT"]) await expect(app.getByRole("option", { name: new RegExp(f) })).toBeVisible();
  await app.keyboard.press("Escape");
  await dlg.getByRole("button", { name: "Cancel" }).click();

  // Points → KML, moved to WGS 84, two fields only.
  await gis(app, (g, id) => g.actions.exportLayer(id), pts.id);
  await app.getByRole("combobox", { name: "File format" }).click();
  await app.getByRole("option", { name: /Google Earth KML/ }).click();
  await expect(app.getByRole("dialog")).toContainText("Always WGS 84");
  const fields = nameOf(pts).filter((n) => n !== "fid");
  expect(fields.length).toBeGreaterThan(1);
  await app.getByRole("checkbox", { name: /All fields/ }).uncheck();
  for (const n of fields.slice(1)) await app.getByRole("checkbox", { name: n, exact: true }).uncheck();
  const path = join(dir, "points.kml");
  app.once("dialog", (d) => void d.accept(path));
  await app.getByRole("button", { name: "Choose file…" }).click();
  await expect.poll(() => existsSync(path), { timeout: 15_000 }).toBe(true);
  const kml = readFileSync(path, "utf8");
  expect(kml).toContain("<kml");
  expect(kml).toContain(fields[0]);
  expect(kml).not.toContain(`name="${fields[1]}"`);
  await expect(app.locator(".statusbar")).toContainText(/Exported 12 feature\(s\)/);
  await expect(app.locator(".statusbar")).toContainText(/fields written/);
});
