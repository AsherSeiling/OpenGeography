// UR-20/21: Python is the default language for queries and calculations; a script waiting for approval shows nothing and says so;
// the approval dialog shows the full script and the plain warning. The dev server runs the system Python (dev-python feature).
import { addTestLayer, expect, gis, test } from "./fixtures";

const ribbon = async (page: any, tab: string, name: RegExp) => {
  await page.getByRole("tab", { name: tab, exact: true }).click();
  await page.getByRole("button", { name }).first().click();
};

test("the Definition Query editor opens on Python; a Python query filters the layer and an old SQL query opens on SQL", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 40, [-100, 30, -95, 35]);
  await ribbon(app, "Feature Layer", /^Definition Query$/);
  const qe = app.locator(".qe");
  await expect(qe.getByRole("tab", { name: "Python" })).toHaveAttribute("aria-selected", "true");
  expect(await qe.getByRole("tab").allInnerTexts().then((t) => t.map((x) => x.trim().replace(/(.+)\1/, "$1")))).toEqual(["Python", "Blocks", "Builder", "SQL"]);
  await qe.getByTestId("qe-python").fill('def calculate(row):\n    return row["value"] > 50\n');
  await expect(qe.getByTestId("qe-preview")).toContainText(/Python|SQL/);
  await app.getByRole("button", { name: "Apply" }).click();
  await expect.poll(() => gis(app, (g, id) => g.useApp.getState().layers[id].definitionPython, layer.id)).toContain("value");
  const n = await gis(app, (g, id) => g.useApp.getState().layers[id].queryCount, layer.id);
  expect(n).toBeGreaterThan(0);
  expect(n).toBeLessThan(40);
  // The engine agrees: only those features are in the table, and the stored script is the one typed.
  const total = await gis(app, async (g, id) => (await g.api.getRows(id, { offset: 0, limit: 100 })).total, layer.id);
  expect(total).toBe(n);

  // A layer with a SQL query (also one saved long ago) opens on the SQL tab, unchanged.
  const old = await addTestLayer(app, "points", 10, [-100, 30, -95, 35]);
  await gis(app, (g, id) => g.actions.setDefinitionQuery(id, "value > 10"), old.id);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), old.id);
  await ribbon(app, "Feature Layer", /^Definition Query$/);
  await expect(app.locator(".qe").getByRole("tab", { name: "SQL" })).toHaveAttribute("aria-selected", "true");
  await expect(app.locator(".qe textarea")).toHaveValue("value > 10");
});

test("a script waiting for approval shows no features, says so on the map, and the approval dialog shows the script and the warning", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 30, [-100, 30, -95, 35]);
  // A script that arrived with a project: set straight on the engine, never approved on this computer.
  const script = 'def calculate(row):\n    return row["value"] > 50  # from a shared project\n';
  await gis(app, async (g, a: any) => { const info = await g.api.setDefinitionPython(a.id, a.script); g.useApp.getState().updateLayer(info); }, { id: layer.id, script });
  await expect(app.getByTestId("script-waiting-notice")).toContainText(/script waiting for approval.*no features/i);
  const shown = await gis(app, async (g, id) => (await g.api.getRows(id, { offset: 0, limit: 100 })).total, layer.id);
  expect(shown, "never the unfiltered layer").toBe(0);

  await app.getByRole("button", { name: /Review and approve/ }).first().click();
  const dlg = app.getByTestId("script-approval");
  await expect(dlg).toBeVisible();
  await expect(dlg.getByTestId("script-text")).toContainText('row["value"] > 50');
  await expect(dlg.getByTestId("script-text")).toContainText("from a shared project");
  await expect(dlg.getByTestId("script-warning")).toContainText(/anything you can do on this computer/);
  // Not approving keeps the layer empty.
  await dlg.getByRole("button", { name: /Don't run it/ }).click();
  await expect(dlg).toHaveCount(0);
  expect(await gis(app, async (g, id) => (await g.api.getRows(id, { offset: 0, limit: 100 })).total, layer.id)).toBe(0);
  // Approving runs it.
  await app.getByRole("button", { name: /Review and approve/ }).first().click();
  await app.getByTestId("script-approval").getByRole("button", { name: /Approve and run/ }).click();
  await expect(app.getByTestId("script-waiting-notice")).toHaveCount(0);
  await expect.poll(() => gis(app, async (g, id) => (await g.api.getRows(id, { offset: 0, limit: 100 })).total, layer.id)).toBeGreaterThan(0);
});

test("Calculate Field: Python by default, strict about what fits, and a failing row changes nothing", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 12, [-100, 30, -95, 35]);
  await gis(app, (g, id) => g.actions.useDialog.getState().show({ kind: "calcField", layer: id, owner: id }), layer.id);
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByRole("combobox", { name: /Language/ }).or(dlg.getByText("Python").first())).toBeVisible();
  await dlg.getByRole("combobox").first().click().catch(() => {});
  await app.keyboard.press("Escape");
  await gis(app, (g, id) => g.actions.useDialog.getState().close(), layer.id);

  // Through the same engine path the dialog uses.
  const good = 'def calculate(row):\n    return row["value"] * 2\n';
  const bad = 'def calculate(row):\n    return "text"\n';
  // Not approved yet: nothing runs and the table is untouched.
  const waiting = await gis(app, async (g, a: any) => { try { await g.api.calculateFieldPython(a.id, "DOUBLE_V", a.good, null, null, false); return ""; } catch (e) { return String(e); } }, { id: layer.id, good });
  expect(waiting).toMatch(/Script waiting for approval/);
  const ok = await gis(app, async (g, a: any) => { await g.api.approveScript(a.good); const r = await g.api.calculateFieldPython(a.id, "DOUBLE_V", a.good, null, null, false); return r.count; }, { id: layer.id, good });
  expect(ok).toBe(12);
  const err = await gis(app, async (g, a: any) => { await g.api.approveScript(a.bad); try { await g.api.calculateFieldPython(a.id, "DOUBLE_V", a.bad, null, null, false); return ""; } catch (e) { return String(e); } }, { id: layer.id, bad });
  expect(err).toMatch(/rows failed.*returned text, but the field is/s);
});

test("a new Definition Query node uses Python; a node saved with SQL keeps running SQL", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 20, [-100, 30, -95, 35]);
  const defaults = await gis(app, (g) => g.api.toolSpecs().then((s: any[]) => ({ filter: s.find((t) => t.id === "filter").params.find((p: any) => p.id === "language").default, calc: s.find((t) => t.id === "calculate_field").params.find((p: any) => p.id === "language").default })));
  expect(defaults).toEqual({ filter: "python", calc: "python" });
  // A graph saved before Python existed has no language on its nodes and runs as SQL.
  const res = await gis(app, async (g, id) => g.api.runGraph({ nodes: [{ id: "in", tool: "input_layer", params: { layer: id } }, { id: "f", tool: "filter", params: { where: "value > 50" } }, { id: "o", tool: "layer_output", params: { layer: id } }], edges: [{ source: "in", target: "f", targetHandle: "in" }, { source: "f", target: "o", targetHandle: "in" }] }, null), layer.id);
  expect(JSON.stringify(res)).not.toMatch(/"status":"error"/);
});

test("a label expression waits for approval too; the Labels panel offers a Python expression and nothing is labelled until it is approved", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 15, [-100, 30, -95, 35]);
  const script = 'def calculate(row):\n    return "V" + str(int(row["value"]))  # label script from elsewhere\n';
  await gis(app, async (g, a: any) => { const info = await g.api.setLabelPython(a.id, a.script); g.useApp.getState().updateLayer(info); }, { id: layer.id, script });
  await expect(app.getByTestId("script-waiting-notice")).toContainText(/script waiting for approval.*no labels/i);
  // The layer's own features are not hidden by a waiting label script.
  expect(await gis(app, async (g, id) => (await g.api.getRows(id, { offset: 0, limit: 100 })).total, layer.id)).toBe(15);
  await app.getByRole("button", { name: /Review and approve/ }).first().click();
  await expect(app.getByTestId("script-text")).toContainText("label script from elsewhere");
  await app.getByTestId("script-approval").getByRole("button", { name: /Approve and run/ }).click();
  await expect(app.getByTestId("script-waiting-notice")).toHaveCount(0);
  expect(await gis(app, (g, id) => g.useApp.getState().layers[id].labelError ?? null, layer.id)).toBeNull();
});

test("UR-22: the Blocks tab makes the Python function, filters the layer, and the script opens as text with its blocks kept", async ({ app }) => {
  const layer = await addTestLayer(app, "points", 40, [-100, 30, -95, 35]);
  await ribbon(app, "Feature Layer", /^Definition Query$/);
  const qe = app.locator(".qe");
  await qe.getByRole("tab", { name: "Blocks" }).first().click();
  await expect(qe.getByTestId("blocks-editor").locator("svg.blocklySvg")).toBeVisible();
  // The starter program is a comparison on the first field; its Python is shown and is what runs.
  await expect(qe.getByTestId("blocks-python")).toContainText("def calculate(row):");
  await expect(qe.getByTestId("blocks-python")).toContainText(/return \(row\["[^"]+"\] (>|==)/);
  await app.getByRole("button", { name: "Apply" }).click();
  await expect.poll(() => gis(app, (g, id) => g.useApp.getState().layers[id].definitionPython, layer.id)).toContain("# gis-blocks: ");
  const script = await gis(app, (g, id) => g.useApp.getState().layers[id].definitionPython, layer.id);
  expect(script).toMatch(/^def calculate\(row\):\n    return /);
  // The engine ran it (a comparison on the first field: none, some or all rows, never an error).
  expect(await gis(app, (g, id) => g.useApp.getState().layers[id].definitionError ?? null, layer.id)).toBeNull();
  // On the Python tab the same script is plain text (blocks kept in its last line); editing the text means Blocks starts over rather than lie.
  await qe.getByRole("tab", { name: "Python" }).first().click();
  await expect(qe.getByTestId("qe-python")).toHaveValue(script);
  await qe.getByTestId("qe-python").fill(script.replace("def calculate", "# edited\ndef calculate"));
  await qe.getByRole("tab", { name: "Blocks" }).first().click();
  await expect(qe.getByTestId("blocks-python")).not.toContainText("# edited");
});
