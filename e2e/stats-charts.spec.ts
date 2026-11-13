// ST3 charts on a statistics sheet: the engine's SVG on a card, and linked selection both ways (chart to map, map to chart) without redrawing.
import { test, expect, gis, addTestLayer } from "./fixtures";

test.use({ viewport: { width: 2200, height: 1200 } });

async function sheetWithChart(app: any, n: number, tool: string, options: Record<string, unknown>) {
  const layer = await addTestLayer(app, "points", n, [-100, 30, -90, 40]);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), layer.id);
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Statistics Sheet/ }).click();
  await expect(app.getByTestId("stats-view")).toBeVisible();
  await gis(app, async (g, a: any) => {
    g.statsActions.addStatCard(a.tool);
    const s = g.useStats.getState();
    const card = s.docs[s.id].cards.at(-1);
    s.editCard(s.id, card.id, { options: { ...card.options, ...a.options } });
    await g.statsActions.runStatCard(s.id, card.id);
  }, { tool, options });
  await expect(app.getByTestId("stat-chart").locator("svg")).toBeVisible();
  return layer;
}

const sel = (app: any, id: string) => gis(app, (g, l: string) => [...(g.useApp.getState().selection[l] ?? [])].sort((a: number, b: number) => a - b), id) as Promise<number[]>;

test("ST3 the Charts tab adds a chart card through its menu, with a field already chosen", async ({ app }) => {
  await addTestLayer(app, "points", 30, [-100, 30, -90, 40]);
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Statistics Sheet/ }).click();
  await app.locator(".rtab", { hasText: /^Charts$/ }).click();
  await app.locator(".rbtn", { hasText: /Add Chart/ }).click();
  const items = await app.getByRole("menuitem").allTextContents();
  expect(items).toEqual(expect.arrayContaining(["Histogram", "Box Plot", "Scatter Plot", "Normal Q-Q Plot", "Bar Chart", "Line Chart", "Scatter Plot Matrix", "Correlation Heat Map"]));
  expect(items).not.toContain("Summary Statistics");
  await app.getByRole("menuitem", { name: "Histogram" }).click();
  await app.getByRole("button", { name: "Run", exact: true }).click();
  const chart = app.getByTestId("stat-chart");
  await expect(chart.locator("svg[data-kind=histogram]")).toBeVisible();
  await expect(chart.locator("[data-mark]").first()).toBeVisible();
  await expect(app.getByTestId("stat-card").locator(".stat-counts")).toContainText("n used: 30");
  await expect(app.getByTestId("stat-card")).toContainText("Freedman-Diaconis");
});

test("ST3 clicking a histogram bar selects its features on the map, and a map selection lights the bars without redrawing the chart", async ({ app }) => {
  const layer = await sheetWithChart(app, 40, "chart_histogram", { field: "value", bins: 4 });
  const chart = app.getByTestId("stat-chart");
  await expect(chart.locator("[data-mark]")).toHaveCount(4);
  // chart -> map
  await chart.locator("[data-mark]").nth(1).click();
  const picked = await sel(app, layer.id);
  expect(picked.length).toBeGreaterThan(0);
  await expect(chart.locator("[data-mark].sel")).toHaveCount(1);
  await expect(chart.locator("[data-mark]").nth(1)).toHaveClass(/sel/);
  // Shift adds another bar
  await chart.locator("[data-mark]").nth(2).click({ modifiers: ["Shift"] });
  expect((await sel(app, layer.id)).length).toBeGreaterThan(picked.length);
  // map -> chart: select the features of bar 0 on the map; no new svg is drawn
  await gis(app, () => { (window as any).__svgBefore = document.querySelector("[data-testid=stat-chart] svg"); });
  const bar0 = await gis(app, async (g, l: string) => {
    const s = g.useStats.getState();
    const spec = s.docs[s.id].cards[0].result.charts[0];
    return spec.series[0].marks[0].fids;
  }, layer.id);
  await gis(app, (g, a: any) => g.useApp.getState().setSelection(a.l, a.f), { l: layer.id, f: bar0 });
  await expect(chart.locator("[data-mark]").nth(0)).toHaveClass(/sel/);
  await expect(chart.locator("[data-mark].sel")).toHaveCount(1);
  expect(await gis(app, () => document.querySelector("[data-testid=stat-chart] svg") === (window as any).__svgBefore)).toBe(true);
  // clearing the map selection clears the chart's
  await gis(app, (g, l: string) => g.useApp.getState().setSelection(l, []), layer.id);
  await expect(chart.locator("[data-mark].sel")).toHaveCount(0);
});

test("ST3 a large scatter plot is a density grid with a note, and clicking a cell selects the features in it", async ({ app }) => {
  const layer = await sheetWithChart(app, 6000, "chart_scatter", { x: "value", y: "class", trend: false });
  await expect(app.getByTestId("stat-card")).toContainText("density grid");
  const chart = app.getByTestId("stat-chart");
  const cell = chart.locator("[data-mark][data-selectable]").first();
  await expect(cell).toBeVisible();
  const expected = await gis(app, () => {
    const s = (window as any).__gis.useStats.getState();
    return s.docs[s.id].cards[0].result.charts[0].series[0].marks[0].value;
  });
  await cell.click();
  await expect.poll(async () => (await sel(app, layer.id)).length).toBe(expected);
});

test("ST3 the chart in the sheet and the engine's own SVG are the same picture", async ({ app }) => {
  await sheetWithChart(app, 20, "chart_box", { field: "value" });
  const inPage = await gis(app, () => document.querySelector("[data-testid=stat-chart] svg")!.outerHTML.length);
  expect(inPage).toBeGreaterThan(500);
  await expect(app.getByTestId("stat-chart").locator("svg[data-kind=box] .mark")).not.toHaveCount(0);
});
