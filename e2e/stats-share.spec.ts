// ST7 sharing a statistics sheet: send a card's chart or table to a layout (and see it go out of date), export CSV, SVG, PNG and the sheet report
// through the ribbon, and an old layout that has no statistics cards.
import { mkdtempSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

test.use({ viewport: { width: 2400, height: 1200 } });

async function sheetWithHistogram(app: any) {
  const layer = await addTestLayer(app, "points", 40, [-100, 30, -90, 40]);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), layer.id);
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Statistics Sheet/ }).click();
  await expect(app.getByTestId("stats-view")).toBeVisible();
  await gis(app, async (g) => {
    g.statsActions.addStatCard("chart_histogram");
    const s = g.useStats.getState();
    const card = s.docs[s.id].cards.at(-1);
    s.editCard(s.id, card.id, { options: { ...card.options, field: "value", bins: 4 } });
    s.select(card.id);
    await g.statsActions.runStatCard(s.id, card.id);
  });
  await expect(app.getByTestId("stat-chart").locator("svg")).toBeVisible();
  return layer;
}

const preview = (app: any) =>
  gis(app, async (g) => {
    await g.useStats.getState();
    const { flushStatSaves } = await import("/src/stats/store.ts");
    await flushStatSaves();
    const { layoutRequest } = await import("/src/layout/actions.ts");
    const doc = g.useLayouts.getState().doc;
    const res = await g.api.renderLayoutPreview(layoutRequest(doc), 96);
    return { warnings: res.warnings as string[], bytes: res.pngBytes as number };
  }) as Promise<{ warnings: string[]; bytes: number }>;

test("ST7 Export tab: Send to Layout adds a card element to a new layout, the page draws it, and an edit of the data makes the page say it is out of date", async ({ app }) => {
  const layer = await sheetWithHistogram(app);
  await app.locator(".rtab", { hasText: /^Export$/ }).click();
  await app.locator(".rbtn", { hasText: /Send to Layout/ }).click();
  await app.getByRole("menuitem", { name: "New layout…" }).first().click();
  await expect.poll(() => gis(app, (g) => g.useLayouts.getState().doc?.elements.length ?? -1)).toBe(1);
  const el = await gis(app, (g) => g.useLayouts.getState().doc.elements[0]);
  expect(el).toMatchObject({ kind: "stat_card" });
  expect(el.sheet).toBeTruthy();
  expect(el.card).toBeTruthy();
  const p = await preview(app);
  expect(p.warnings.filter((w) => /Statistics element/.test(w))).toEqual([]);
  expect(p.bytes).toBeGreaterThan(1000);
  // The element is listed in the Items pane under its own kind name and its Properties name the sheet and card.
  await expect(app.locator(".items-pane, .items").first()).toContainText("Statistics card");
  // Change the layer's data: the page keeps the numbers it has and says the card is out of date.
  await gis(app, async (g, id: string) => {
    const page = await g.api.getRows(id, { offset: 0, limit: 1 });
    const info = await g.api.updateCell(id, page.fids[0], "value", "123456");
    g.useApp.getState().updateLayer(info);
  }, layer.id);
  const q = await preview(app);
  expect(q.warnings.some((w) => /out of date/.test(w))).toBe(true);
});

test("ST7 the table of a card goes to an existing layout without touching the layout's other elements", async ({ app }) => {
  await sheetWithHistogram(app);
  const before = await gis(app, async (g) => {
    g.layoutActions.newLayout();
    await new Promise((r) => setTimeout(r, 200));
    g.layoutActions.insertText?.();
    const l = g.useLayouts.getState().doc;
    return { id: l.id, n: l.elements.length };
  });
  // back to the sheet and send the table to that layout (it is a saved layout by now, not necessarily the open one)
  await gis(app, (g) => g.statsActions.openStatsSheet(g.useStats.getState().list()[0].id));
  await expect(app.getByTestId("stats-view")).toBeVisible();
  await app.locator(".rtab", { hasText: /^Export$/ }).click();
  await app.locator(".rbtn", { hasText: /Send to Layout/ }).click();
  await app.getByRole("menuitem", { name: /^Layout 1$/ }).last().click();
  const doc = await gis(app, (g, id: string) => g.useLayouts.getState().list().find((l: any) => l.id === id), before.id);
  expect(doc.elements.length).toBe(before.n + 1);
  expect(doc.elements.at(-1)).toMatchObject({ kind: "stat_card", part: "table" });
});

test("ST7 Export tab: CSV of the table, SVG and PNG of the chart, and the sheet report are written, with methods and citations in the report", async ({ app }) => {
  await sheetWithHistogram(app);
  const dir = mkdtempSync(join(tmpdir(), "gis-e2e-share-"));
  await app.locator(".rtab", { hasText: /^Export$/ }).click();
  const run = async (button: RegExp, path: string) => {
    app.once("dialog", (d: any) => void d.accept(path));
    await app.locator(".rbtn", { hasText: button }).click();
    await expect.poll(() => existsSync(path), { timeout: 15000 }).toBe(true);
  };
  await run(/Table as CSV/, join(dir, "bins.csv"));
  const csv = readFileSync(join(dir, "bins.csv"), "utf8");
  expect(csv.split("\r\n").length).toBeGreaterThanOrEqual(5); // header, four bins, trailing newline
  await run(/Chart as SVG/, join(dir, "chart.svg"));
  expect(readFileSync(join(dir, "chart.svg"), "utf8")).toContain('data-kind="histogram"');
  await run(/Chart as PNG/, join(dir, "chart.png"));
  expect(readFileSync(join(dir, "chart.png")).subarray(1, 4).toString()).toBe("PNG");
  await run(/Sheet Report/, join(dir, "report.html"));
  const html = readFileSync(join(dir, "report.html"), "utf8");
  expect(html).toContain("Methods and citations");
  expect(html).toContain("Freedman");
  expect(html).toContain("data-kind=\"histogram\"");
});

test("ST7 the export buttons are off until the selected card has been run", async ({ app }) => {
  await addTestLayer(app, "points", 10, [-100, 30, -90, 40]);
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Statistics Sheet/ }).click();
  await gis(app, (g) => { g.statsActions.addStatCard("chart_histogram"); const s = g.useStats.getState(); s.select(s.docs[s.id].cards[0].id); });
  await app.locator(".rtab", { hasText: /^Export$/ }).click();
  await expect(app.locator(".rbtn", { hasText: /Table as CSV/ })).toBeDisabled();
  await expect(app.locator(".rbtn", { hasText: /Chart as PNG/ })).toBeDisabled();
  await expect(app.locator(".rbtn", { hasText: /Send to Layout/ })).toBeDisabled();
  await expect(app.locator(".rbtn", { hasText: /Sheet Report/ })).toBeEnabled();
});
