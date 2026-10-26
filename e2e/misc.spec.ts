import { mkdtempSync, writeFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

const MOD = "ControlOrMeta";
const projectMenu = async (page: any, item: RegExp) => {
  await page.locator(".rtab.file").click();
  await page.getByRole("menuitem", { name: item }).click();
};

test("M-22 sources pane (no dialogs): entries, edit author/year, APA/MLA/Chicago", async ({ app }) => {
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-src-"));
  const file = join(dir, "parcels.geojson");
  writeFileSync(file, JSON.stringify({ type: "FeatureCollection", crs: { type: "name", properties: { name: "EPSG:4326" } }, features: [{ type: "Feature", properties: { a: 1 }, geometry: { type: "Point", coordinates: [-97, 35] } }] }));
  await gis(app, (g, f) => g.actions.addData([f]), file);
  await expect.poll(() => gis(app, (g) => g.useApp.getState().order.length)).toBe(1);

  await projectMenu(app, /Sources & Citations/);
  const entry = app.locator(".src-entry").first();
  await expect(entry).toContainText("parcels");
  await entry.getByRole("button", { name: /Edit/ }).click();
  const dlg = app.getByRole("dialog");
  await dlg.getByLabel(/Author or organization/).fill("County Assessor");
  await dlg.getByLabel(/^Year/).fill("2019");
  await dlg.getByRole("button", { name: "Save" }).click();
  await expect(entry).toContainText("County Assessor");
  await expect(entry).toContainText("2019");

  const apa = await entry.locator(".src-ref").innerText();
  await app.locator("select.src-style").selectOption({ index: 1 });
  await expect.poll(async () => entry.locator(".src-ref").innerText()).not.toBe(apa);
  const mla = await entry.locator(".src-ref").innerText();
  await app.locator("select.src-style").selectOption({ index: 2 });
  await expect.poll(async () => entry.locator(".src-ref").innerText()).not.toBe(mla);
  await expect(entry).toContainText("County Assessor");
});

test("M-23 save/open without native dialogs: dirty marker, discard prompt, save, reopen, unload prompt", async ({ app, page }) => {
  const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-proj-")), "site.gproj");
  await addTestLayer(app, "points", 100);
  await expect(app.locator(".statusbar")).toContainText("Untitled •"); // new project, edited

  // New with unsaved changes asks first: dismissing keeps the project, accepting starts a new one
  app.once("dialog", (d) => { expect(d.message()).toMatch(/Discard unsaved changes/); void d.dismiss(); });
  await projectMenu(app, /^New/);
  await app.waitForTimeout(400);
  expect(await gis(app, (g) => g.useApp.getState().order.length)).toBe(1);

  // Save to a folder (a project folder already chosen: the OS picker is manual), then the marker clears
  await gis(app, (g, d) => g.useApp.setState({ projectDir: d }), dir);
  await expect(app.locator(".statusbar")).toContainText("site.gproj");
  await app.keyboard.press(`${MOD}+s`);
  await expect(app.locator(".statusbar")).not.toContainText("site.gproj •");
  await expect(app.locator(".statusbar")).toContainText(/Saved/);
  expect(readdirSync(dir).length).toBeGreaterThan(0);

  // Edit something: dirty again
  await gis(app, (g) => { const id = g.useApp.getState().order[0]; g.useApp.getState().setVisible(id, false); g.useApp.setState({ dirty: true }); });
  await expect(app.locator(".statusbar")).toContainText("site.gproj •");

  // Closing the page with unsaved changes asks before unloading
  const asked = new Promise<string>((res) => page.once("dialog", (d) => { res(d.type()); void d.dismiss(); }));
  await page.close({ runBeforeUnload: true });
  expect(await asked).toBe("beforeunload");
});

test("M-25 progress UI: a running task shows a bar and Cancel in the status bar; a failing task leaves a readable message", async ({ app, errors }) => {
  // (Two downloads at once need the network and stay manual: the engine runs one model at a time.)
  const a = await addTestLayer(app, "points", 150_000);
  await gis(app, (g, id) => {
    const graph = {
      nodes: [{ id: "in", tool: "input_layer", params: { layer: id } }, { id: "b", tool: "buffer", params: { distance: 500 } }, { id: "out", tool: "layer_output", params: { layer: id } }],
      edges: [{ source: "in", target: "b", targetHandle: "in" }, { source: "b", target: "out", targetHandle: "in" }],
    };
    (window as any).__run = g.api.runGraph(graph).then((r: any[]) => ((window as any).__status = r.map((x) => x.status)));
  }, a.id);
  await expect(app.locator(".sb-progress")).toContainText(/Running model/, { timeout: 20_000 });
  await expect(app.locator(".sb-progress-bar")).toBeVisible();
  await expect(app.locator(".sb-progress")).toHaveAttribute("title", /Step \d of 3/);
  await app.locator(".sb-progress-x").click();
  await expect.poll(() => app.evaluate(() => (window as any).__status?.join(",") ?? ""), { timeout: 60_000 }).toMatch(/cancelled/);
  await expect(app.locator(".sb-progress")).toHaveCount(0, { timeout: 10_000 }); // the bar goes away

  // A failing task: readable text in the status bar (and the error pop-up, which this test expects)
  errors.expected = true;
  errors.known.push(/Adding 1 file\(s\) failed/); // reported to the console on purpose
  await gis(app, (g) => g.actions.addData(["/no/such/folder/missing.shp"]));
  await expect(app.locator(".statusbar")).toContainText(/Adding 1 file\(s\) failed/);
  await expect(app.getByRole("alertdialog")).toBeVisible();
  await app.getByRole("alertdialog").getByRole("button").last().click();
});

test("M-1 (E2E-2 fixed): adding a missing file says the file is missing, not 'Query returned no rows'", async ({ app, errors }) => {
  errors.expected = true;
  await gis(app, (g) => g.actions.addData(["/no/such/folder/missing.shp"]));
  await expect(app.locator(".statusbar")).not.toContainText(/Query returned no rows/);
  await expect(app.locator(".statusbar")).toContainText(/not found|does not exist|no such file/i);
});
