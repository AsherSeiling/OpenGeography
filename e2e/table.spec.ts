import { test, expect, gis, ribbon, addKnownPoints, addTestLayer } from "./fixtures";

const rows = (page: any, id: string, query: object = {}) => gis(page, (g, a: any) => g.api.getRows(a.id, { offset: 0, limit: 100, ...a.q }), { id, q: query }) as Promise<any>;
const grid = (page: any) => page.locator(".grid-inner");
const cell = (page: any, row: number, col: number) => page.locator(".grid-row").nth(row).locator(".cell:not(.fid)").nth(col);

async function openTable(page: any) {
  await ribbon(page, "Feature Layer", /^Attribute Table$/);
  await expect(page.locator(".attr")).toBeVisible();
  await expect(page.locator(".grid-row").first()).toBeVisible();
}

test("M-11 attribute table: edit, cancel, Tab, sort, filter, fields and errors", async ({ app }) => {
  const info = await addKnownPoints(app);
  await openTable(app);
  // columns: name, pop (fid is separate)
  const nameCol = 0, popCol = 1;
  await cell(app, 0, nameCol).dblclick({ delay: 80 }); // a human double-click takes longer than one animation frame (focusCell refocuses the grid on rAF)
  await app.locator(".cell-editor").fill("zzz");
  await app.keyboard.press("Enter");
  await expect.poll(async () => (await rows(app, info.id)).rows.some((r: any[]) => r.includes("zzz"))).toBe(true);

  // Esc cancels
  await cell(app, 1, nameCol).dblclick({ delay: 80 }); // a human double-click takes longer than one animation frame (focusCell refocuses the grid on rAF)
  await app.locator(".cell-editor").fill("never");
  await app.keyboard.press("Escape");
  await app.waitForTimeout(300);
  expect((await rows(app, info.id)).rows.some((r: any[]) => r.includes("never"))).toBe(false);

  // Tab moves right: edit name, Tab, the pop cell becomes the editor
  await cell(app, 2, nameCol).dblclick({ delay: 80 }); // a human double-click takes longer than one animation frame (focusCell refocuses the grid on rAF)
  await app.locator(".cell-editor").fill("tabbed");
  await app.keyboard.press("Tab");
  await expect.poll(async () => (await rows(app, info.id)).rows.some((r: any[]) => r.includes("tabbed"))).toBe(true);
  await app.keyboard.press("Escape");

  // Sort: click header; twice for descending
  const head = app.locator(".cell.head", { hasText: "pop" });
  await head.click();
  await expect.poll(async () => Number(await cell(app, 0, popCol).innerText())).toBeLessThanOrEqual(10);
  await head.click();
  await expect.poll(async () => Number(await cell(app, 0, popCol).innerText())).toBeGreaterThanOrEqual(30);

  // Filter
  await app.getByPlaceholder(/Filter, e.g./).fill("pop > 20");
  await app.keyboard.press("Enter");
  await expect(app.locator(".grid-row")).toHaveCount(2);
  await expect(app.locator(".attr-foot")).toContainText("2 shown");
  await app.getByPlaceholder(/Filter, e.g./).fill("");
  await app.keyboard.press("Enter");
  await expect(app.locator(".grid-row")).toHaveCount(4);

  // Add Field
  await app.locator(".attr-bar").getByRole("button", { name: "Add Field" }).click();
  await app.getByRole("dialog").getByPlaceholder("e.g. AREA_ACRES").fill("score");
  await app.getByRole("dialog").getByRole("button", { name: "Add" }).click();
  await expect(app.locator(".cell.head", { hasText: "score" })).toBeVisible();

  // Calculate Field: a bad expression shows its error in the dialog; a good one fills the table
  await app.locator(".attr-bar").getByRole("button", { name: "Calculate Field" }).click();
  const dlg = app.getByRole("dialog");
  await dlg.getByRole("combobox").first().click().catch(() => {});
  await dlg.locator("input").first().fill("score");
  await dlg.locator("textarea").fill("nonexistent_col * 2");
  await dlg.getByRole("button", { name: "Calculate" }).click();
  await expect(dlg.locator(".fui-MessageBar, [role=group]").first()).toBeVisible();
  await dlg.locator("textarea").fill("pop * 2");
  await dlg.getByRole("button", { name: "Calculate" }).click();
  await expect(dlg).toHaveCount(0);
  await expect.poll(async () => { const r = await rows(app, info.id); const si = (await gis(app, (g, id) => g.useApp.getState().layers[id].fields.map((f: any) => f.name), info.id) as string[]).indexOf("score"); return r.rows.map((x: any[]) => x[si]).sort().join(","); }).toMatch(/10(\.0)?,20(\.0)?,50(\.0)?,60(\.0)?|\b60/);

  // Calculate Geometry on a polygon layer
  const poly = await addTestLayer(app, "polygons", 5);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), poly.id);
  await app.locator(".attr-bar").getByRole("button", { name: /Calculate Geometry/ }).click();
  await app.getByRole("dialog").getByRole("button", { name: "Calculate" }).click();
  await expect(app.getByRole("dialog")).toHaveCount(0);
  await expect.poll(async () => (await gis(app, (g, id) => g.useApp.getState().layers[id].fields.length, poly.id))).toBeGreaterThan(3);
});

test("M-12 definition query: builder rows, Python and SQL tabs, apply and clear", async ({ app, errors }) => {
  const info = await addKnownPoints(app);
  await ribbon(app, "Data", /^Definition Query$/);
  const qe = app.locator(".qe");
  await expect(qe).toBeVisible();
  const row0 = qe.locator(".qe-row").nth(0);
  await row0.locator("select").nth(0).selectOption("pop");
  await row0.locator("select").nth(1).selectOption("gt");
  await row0.locator(".qe-val").fill("20");
  await qe.getByRole("button", { name: /Add clause/ }).click();
  const row1 = qe.locator(".qe-row").nth(1);
  await row1.locator("select").nth(0).selectOption("AND");
  await row1.locator("select").nth(1).selectOption("name");
  await row1.locator("select").nth(2).selectOption("ne");
  await row1.locator(".qe-val").selectOption("b"); // text fields with few values offer a list
  await expect(qe.locator(".qe-preview")).toContainText(/pop.*>.*20.*AND.*name/is);
  await row1.locator("select").nth(0).selectOption("OR");
  await expect(qe.locator(".qe-preview")).toContainText(/OR/);
  await row1.locator("select").nth(0).selectOption("AND");

  await qe.getByRole("tab", { name: "SQL" }).click();
  await expect(qe.locator("textarea")).toHaveValue(/pop.*20.*AND.*name/is);
  await qe.getByRole("tab", { name: "Python" }).click();
  await qe.locator("textarea").fill("pop > 20 and name != 'b'");
  await expect(qe.locator(".qe-preview")).toContainText(/pop/);
  await qe.getByRole("tab", { name: "Builder" }).click();

  await app.getByRole("button", { name: "Apply" }).click();
  await expect.poll(() => gis(app, (g, id) => g.useApp.getState().layers[id].queryCount, info.id)).toBe(1);
  await openTable(app);
  await expect(app.locator(".grid-row")).toHaveCount(1);
  expect((await gis(app, (g, id) => g.useApp.getState().layers[id].definitionQuery, info.id))).toBeTruthy();

  await ribbon(app, "Data", /^Definition Query$/);
  await app.getByRole("button", { name: "Clear", exact: true }).click();
  await expect.poll(() => gis(app, (g, id) => g.useApp.getState().layers[id].definitionQuery, info.id)).toBeNull();
  await expect(app.locator(".grid-row")).toHaveCount(4);
});

test("M-12 (E2E-1 fixed): applying a definition query logs no React duplicate-key warning", async ({ app }) => {
  const info = await addKnownPoints(app);
  await gis(app, (g, id) => g.actions.setDefinitionQuery(id, "pop > 20"), info.id);
  await app.waitForTimeout(1500);
  // the fixture fails the test on any uncaught console error
});
