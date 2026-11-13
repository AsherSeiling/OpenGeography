// ST1 statistics workspace shell (docs/STATISTICS_WORKSPACE_PLAN.md): create a sheet, add a card, run it, save, reopen, see it go out of date after
// an edit, and come back to the map. Also the workspace's panes and tabs.
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

test.use({ viewport: { width: 2200, height: 1200 } });

const dockIds = (page: any) => gis(page, async () => (await import("/src/panes/panes.ts")).getDock()!.panels.map((p: any) => p.id)) as Promise<string[]>;
const tabNames = (page: any) => page.locator(".ribbon-tabs .rtab").evaluateAll((els: Element[]) => els.map((e) => (e.textContent ?? "").trim()));

async function newSheetFromRibbon(app: any) {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Statistics Sheet/ }).click();
  await expect(app.getByTestId("stats-view")).toBeVisible();
}

async function addSummaryCard(app: any) {
  await app.locator(".rbtn", { hasText: /Add Card/ }).click();
  await app.getByRole("menuitem", { name: "Summary Statistics" }).click();
  await expect(app.getByTestId("stat-card")).toHaveCount(1);
}

test("ST1 a new sheet changes the window like a layout, with the six statistics tabs and a Data pane, and Back to Map restores it", async ({ app }) => {
  await addTestLayer(app, "points", 8, [-100, 30, -90, 40]);
  const before = await dockIds(app);
  expect(before).toContain("contents");
  await newSheetFromRibbon(app);
  expect(await tabNames(app)).toEqual(["Explore", "Charts", "Tests", "Regression", "Spatial", "Export"]);
  const during = await dockIds(app);
  expect(during.some((id) => id.startsWith("stats:"))).toBe(true);
  expect(during).toContain("statdata");
  expect(during).not.toContain("contents");
  expect(during).not.toContain("model");
  await expect(app.locator(".rtab-back")).toBeVisible();
  // The not-yet-built tabs say which package builds them, in text.
  await app.locator(".rtab", { hasText: /^Charts$/ }).click();
  await expect(app.locator(".ribbon-body")).toContainText("ST3");
  // Back to Map: the map's tabs and panes return.
  await app.locator(".rtab-back").click();
  await expect(app.locator(".rtab-back")).toHaveCount(0);
  expect(await tabNames(app)).toContain("Map");
  const after = await dockIds(app);
  expect(after).toContain("contents");
  expect(after).not.toContain("statdata");
  // The sheet's tab stays; clicking it comes straight back.
  await app.locator(".dv-tab", { hasText: /Statistics 1/ }).click();
  await expect(app.getByTestId("stats-view")).toBeVisible();
  expect(await tabNames(app)).toContain("Explore");
});

test("ST1 the Data pane lists layers and tables with field types and missing counts, filters, and counts selected features only", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 8, [-100, 30, -90, 40]);
  await newSheetFromRibbon(app);
  const pane = app.locator(".stat-data");
  await pane.locator(".stat-data-name").first().click();
  await expect(pane.locator(".stat-fields")).toContainText("value");
  await expect(pane.locator(".stat-fields")).toContainText("double");
  await expect(pane).toContainText("8 rows");
  await pane.getByPlaceholder("Filter layers and fields").fill("zzz");
  await expect(pane).toContainText("Nothing matches");
  await pane.getByPlaceholder("Filter layers and fields").fill("clas");
  await expect(pane.locator(".stat-fields tbody tr")).toHaveCount(1);
  await pane.getByPlaceholder("Filter layers and fields").fill("");
  await gis(app, (g, id) => g.useApp.getState().setSelection(id, [1, 2, 3]), layer.id);
  await pane.getByLabel("Selected features only").check();
  await expect(pane).toContainText("3 selected rows");
});

test("ST1 create, add a card, run it, save, reopen, go out of date after an edit, run again, and Back to Map", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 8, [-100, 30, -90, 40]);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), layer.id);
  await newSheetFromRibbon(app);
  await addSummaryCard(app);
  const card = app.getByTestId("stat-card");
  await expect(card).toHaveAttribute("data-state", "not-run");
  // Properties shows the card's settings from its ToolSpec.
  await expect(app.locator(".props-title")).toContainText("Summary Statistics");
  // Running without a field is refused in a sentence, before it reaches the engine.
  await card.getByRole("button", { name: "Run", exact: true }).click();
  await expect(card).toContainText("Choose a field");
  await app.getByRole("combobox", { name: /Statistics field/ }).click();
  await app.getByRole("option", { name: "value", exact: true }).click();
  await card.getByRole("button", { name: "Run", exact: true }).click();
  await expect(card).toHaveAttribute("data-state", "current");
  await expect(card.locator(".stat-table th", { hasText: "COUNT" })).toBeVisible();
  await expect(card.locator(".stat-counts")).toContainText("n used: not reported by this tool");
  await expect(card.locator(".stat-method")).toContainText("Count, sum, mean");

  // Save, wreck the state, reopen.
  const dir = join(mkdtempSync(join(tmpdir(), "gis-e2e-stats-")), "stats.gproj");
  await gis(app, (g, d) => g.useApp.setState({ projectDir: d }), dir);
  await gis(app, (g) => g.actions.saveProject());
  await expect.poll(() => gis(app, (g) => g.useApp.getState().dirty)).toBe(false);
  await gis(app, (g) => g.actions.newProject());
  expect(await gis(app, (g) => g.useStats.getState().list().length)).toBe(0);
  await gis(app, (g, d) => g.actions.openProject(d), dir);
  expect(await gis(app, (g) => g.useStats.getState().list().length)).toBe(1);
  // The sheet's tab came back with the dock; open it.
  await app.locator(".dv-tab", { hasText: /Statistics 1/ }).click();
  const back = app.getByTestId("stat-card");
  await expect(back).toHaveAttribute("data-state", "current"); // same data version: still the layer's numbers
  await expect(back.locator(".stat-table th", { hasText: "MEAN" })).toBeVisible();

  // Edit a cell of the layer: the card says it is out of date and keeps the old numbers until asked.
  const sum = async () => back.locator(".stat-table tbody td").nth(1).textContent();
  const was = await sum();
  const layerId = await gis(app, (g) => Object.keys(g.useApp.getState().layers)[0]);
  await gis(app, async (g, id: string) => { const page = await g.api.getRows(id, { offset: 0, limit: 1 }); const info = await g.api.updateCell(id, page.fids[0], "value", "123456"); g.useApp.getState().updateLayer(info); }, layerId);
  await expect(back).toHaveAttribute("data-state", "out-of-date");
  await expect(back.locator(".stat-banner")).toContainText("Out of date");
  expect(await sum()).toBe(was);
  await back.getByRole("button", { name: "Run again" }).click();
  await expect(back).toHaveAttribute("data-state", "current");
  expect(await sum()).not.toBe(was);

  // Back to Map.
  await app.locator(".rtab-back").click();
  await expect(app.locator(".rtab-back")).toHaveCount(0);
  expect(await dockIds(app)).toContain("contents");
});

test("ST1 the Catalog lists sheets: New, rename and delete", async ({ app }) => {
  await addTestLayer(app, "points", 4, [-100, 30, -90, 40]);
  await app.getByRole("treeitem", { name: /Statistics sheets/ }).locator(".cat-new").click();
  await expect(app.getByTestId("stats-view")).toBeVisible();
  await app.getByRole("treeitem", { name: /Statistics 1/ }).hover();
  app.once("dialog", (d) => void d.accept("Parcels"));
  await app.getByRole("button", { name: "Rename Statistics 1" }).click();
  await expect(app.locator(".dv-tab", { hasText: /Parcels/ })).toBeVisible();
  expect(await gis(app, (g) => g.useStats.getState().list().map((s: any) => s.name))).toEqual(["Parcels"]);
  await app.getByRole("treeitem", { name: /Parcels/ }).hover();
  app.once("dialog", (d) => void d.accept());
  await app.getByRole("button", { name: "Delete Parcels" }).click();
  await expect.poll(() => gis(app, (g) => g.useStats.getState().list().length)).toBe(0);
  await expect(app.locator(".rtab-back")).toHaveCount(0);
});

test("ST1 a card can be added to a sheet only from a Statistics tool, and moves and removes from the ribbon", async ({ app }) => {
  await addTestLayer(app, "points", 4, [-100, 30, -90, 40]);
  await newSheetFromRibbon(app);
  await app.locator(".rbtn", { hasText: /Add Card/ }).click();
  const items = await app.getByRole("menuitem").allTextContents();
  expect(items).toContain("Summary Statistics");
  expect(items).not.toContain("Buffer");
  await app.getByRole("menuitem", { name: "Summary Statistics" }).click();
  await app.locator(".rbtn", { hasText: /Add Card/ }).click();
  await app.getByRole("menuitem", { name: "Summary Statistics" }).click();
  await expect(app.getByTestId("stat-card")).toHaveCount(2);
  // The second card is selected: Move Up works, Move Down does not.
  await expect(app.locator(".rbtn", { hasText: /Move Down/ })).toBeDisabled();
  await app.locator(".rbtn", { hasText: /Move Up/ }).click();
  await expect(app.locator(".rbtn", { hasText: /Move Up/ })).toBeDisabled();
  await app.locator(".rbtn", { hasText: /Remove Card/ }).click();
  await expect(app.getByTestId("stat-card")).toHaveCount(1);
});
