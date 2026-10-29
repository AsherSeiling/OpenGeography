import { test, expect, gis } from "./fixtures";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

test("RS3 raster symbology pane: render type, ramp library, histogram handles, classes", async ({ app }) => {
  // A small ASCII grid (the engine imports it as a raster); no CRS, so the map draws nothing, but the pane works.
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "rs3-"));
  const file = path.join(dir, "ramp.asc");
  const rows = Array.from({ length: 20 }, (_, r) => Array.from({ length: 20 }, (_, c) => c * 10 + r).join(" "));
  fs.writeFileSync(file, `ncols 20\nnrows 20\nxllcorner -100\nyllcorner 30\ncellsize 0.01\nNODATA_value -9999\n${rows.join("\n")}\n`);
  const info = await gis(app, async (g, a: any) => {
    const [l] = await g.api.importFiles([a.file]);
    g.useApp.getState().addLayers([l]);
    g.useApp.getState().setProperties({ kind: "layer", id: l.id });
    return l;
  }, { file }) as any;

  await expect(app.getByText("Render type")).toBeVisible();
  const style = () => gis(app, (g, a: any) => g.useApp.getState().styles[a.id].raster ?? null, { id: info.id }) as Promise<any>;
  expect(await style()).toBeNull();

  await app.getByRole("combobox", { name: "Render type" }).click();
  await app.getByRole("option", { name: "Singleband pseudocolor" }).click();
  expect((await style()).renderer).toBe("pseudocolor");

  // The ramp library comes from the engine, grouped, with colour-blind-safe marks.
  await app.getByRole("combobox", { name: "Colour ramp" }).click();
  await expect(app.getByRole("option", { name: /cividis ✓/ })).toBeVisible();
  await expect(app.getByRole("option", { name: /^Spectral$/ })).toBeVisible();
  await app.getByRole("option", { name: /magma/ }).click();
  expect((await style()).ramp).toBe("magma");

  // Dragging a histogram handle switches the stretch to manual and sets min.
  const hist = app.getByRole("img", { name: /^Histogram/ });
  await expect(hist).toBeVisible();
  const b = (await hist.boundingBox())!;
  await app.mouse.move(b.x + 2, b.y + b.height / 2);
  await app.mouse.down();
  await app.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 5 });
  await app.mouse.up();
  const s = await style();
  expect(s.stretch.mode).toBe("manual");
  expect(s.stretch.min).toBeGreaterThan(50);

  // Classified colours: breaks come from the engine.
  await app.getByRole("combobox", { name: "Colour classes" }).click();
  await app.getByRole("option", { name: "Classified" }).click();
  await expect.poll(async () => (await style()).breaks?.length).toBe(4);
  expect((await style()).mode).toBe("discrete");

  // Unique values: classes come from the band's distinct values.
  await app.getByRole("combobox", { name: "Render type" }).click();
  await app.getByRole("option", { name: "Paletted / Unique values" }).click();
  await app.getByRole("button", { name: "Load classes" }).click();
  await expect.poll(async () => (await style()).entries?.length).toBe(200);
  await expect(app.getByText(/more than 200 distinct values/)).toBeVisible();
  await app.getByRole("checkbox", { name: "Show class 0" }).uncheck();
  expect((await style()).entries[0].hidden).toBe(true);
});
