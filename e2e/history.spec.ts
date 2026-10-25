import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer, addNode, frameAll, connect, modelState, nodeOf, hoverCanvas } from "./fixtures";

const MOD = "ControlOrMeta";

test("M-18 attribute history UI: node picker, diff chips, node menu, reopen without re-running", async ({ app }) => {
  app.on("dialog", (d) => d.accept()); // "discard unsaved changes?" when starting a new project
  const layer = await addTestLayer(app, "points", 300, [-100, 30, -95, 35]);
  const filter = await addNode(app, "definition", "filter", 230, 60);
  const buf = await addNode(app, "buffer", "buffer", 560, 60);
  await frameAll(app);
  const inId = (await nodeOf(app, "input_layer")).id, outId = (await nodeOf(app, "layer_output")).id;
  await connect(app, inId, filter);
  await connect(app, filter, buf);
  await connect(app, buf, outId);
  await gis(app, (g, a: any) => g.useModel.getState().setParam(a.f, "where", "value > 50"), { f: filter });
  await hoverCanvas(app);
  await app.keyboard.press(`${MOD}+Enter`);
  await expect.poll(() => gis(app, (g, id) => !!g.useApp.getState().results[id], layer.id), { timeout: 30_000 }).toBe(true);

  // Attribute Table shows the Output (node result, read-only)
  await app.locator(".dv-default-tab, .dv-tab", { hasText: /^Attribute Table$/ }).first().click();
  await expect(app.locator(".attr")).toContainText(/node result|Read-only/);
  const outRows = await gis(app, (g, id) => g.useApp.getState().results[id].featureCount, layer.id);
  expect(outRows).toBeGreaterThan(0);
  expect(outRows).toBeLessThan(300);

  // Node picker → Definition Query step; compare with input via the diff bar
  await app.locator(".attr-bar").getByRole("combobox").nth(1).click();
  await app.getByRole("option", { name: /Definition Query/ }).click();
  await expect(app.locator(".attr-banner.node")).toContainText(/Definition Query/);
  await app.locator(".attr-bar").getByRole("button", { name: /Show what this step changed/ }).click();
  const removed = app.locator(".diff-chip.removed");
  await expect(removed).toBeVisible();
  await expect(app.locator(".diff-chip.added")).toBeVisible();
  await expect(app.locator(".diff-chip.changed")).toBeVisible();
  const removedN = Number((await removed.innerText()).match(/\d+/)![0]);
  expect(removedN).toBeGreaterThan(0);
  await removed.click();
  await expect(removed).toHaveClass(/\bon\b/);
  await expect(app.locator(".attr-foot")).toContainText(String(removedN));
  await removed.click();

  // Node-card context menu opens the table at that node
  await app.locator(".dv-default-tab, .dv-tab", { hasText: /^Model$/ }).first().click();
  await app.locator(`.react-flow__node[data-id="${buf}"]`).locator(".node-head, .nhead, [class*=head]").first().click({ button: "right" });
  await app.getByRole("menuitem", { name: "View table" }).click();
  await expect(app.locator(".attr-bar")).toContainText(/Buffer/);

  // Save, close, reopen: the same node's rows load without running again
  const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-")), "hist.gproj");
  await gis(app, (g, d) => g.useApp.setState({ projectDir: d }), dir);
  await app.keyboard.press(`${MOD}+s`);
  await expect.poll(() => gis(app, (g) => g.useApp.getState().dirty)).toBe(false);
  await gis(app, (g) => g.actions.newProject?.());
  await gis(app, (g, d) => g.actions.openProject(d), dir);
  await expect.poll(() => gis(app, (g) => Object.keys(g.useApp.getState().layers).length)).toBe(1);
  await app.locator(".dv-default-tab, .dv-tab", { hasText: /^Attribute Table$/ }).first().click();
  expect(await gis(app, (g) => Object.keys(g.useRun.getState().results).length)).toBe(0); // no run happened since reopening
  await app.locator(".attr-bar").getByRole("combobox").nth(1).click();
  await app.getByRole("option", { name: /Buffer/ }).click();
  await expect(app.locator(".grid-row").first()).toBeVisible({ timeout: 15_000 });
  await expect(app.locator(".attr-missing")).toHaveCount(0);
});
