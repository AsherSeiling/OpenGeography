// LW-K: Shift+A opens the tool menu with the pointer over the Model canvas, even when a text box elsewhere still has focus.
import { test, expect, gis, addTestLayer, hoverCanvas, expectAddMenuOnScreen, modelState } from "./fixtures";

test("Shift+A works after a text box outside the editor was used, once the pointer is over the canvas", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  const box = app.getByPlaceholder(/Search commands and tools/);
  await box.click();
  await hoverCanvas(app);
  await app.waitForTimeout(1400); // typing wins for a moment, then the pointer decides
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
  await app.keyboard.press("Escape");
  await expect(app.locator(".toolmenu")).toHaveCount(0);
});

test("keys still go to a text box while it is being typed in, with the pointer over the canvas", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  const box = app.getByPlaceholder(/Search commands and tools/);
  await box.click();
  await hoverCanvas(app);
  await app.keyboard.type("ab"); // typing starts: a shortcut now would corrupt it
  await app.keyboard.press("Shift+A");
  await expect(app.locator(".toolmenu")).toHaveCount(0);
  await expect(box).toHaveValue(/^abA?$/i);
});

test("Shift+A on a plain canvas (nothing focused) still opens the Add menu", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await hoverCanvas(app);
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
});

/** A point of the canvas that is empty pane (no node, no control). */
async function emptySpot(app: any) {
  const box = (await app.locator(".model-canvas").boundingBox())!;
  for (const [fx, fy] of [[0.5, 0.85], [0.75, 0.85], [0.5, 0.2], [0.3, 0.6], [0.9, 0.5]]) {
    const x = box.x + box.width * fx, y = box.y + box.height * fy;
    if (await app.evaluate(([px, py]: number[]) => (document.elementFromPoint(px, py) as HTMLElement | null)?.classList.contains("react-flow__pane") ?? false, [x, y])) return { x, y };
  }
  throw new Error("no empty spot on the canvas");
}

test("right-click on the canvas opens the Add menu, and picking a tool adds a node", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  const before = await app.locator(".model .react-flow__node").count();
  const spot = await emptySpot(app);
  await app.mouse.click(spot.x, spot.y, { button: "right" });
  await expectAddMenuOnScreen(app);
  await app.keyboard.type("buffer");
  await app.keyboard.press("Enter");
  await expect(app.locator(".model .react-flow__node")).toHaveCount(before + 1);
});

test("with layers loaded but none selected, Shift+A and right-click still open the Add menu", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.evaluate(() => (window as any).__gis.useApp.getState().setActiveLayer(null));
  await expect(app.getByText("No layer selected")).toBeVisible();
  await hoverCanvas(app);
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
  await app.keyboard.press("Escape");
  await expect(app.locator(".toolmenu")).toHaveCount(0);
  await app.evaluate(() => (window as any).__gis.useApp.getState().setActiveLayer(null));
  const spot = await emptySpot(app);
  await app.mouse.click(spot.x, spot.y, { button: "right" });
  await expectAddMenuOnScreen(app);
});

test("a right click that moves a few pixels (a trackpad) still opens the Add menu", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  const spot = await emptySpot(app);
  await app.mouse.move(spot.x, spot.y);
  await app.mouse.down({ button: "right" });
  await app.mouse.move(spot.x + 2, spot.y + 1);
  await app.mouse.up({ button: "right" });
  await expectAddMenuOnScreen(app);
});

test("Shift+A with the pointer over another pane still opens the Add menu on the canvas", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.mouse.move(40, 400); // the Contents pane, far from the node canvas
  await app.waitForTimeout(200);
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
});

test("the toolbar says what happened to Shift+A and to a right-click", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await hoverCanvas(app);
  await app.keyboard.press("Shift+A");
  await expect(app.getByTestId("model-diag")).toContainText("Add menu opened");
  await app.keyboard.press("Escape");
  const spot = await emptySpot(app);
  await app.mouse.click(spot.x, spot.y, { button: "right" });
  await expect(app.getByTestId("model-diag")).toContainText("Add menu opened");
});

// ---- WP 3: the Shift+A tool menu (docs/UI_REDESIGN_PLAN.md §5) ----
const menu = (app: any) => app.locator(".toolmenu");
const search = (app: any) => app.getByRole("combobox", { name: /Search tools/ });
const names = (app: any): Promise<string[]> => app.locator(".toolmenu [role=option] .tm-name").allTextContents();
const nodeCount = (app: any) => app.locator(".model .react-flow__node").count();

test("every tool in the live catalogue is found by its full name and by a keyword", async ({ app }) => {
  test.setTimeout(180_000);
  await addTestLayer(app, "points", 20);
  const specs: { id: string; name: string; role: string; keywords?: string[] }[] = await gis(app, (g) => g.api.toolSpecs());
  const addable = specs.filter((s) => s.role !== "data" && s.id !== "layer_output" && s.id !== "add_to_map");
  expect(addable.length).toBeGreaterThan(60);
  await hoverCanvas(app);
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
  const missing: string[] = [];
  for (const s of addable) {
    await search(app).fill(s.name);
    if (!(await app.locator(`.toolmenu [role=option][data-tool="${s.id}"]`).count())) missing.push(`name: ${s.name}`);
    const kw = s.keywords?.[0];
    if (!kw) {
      missing.push(`no keywords: ${s.id}`);
      continue;
    }
    await search(app).fill(kw);
    if (!(await app.locator(`.toolmenu [role=option][data-tool="${s.id}"]`).count())) missing.push(`keyword "${kw}": ${s.name}`);
  }
  expect(missing).toEqual([]);
});

test("plain words: cut finds Clip and not Execute-like words; merge finds Union, Merge and Mosaic Rasters", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await hoverCanvas(app);
  await app.keyboard.press("Shift+A");
  await search(app).fill("cut");
  expect(await names(app)).toContain("Clip");
  // Tools with "cut" in their own name (Cut and Fill) rank above Clip, which is found by its keyword.
  await expect(app.locator(".toolmenu [role=option]").filter({ hasText: /^Clip/ }).first().locator(".tm-hit")).toHaveText(/cut/i);
  await search(app).fill("merge");
  const found = await names(app);
  for (const n of ["Union", "Merge", "Mosaic Rasters"]) expect(found).toContain(n);
  await expect(app.locator(".toolmenu b[role=status]")).toHaveText(/\d+ results?/);
});

test("keyboard only: Shift+A, type, Enter adds the node, selects it and closes the menu", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  const before = await nodeCount(app);
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
  await expect(search(app)).toBeFocused();
  await app.keyboard.type("buffer");
  await expect(search(app)).toHaveAttribute("aria-activedescendant", /opt0$/);
  await app.keyboard.press("ArrowDown");
  await app.keyboard.press("ArrowUp");
  await app.keyboard.press("Enter");
  await expect(menu(app)).toHaveCount(0);
  await expect(app.locator(".model .react-flow__node")).toHaveCount(before + 1);
  const st = await gis(app, (g) => ({ sel: g.useModel.getState().nodes.filter((n: any) => n.selected).map((n: any) => n.data.tool), props: g.useApp.getState().properties }));
  expect(st.sel).toEqual(["buffer"]);
  expect(st.props?.kind).toBe("node");
});

test("Mod+D marks a favourite, the Start view lists it, Recent lists what was added", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.evaluate(() => localStorage.removeItem("gis.toolmenu"));
  await app.keyboard.press("Shift+A");
  await app.keyboard.type("dissolve");
  await app.keyboard.press("ControlOrMeta+d");
  await expect(app.locator(".toolmenu .tm-star.fav")).toHaveCount(1);
  await app.keyboard.press("Enter");
  await app.keyboard.press("Shift+A");
  await expect(app.locator(".toolmenu .tm-sec", { hasText: /^Favourites$/ })).toBeVisible();
  await expect(app.locator(".toolmenu .tm-sec", { hasText: /^Recent$/ })).toBeVisible();
  await app.keyboard.press("ArrowRight"); // empty field: next category
  await expect(app.locator(".toolmenu .tm-rail button.on")).toContainText("Layers");
});

test("Shift+A works with the Node Editor closed, and picking opens it first", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.locator(".dv-default-tab, .dv-tab", { hasText: /^Processing nodes$/ }).first().locator(".dv-default-tab-action").click();
  await expect(app.locator(".model-canvas")).toBeHidden();
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
  await app.keyboard.type("buffer");
  await app.keyboard.press("Enter");
  await expect(app.locator(".model-canvas")).toBeVisible();
  await expect(menu(app)).toHaveCount(0);
  await expect.poll(async () => (await modelStateTools(app)).includes("buffer")).toBe(true);
});

test("Shift+A works with the Node Editor's tab hidden behind another tab", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.locator(".dv-default-tab, .dv-tab", { hasText: /^Attribute Table$/ }).first().click();
  await expect(app.locator(".model-canvas")).toBeHidden();
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
  await app.keyboard.type("buffer");
  await app.keyboard.press("Enter");
  await expect(app.locator(".model-canvas")).toBeVisible();
  await expect.poll(async () => (await modelStateTools(app)).includes("buffer")).toBe(true);
});

async function modelStateTools(app: any): Promise<string[]> {
  return gis(app, (g) => g.useModel.getState().nodes.map((n: any) => n.data.tool));
}

test("with no layer selected, picking a tool asks for the layer, then adds the tool", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.evaluate(() => (window as any).__gis.useApp.getState().setActiveLayer(null));
  await app.keyboard.press("Shift+A");
  await app.keyboard.type("buffer");
  await app.keyboard.press("Enter");
  await expect(menu(app)).toContainText("Tools go into a layer's node tree");
  await app.keyboard.press("Enter");
  await expect(menu(app)).toHaveCount(0);
  await expect.poll(async () => (await modelStateTools(app)).includes("buffer")).toBe(true);
});

test("with no layers at all, the menu offers Add Data", async ({ app }) => {
  await app.keyboard.press("Shift+A");
  await app.keyboard.type("buffer");
  await app.keyboard.press("Enter");
  await expect(menu(app).getByRole("button", { name: "Add Data" })).toBeVisible();
});

test("the menu stays on screen at a narrow window, with the rail as a dropdown", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.setViewportSize({ width: 600, height: 500 });
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
  await expect(app.locator(".toolmenu .tm-rail")).toBeHidden();
  await expect(app.locator(".toolmenu .tm-cat")).toBeVisible();
  await expect(app.locator(".toolmenu .tm-prev")).toBeHidden();
  await app.keyboard.type("buffer");
  await expect(app.locator(".toolmenu .tm-prevline")).toBeVisible();
  await app.setViewportSize({ width: 1600, height: 950 });
});

test("a tool added on a selected node connects after it; a raster tool on a vector node stays unconnected", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await app.keyboard.press("Shift+A");
  await app.keyboard.type("buffer");
  await app.keyboard.press("Enter"); // rule 3: between Layer Input and Output
  await expect.poll(async () => (await modelState(app)).edges.length).toBe(2);
  const before = (await modelState(app)).edges.length;
  await app.keyboard.press("Shift+A");
  await app.keyboard.type("hillshade");
  await expect(app.locator(".toolmenu .tm-conn")).toContainText(/unconnected/);
  await app.keyboard.press("Enter");
  await expect.poll(async () => (await modelState(app)).nodes.length).toBe(4);
  expect((await modelState(app)).edges.length).toBe(before);
});
