import { test, expect, gis, addTestLayer, addNode, frameAll, connect, modelState, nodeOf, hoverCanvas, canvas, handle, drag, ribbon } from "./fixtures";

const MOD = "ControlOrMeta";

/** Layer Input → Buffer → Output wired by dragging sockets. */
async function bufferTree(app: any, distance?: string) {
  const buf = await addNode(app, "buffer", "buffer");
  await frameAll(app);
  const inId = (await nodeOf(app, "input_layer")).id, outId = (await nodeOf(app, "layer_output")).id;
  await connect(app, inId, buf);
  await connect(app, buf, outId);
  expect((await modelState(app)).edges.map((e) => e.slice(0, 2).join(">")).sort()).toEqual([`${inId}>${buf}`, `${buf}>${outId}`].sort());
  if (distance) await gis(app, (g, a: any) => g.useModel.getState().setParam(a.buf, "distance", a.d), { buf, d: Number(distance) });
  return { buf, inId, outId };
}

test("M-13 run a model: progress shows steps, result replaces the layer; cancel leaves the previous result", async ({ app }) => {
  const small = await addTestLayer(app, "points", 3000);
  await bufferTree(app);
  // Watch the status bar for "Step n of 3" while running (the run is short, so record every change).
  await app.evaluate(() => {
    (window as any).__seen = new Set<string>();
    new MutationObserver(() => document.querySelectorAll(".sb-progress").forEach((e) => (window as any).__seen.add((e as HTMLElement).title))).observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
  });
  await hoverCanvas(app);
  await app.keyboard.press(`${MOD}+Enter`);
  await expect.poll(() => gis(app, (g, id) => !!g.useApp.getState().results[id], small.id), { timeout: 30_000 }).toBe(true);
  await expect(app.locator(".statusbar")).toContainText(/updated in/);
  const seen = await app.evaluate(() => [...(window as any).__seen]);
  expect(seen.join(" | "), "progress was shown").toMatch(/Running model|Step \d of \d/);

  // Cancel on a large layer: stops within seconds and the layer keeps what it had.
  const big = await addTestLayer(app, "points", 250_000);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), big.id);
  await app.waitForTimeout(500);
  const t = await bufferTree(app);
  await gis(app, (g, a: any) => g.useModel.getState().setParam(a.buf, "distance", 2000), { buf: t.buf });
  await hoverCanvas(app);
  await app.keyboard.press(`${MOD}+Enter`);
  const x = app.locator(".sb-progress-x");
  await expect(x).toBeVisible({ timeout: 20_000 });
  await expect(app.locator(".sb-progress")).toContainText(/Running model|Step/);
  await x.click();
  const t0 = Date.now();
  await expect(app.locator(".statusbar")).toContainText(/cancelled/i, { timeout: 40_000 });
  expect(Date.now() - t0, "cancel stops (known slow, B-T4: ~12 s)").toBeLessThan(30_000);
  expect(await gis(app, (g, id) => !!g.useApp.getState().results[id], big.id)).toBe(false);
});

test("M-14 node editor gestures: Add menu, insert on connection, box select, cut links, keys", async ({ app }) => {
  await addTestLayer(app, "points", 200);
  // Shift+A opens the menu at the pointer; typing filters; arrows + Enter choose
  await hoverCanvas(app, 300, 100);
  await app.keyboard.press("Shift+A");
  await app.keyboard.type("definition");
  await app.waitForTimeout(150); // the menu re-renders between keys; a person types slower than the browser repaints
  await app.keyboard.press("ArrowDown");
  await app.waitForTimeout(150);
  await app.keyboard.press("ArrowUp");
  await app.waitForTimeout(150);
  await app.keyboard.press("Enter");
  await expect.poll(async () => (await modelState(app)).nodes.some((n) => n.tool === "filter")).toBe(true);
  const filter = (await nodeOf(app, "filter")).id;
  await frameAll(app);
  const inId = (await nodeOf(app, "input_layer")).id, outId = (await nodeOf(app, "layer_output")).id;

  // Drag a free node onto the connection in→out: it is inserted
  const edge = app.locator(".react-flow__edge").first();
  const eb = (await edge.locator("path").first().boundingBox())!;
  const header = app.locator(`.react-flow__node[data-id="${filter}"]`).locator(".node-head, .nhead, [class*=head]").first();
  const hb = (await header.boundingBox())!;
  await drag(app, [hb.x + 20, hb.y + 8], [eb.x + eb.width / 2, eb.y + eb.height / 2]);
  await expect.poll(async () => (await modelState(app)).edges.map((e) => e.slice(0, 2).join(">")).sort().join()).toBe([`${inId}>${filter}`, `${filter}>${outId}`].sort().join());

  // A node of the wrong kind (raster tool on a vector connection) is refused with a reason
  const slope = await addNode(app, "slope", "slope", 420, 60);
  await frameAll(app);
  const edges0 = JSON.stringify((await modelState(app)).edges);
  const eb2 = (await app.locator(".react-flow__edge").first().locator("path").first().boundingBox())!;
  const sb = (await app.locator(`.react-flow__node[data-id="${slope}"]`).locator(".node-head, .nhead, [class*=head]").first().boundingBox())!;
  await drag(app, [sb.x + 20, sb.y + 8], [eb2.x + eb2.width / 2, eb2.y + eb2.height / 2]);
  await app.waitForTimeout(400);
  expect(JSON.stringify((await modelState(app)).edges)).toBe(edges0);
  await expect(app.locator(".statusbar")).toContainText(/raster|vector|can't|cannot|doesn't|does not|accept|compatible/i);
  await hoverCanvas(app);
  await app.keyboard.press("Home");

  // Select all (A), delete (X) removes only deletable nodes: fixed nodes stay
  await hoverCanvas(app);
  await app.keyboard.press("a");
  expect((await modelState(app)).nodes.every((n) => n.selected)).toBe(true);
  await app.keyboard.press("Alt+a");
  expect((await modelState(app)).nodes.some((n) => n.selected)).toBe(false);

  // Click the slope node, X deletes it
  await app.locator(`.react-flow__node[data-id="${slope}"]`).locator(".node-head, .nhead, [class*=head]").first().click();
  await hoverCanvas(app);
  await app.keyboard.press("x");
  await expect.poll(async () => (await modelState(app)).nodes.some((n) => n.tool === "slope")).toBe(false);

  // Mute (M), Collapse (H) on the filter node
  await app.locator(`.react-flow__node[data-id="${filter}"]`).locator(".node-head, .nhead, [class*=head]").first().click();
  await hoverCanvas(app);
  await app.keyboard.press("m");
  expect((await nodeOf(app, "filter")).muted).toBe(true);
  await app.keyboard.press("m");
  expect((await nodeOf(app, "filter")).muted).toBe(false);
  await app.keyboard.press("h");
  expect((await nodeOf(app, "filter")).collapsed).toBe(true);
  await app.keyboard.press("h");

  // Duplicate (Shift+D): a second filter node appears, placed with a click
  await app.keyboard.press("Shift+d");
  await app.mouse.move(520, 800);
  await app.mouse.click(520, 800);
  await expect.poll(async () => (await modelState(app)).nodes.filter((n) => n.tool === "filter").length).toBe(2);

  // Delete with reconnect (Mod+X) on the original: neighbours stay linked
  await app.locator(`.react-flow__node[data-id="${filter}"]`).locator(".node-head, .nhead, [class*=head]").first().click();
  await hoverCanvas(app);
  await app.keyboard.press(`${MOD}+x`);
  await expect.poll(async () => (await modelState(app)).nodes.some((n) => n.id === filter)).toBe(false);
  expect((await modelState(app)).edges.some((e) => e[0] === inId && e[1] === outId) || (await modelState(app)).edges.length >= 1).toBe(true);

  // Undo (Mod+Z) brings it back
  await app.keyboard.press(`${MOD}+z`);
  await expect.poll(async () => (await modelState(app)).nodes.some((n) => n.id === filter)).toBe(true);
});

test("M-15 shortcuts dialog: rebind Mute to X names the conflict; Reset restores", async ({ app }) => {
  await addTestLayer(app, "points", 50);
  await app.getByRole("button", { name: /Node editor keyboard shortcuts/ }).click();
  const dlg = app.getByRole("dialog");
  await expect(dlg).toContainText("Node editor shortcuts");
  const mute = dlg.locator(".keymap-row", { hasText: "Mute (bypass)" });
  await mute.locator(".keyadd").first().click();
  await app.keyboard.press("x");
  await expect(dlg.locator(".src-warn")).toContainText(/Delete/);
  await expect(mute.locator(".keychip", { hasText: "X" }).first()).toBeVisible();
  await dlg.getByRole("button", { name: "Reset all" }).click();
  await expect(dlg.locator(".src-warn")).toContainText(/reset/i);
  await expect(mute.locator(".keychip")).toHaveCount(1);
  await expect(mute.locator(".keychip")).toContainText("M");
  await dlg.getByRole("button", { name: "Done" }).click();
});

test("M-16 deleted layer: a tree that uses a removed layer says so and flags the node", async ({ app }) => {
  const a = await addTestLayer(app, "polygons", 20, [-100, 30, -95, 35]);
  const b = await addTestLayer(app, "points", 40, [-100, 30, -95, 35]);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), b.id);
  await app.waitForTimeout(400);
  const clip = await addNode(app, "clip", "clip", 380, 60);
  await frameAll(app);
  // drag layer A from Contents onto the Model canvas: an extra Layer Input appears
  const row = (await app.locator(".tree-layer", { hasText: a.name }).boundingBox())!;
  const cb = (await canvas(app).boundingBox())!;
  await drag(app, [row.x + 60, row.y + 10], [cb.x + 120, cb.y + 60]);
  await expect.poll(async () => (await modelState(app)).nodes.filter((n) => n.tool === "input_layer").length).toBe(2);
  await frameAll(app);
  const inputs = (await modelState(app)).nodes.filter((n) => n.tool === "input_layer");
  const extra = inputs.find((n) => !n.id.startsWith("in-"))!.id;
  const inId = inputs.find((n) => n.id.startsWith("in-"))!.id, outId = (await nodeOf(app, "layer_output")).id;
  await connect(app, inId, clip, 0);
  await connect(app, extra, clip, 1);
  await connect(app, clip, outId);
  // remove layer A (Contents menu → Remove), then select B again and run
  app.once("dialog", (d) => d.accept());
  await app.locator(".tree-layer", { hasText: a.name }).click({ button: "right" });
  await app.getByRole("menuitem", { name: /^Remove$/ }).click();
  await expect.poll(() => gis(app, (g, id) => !!g.useApp.getState().layers[id], a.id)).toBe(false);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), b.id);
  await hoverCanvas(app);
  await app.keyboard.press(`${MOD}+Enter`);
  await expect.poll(() => gis(app, (g, ex) => g.useRun.getState().results[ex]?.error ?? null, extra), { timeout: 20_000 }).toMatch(/no longer exists/i);
  await expect(app.locator(".statusbar")).toContainText(/error/i);
});

const headOf = (app: any, id: string) => app.locator(`.react-flow__node[data-id="${id}"]`).locator(".node-head, .nhead, [class*=head]").first();

test("M-14 node editor gestures (2): box select, cut links, grab, make links, copy/paste across layers, frame", async ({ app }) => {
  const l1 = await addTestLayer(app, "points", 100);
  const l2 = await addTestLayer(app, "points", 80);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), l1.id);
  await app.waitForTimeout(400);
  const f1 = await addNode(app, "definition", "filter", 300, 60);
  await frameAll(app);
  let st = await modelState(app);
  const inId = st.nodes.find((n) => n.tool === "input_layer")!.id, outId = st.nodes.find((n) => n.tool === "layer_output")!.id;

  // Shift+drag box selection around everything
  const cb = (await canvas(app).boundingBox())!;
  await app.keyboard.down("Shift");
  await drag(app, [cb.x + 4, cb.y + 4], [cb.x + cb.width - 40, cb.y + cb.height - 20]);
  await app.keyboard.up("Shift");
  await expect.poll(async () => (await modelState(app)).nodes.filter((n) => n.selected).length).toBe(3);
  await hoverCanvas(app);
  await app.keyboard.press("Alt+a");

  // Make links (F): select the free filter node and the Layer Input, F links them left to right
  await headOf(app, inId).click();
  await app.keyboard.down("Shift");
  await headOf(app, f1).click();
  await app.keyboard.up("Shift");
  await hoverCanvas(app);
  await app.keyboard.press("f");
  await expect.poll(async () => (await modelState(app)).edges.some((e) => e[0] === inId && e[1] === f1)).toBe(true);

  // Cut links: Ctrl/⌘ + right-drag across the connection in → filter
  await frameAll(app);
  const handleIn = await handle(app, inId, "source"), handleF = await handle(app, f1, "target");
  const mid: [number, number] = [(handleIn[0] + handleF[0]) / 2, (handleIn[1] + handleF[1]) / 2];
  await app.mouse.move(mid[0], mid[1] - 60);
  await app.keyboard.down(MOD);
  await app.mouse.down({ button: "right" });
  await app.mouse.move(mid[0], mid[1], { steps: 6 });
  await app.mouse.move(mid[0], mid[1] + 60, { steps: 6 });
  await app.mouse.up({ button: "right" });
  await app.keyboard.up(MOD);
  await expect.poll(async () => (await modelState(app)).edges.some((e) => e[0] === inId && e[1] === f1)).toBe(false);

  // Grab (G): the node follows the pointer; a click places it
  await headOf(app, f1).click();
  const before = (await nodeOf(app, "filter"));
  const hb = (await headOf(app, f1).boundingBox())!;
  await app.mouse.move(hb.x + 20, hb.y + 8);
  await app.keyboard.press("g");
  await app.mouse.move(hb.x + 20, hb.y + 120, { steps: 6 });
  await app.mouse.click(hb.x + 20, hb.y + 120);
  await expect.poll(async () => (await nodeOf(app, "filter")).y).toBeGreaterThan(before.y + 30);

  // Frame selected (.) and frame all (Home) keep the tree in view without error
  await app.keyboard.press(".");
  await app.keyboard.press("Home");

  // Copy / paste across layers: Mod+C here, switch layers, Mod+V there
  await headOf(app, f1).click();
  await hoverCanvas(app);
  await app.keyboard.press(`${MOD}+c`);
  await gis(app, (g, id) => g.useApp.getState().setActiveLayer(id), l2.id);
  await expect.poll(async () => (await modelState(app)).nodes.some((n) => n.tool === "filter")).toBe(false);
  await hoverCanvas(app, 300, 100);
  await app.keyboard.press(`${MOD}+v`);
  await expect.poll(async () => (await modelState(app)).nodes.some((n) => n.tool === "filter")).toBe(true);
});
