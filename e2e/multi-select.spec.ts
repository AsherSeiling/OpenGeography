// Selecting several layers in the Contents pane. Cases MS-* of docs/TEST_PLAN.md.
import { existsSync, mkdtempSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

async function three(app: any) {
  const ids: string[] = [];
  for (const bbox of [[-100, 30, -99, 31], [-98, 30, -97, 31], [-96, 30, -95, 31]]) ids.push((await addTestLayer(app, "points", 5, bbox)).id);
  return ids;
}
const rows = (app: any) => app.locator(".tree-layer");
const picked = (app: any) => gis(app, (g) => g.useApp.getState().selectedLayers);

test("MS-1 Ctrl/Cmd toggles, Shift selects a range, plain click resets", async ({ app }) => {
  const ids = await three(app);
  const order: string[] = await gis(app, (g) => g.useApp.getState().order);
  expect(order.length).toBe(3);
  await rows(app).nth(0).click();
  await rows(app).nth(2).click({ modifiers: ["Shift"] });
  expect(await picked(app)).toEqual(order);
  await rows(app).nth(1).click({ modifiers: ["ControlOrMeta"] });
  expect(await picked(app)).toEqual([order[0], order[2]]);
  await rows(app).nth(1).click();
  expect(await picked(app)).toEqual([order[1]]);
  expect(ids.length).toBe(3);
});

test("MS-2 actions on the selection: hide, move to top, apply symbology, export, remove", async ({ app }) => {
  await three(app);
  const order: string[] = await gis(app, (g) => g.useApp.getState().order);
  await rows(app).nth(1).click();
  await rows(app).nth(2).click({ modifiers: ["ControlOrMeta"] });
  let row = 2; // a picked row: after "Move To Top" the picked layers are rows 0 and 1
  const menu = async (name: RegExp) => { await rows(app).nth(row).click({ button: "right" }); await app.getByRole("menuitem", { name }).click(); };

  await menu(/Hide All/);
  expect(await gis(app, (g, o) => o.map((i: string) => !!g.useApp.getState().visible[i]), order)).toEqual([true, false, false]);

  await menu(/Move To Top/);
  row = 1;
  expect(await gis(app, (g) => g.useApp.getState().order)).toEqual([order[1], order[2], order[0]]);

  // Symbology of the right-clicked layer (order[2]) goes onto the other picked layer.
  await gis(app, (g, o) => { const s = g.useApp.getState(); s.setStyle(o[2], { ...s.styles[o[2]], fill: "#123456" }); }, order);
  await rows(app).nth(1).click({ button: "right" });
  await app.getByRole("menuitem", { name: /Apply Symbology/ }).click();
  expect(await gis(app, (g, o) => g.useApp.getState().styles[o[1]].fill, order)).toBe("#123456");

  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-ms-"));
  await menu(/Export Layers/);
  await app.getByPlaceholder("Folder to write into").fill(dir);
  await app.getByRole("button", { name: "Export", exact: true }).click();
  await expect.poll(() => readdirSync(dir).filter((f) => f.endsWith(".gpkg")).length).toBe(2);

  app.once("dialog", (d: any) => d.accept());
  await menu(/Remove 2 Layers/);
  await expect.poll(async () => (await gis(app, (g) => g.useApp.getState().order)).length).toBe(1);
  expect(existsSync(dir)).toBe(true);
});
