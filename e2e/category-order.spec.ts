// B7 category drawing order: the Symbology list sets it. Cases B7-* of docs/TEST_PLAN.md.
import { test, expect, gis, addTestLayer } from "./fixtures";

test("B7 unique values: move a category with the arrows, reverse the list, the order is saved in the style", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 30, [-100, 30, -95, 35]);
  await gis(app, async (g, id) => {
    const vals = await g.api.uniqueValues(id, "class", 60);
    const s = g.useApp.getState();
    s.setStyle(id, { ...s.styles[id], kind: "categorized", field: "class", classes: vals.map(([v]: [string, number], i: number) => ({ value: v, color: ["#e41a1c", "#377eb8", "#4daf4a", "#984ea3"][i % 4], label: String(v) })) });
    s.setActiveLayer(id);
    s.setProperties({ kind: "layer", id });
  }, layer.id);
  const order = () => gis(app, (g, id) => g.useApp.getState().styles[id].classes.map((c: any) => c.value), layer.id) as Promise<string[]>;
  const list = app.getByTestId("classes");
  await expect(list).toBeVisible();
  const first = await order();
  expect(first.length).toBeGreaterThan(2);

  // The first category lower by one: the second moves to the top.
  await app.getByRole("button", { name: `Draw ${first[0]} lower` }).click();
  await expect.poll(order).toEqual([first[1], first[0], ...first.slice(2)]);
  await expect(list.locator(".class-row").first()).toContainText(first[1]);
  // The top row can't go higher, the bottom row can't go lower.
  await expect(app.getByRole("button", { name: `Draw ${first[1]} higher` })).toBeDisabled();
  await expect(app.getByRole("button", { name: `Draw ${first[first.length - 1]} lower` })).toBeDisabled();

  // Reverse order.
  const before = await order();
  await app.getByRole("button", { name: "Reverse order" }).click();
  await expect.poll(order).toEqual([...before].reverse());

  // Saved with the project's styles and still one entry per value.
  const saved = await gis(app, (g, id) => JSON.parse(JSON.stringify(g.useApp.getState().styles[id])).classes.map((c: any) => c.value), layer.id);
  expect(new Set(saved).size).toBe(saved.length);
  expect(saved).toEqual([...before].reverse());
});
