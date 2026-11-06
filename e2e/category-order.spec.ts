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
  await expect(list.locator(".class-row").first().getByRole("textbox", { name: /Legend text of class/ })).toHaveValue(first[1]); // the legend text is an editable box now
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

test("UR-12 unique values start empty; add a class by hand or from the field's values, group values, leave the others out; saved in the style", async ({ app }) => {
  const layer = await addTestLayer(app, "polygons", 30, [-100, 30, -95, 35]);
  await gis(app, (g, id) => { const s = g.useApp.getState(); s.setActiveLayer(id); s.setProperties({ kind: "layer", id }); }, layer.id);
  const st = () => gis(app, (g, id) => JSON.parse(JSON.stringify(g.useApp.getState().styles[id])), layer.id) as Promise<any>;

  // Choosing Unique values makes no classes.
  await app.getByRole("combobox", { name: "Primary symbology" }).click();
  await app.getByRole("option", { name: "Unique values" }).click();
  await expect.poll(async () => (await st()).kind).toBe("categorized");
  expect((await st()).classes).toEqual([]);
  await expect(app.getByText(/No classes yet/)).toBeVisible();
  const field = (await st()).field as string;
  const vals = (await gis(app, (g, a: any) => g.api.uniqueValues(a.id, a.field, 500), { id: layer.id, field })) as [string, number][];
  expect(vals.length).toBeGreaterThan(3);

  // Pick one value from the field.
  await app.getByRole("combobox", { name: "Pick a value to add" }).click();
  await app.getByRole("option", { name: new RegExp(`^${vals[0][0]} \\(`) }).click();
  await expect.poll(async () => (await st()).classes.map((c: any) => c.value)).toEqual([vals[0][0]]);
  // Type one.
  await app.getByLabel("Type a value to add").fill(vals[1][0]);
  await app.getByRole("button", { name: "Add class" }).click();
  await expect.poll(async () => (await st()).classes.length).toBe(2);
  // A value that has a class is refused in words.
  await app.getByLabel("Type a value to add").fill(vals[1][0]);
  await app.getByRole("button", { name: "Add class" }).click();
  await expect(app.getByText(/already has a class/)).toBeVisible();
  expect((await st()).classes.length).toBe(2);

  // Add all values: the rest get classes, none twice.
  await app.getByRole("button", { name: /^Add all values/ }).click();
  await expect.poll(async () => (await st()).classes.length).toBe(vals.length);

  // Group the first two classes into one.
  const before = (await st()).classes.map((c: any) => c.value) as string[];
  await app.getByRole("checkbox", { name: /^Select / }).nth(0).check();
  await app.getByRole("checkbox", { name: /^Select / }).nth(1).check();
  await app.getByRole("button", { name: "Group ticked classes" }).click();
  await expect.poll(async () => (await st()).classes.length).toBe(vals.length - 1);
  const g1 = (await st()).classes[0];
  expect([g1.value, ...g1.values]).toEqual([before[0], before[1]]);

  // Remove one class, then switch "all other values" off: the style says so (the map filters to the listed values).
  const n = (await st()).classes.length;
  await app.getByRole("button", { name: new RegExp(`^Remove `) }).last().click();
  await expect.poll(async () => (await st()).classes.length).toBe(n - 1);
  await app.getByLabel("Legend text of all other values").fill("Everything else");
  await app.getByRole("checkbox", { name: "All other values" }).uncheck();
  await expect.poll(async () => (await st()).otherHidden).toBe(true);
  expect((await st()).otherLabel).toBe("Everything else");
  // And it comes back with the project's styles untouched.
  await app.getByRole("checkbox", { name: "All other values" }).check();
  await expect.poll(async () => (await st()).otherHidden).toBeUndefined();
});
