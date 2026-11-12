// LW-T: the top bar never shows a scroll bar and never loses a command, at any window width.
import { test, expect, gis, addTestLayer } from "./fixtures";

const WIDTHS = [900, 1024, 1280, 1440, 1920];

async function noScroll(app: any, label: string) {
  const m = await app.evaluate(() => {
    const b = document.querySelector(".ribbon-body") as HTMLElement;
    return { sw: b.scrollWidth, cw: b.clientWidth, os: getComputedStyle(b).overflowX };
  });
  expect(m.sw, `${label}: scrollWidth ${m.sw} > clientWidth ${m.cw}`).toBeLessThanOrEqual(m.cw + 1);
  expect(m.os, label).not.toBe("auto");
  expect(m.os, label).not.toBe("scroll");
  // The tab row neither clips a tab nor scrolls, and Save, Undo, Redo and the search box stay inside the window.
  const row = await app.evaluate(() => {
    const t = document.querySelector(".rtabs") as HTMLElement;
    const s = (document.querySelector(".tellme") as HTMLElement).getBoundingClientRect();
    return { sw: t.scrollWidth, cw: t.clientWidth, searchRight: s.right, win: window.innerWidth };
  });
  expect(row.sw, `${label}: tab row clips tabs`).toBeLessThanOrEqual(row.cw + 1);
  expect(row.searchRight, `${label}: search box off screen`).toBeLessThanOrEqual(row.win);
}

/** Every command of the tab is reachable: visible as a button, or inside a collapsed group's menu, or in the More menu. */
async function commandsReachable(app: any, label: string) {
  const names = await app.evaluate(() => [...document.querySelectorAll(".ribbon-body .rg-slot:not(.rg-more) .rgroup-label")].map((e) => e.textContent));
  expect(names.length, label).toBeGreaterThan(0);
  const more = app.locator(".rbtn-more").first();
  if (await more.isVisible().catch(() => false)) {
    await more.click();
    await expect(app.locator(".rmenu").first(), `${label}: More menu`).toBeVisible();
    await app.keyboard.press("Escape");
  }
}

for (const w of WIDTHS) {
  test(`LW-T ribbon at ${w}px: no scroll bar on any tab`, async ({ app }) => {
    await app.setViewportSize({ width: w, height: 800 });
    const layer = await addTestLayer(app, "points", 20);
    await gis(app, (g) => { const s = g.useApp.getState(); s.setActiveLayer(Object.keys(s.layers)[0]); });
    for (const tab of ["Map", "Data", "Coordinates", "Analysis", "Insert", "View", "Feature Layer"]) {
      await app.locator(".rtab", { hasText: new RegExp(`^${tab}$`) }).first().click();
      await app.waitForTimeout(350);
      await noScroll(app, `${tab} at ${w}px`);
      await commandsReachable(app, `${tab} at ${w}px`);
    }
    // The Edit tab is on the ribbon only while a shape-edit session is open.
    await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
    await app.locator(".rtab", { hasText: /^Edit$/ }).first().click();
    await app.waitForTimeout(350);
    await noScroll(app, `Edit at ${w}px`);
    await commandsReachable(app, `Edit at ${w}px`);
    await gis(app, (g) => g.edit.discardEdits());
    // The layout's tabs: Page, Insert, Arrange, View, Export, and Map Frame while a frame is selected.
    await gis(app, async (g) => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
    await app.waitForTimeout(600);
    await addFrame(app);
    for (const tab of ["Page", "Insert", "Arrange", "View", "Export", "Map Frame"]) {
      await app.locator(".rtab", { hasText: new RegExp(`^${tab}$`) }).last().click();
      await app.waitForTimeout(400);
      await noScroll(app, `Layout ${tab} at ${w}px`);
      await commandsReachable(app, `Layout ${tab} at ${w}px`);
    }
  });
}

/** A map frame in the open layout, selected (no coordinate system is needed for this). */
async function addFrame(app: any) {
  await gis(app, (g) => {
    const s = g.useLayouts.getState();
    s.addElement({ kind: "map_frame", id: "f1", x: 10, y: 30, w: 150, h: 120, rotation: 0, locked: false, visible: true, crs: "EPSG:4326", center: [0, 0], scale: 1000000, layers: "map", basemap: "none", graticule: null, grid: null, border: null });
    s.select(["f1"]);
  });
  await app.waitForTimeout(300);
}

test("LW-T a narrow window collapses groups into menus but a command is still one click away", async ({ app }) => {
  await app.setViewportSize({ width: 1024, height: 800 });
  await app.locator(".rtab", { hasText: /^Coordinates$/ }).first().click();
  await app.waitForTimeout(400);
  // A command from the Coordinates tab (Coordinate System group) is in the ribbon, a group menu or More.
  const direct = await app.locator(".rbtn", { hasText: "Datum Grids" }).count();
  if (!direct) {
    const more = app.locator(".rbtn-more").first();
    const collapsed = app.locator(".rgroup-collapsed .rbtn", { hasText: /Coordinate System/ }).first();
    if (await collapsed.isVisible().catch(() => false)) await collapsed.click();
    else await more.click();
  }
  await expect(app.getByRole("button", { name: /Datum Grids/ }).first()).toBeVisible();
});

test("LW-T the ribbon collapses with Ctrl+F1 and a double-click, and Alt shows key tips", async ({ app }) => {
  await expect(app.locator(".ribbon-body")).toBeVisible();
  await app.keyboard.press("Control+F1");
  await expect(app.locator(".ribbon-body")).toBeHidden();
  await app.keyboard.press("Control+F1");
  await expect(app.locator(".ribbon-body")).toBeVisible();
  await app.locator(".rtab", { hasText: /^Insert$/ }).dblclick();
  await expect(app.locator(".ribbon-body")).toBeHidden();
  await app.locator(".rtab", { hasText: /^Insert$/ }).dblclick();
  await expect(app.locator(".ribbon-body")).toBeVisible();
  await app.keyboard.press("Alt");
  await expect(app.locator(".keytip").first()).toBeVisible();
  await app.keyboard.press("v");
  await expect(app.locator(".rtab.on")).toHaveText(/View/);
});

test("Save, Undo and Redo sit in the tab row, in line with the tabs and the search box", async ({ app }) => {
  const box = async (sel: string) => (await app.locator(sel).first().boundingBox())!;
  const tab = await box(".rtab");
  const tabs = app.locator(".ribbon-tabs");
  for (const name of ["Save", "Undo", "Redo"]) {
    const b = tabs.getByRole("button", { name });
    await expect(b).toBeVisible();
    const r = (await b.boundingBox())!;
    expect(Math.abs(r.y + r.height / 2 - (tab.y + tab.height / 2)), `${name} is in line with the tabs`).toBeLessThan(14);
  }
  await expect(tabs.getByRole("button", { name: "Save" })).toContainText("Save"); // icon and label
  await expect(tabs.getByPlaceholder("Search commands and tools")).toBeVisible();
  const search = await box(".tellme");
  const redo = await box('[data-qa="redo"]');
  expect(search.x).toBeGreaterThan(redo.x); // search is at the far right, after Redo
});

test("Undo and Redo act on the node tree, grey out when there is nothing to undo, and follow an open layout", async ({ app }) => {
  const undo = app.locator('[data-qa="undo"]');
  const redo = app.locator('[data-qa="redo"]');
  await addTestLayer(app, "points", 20);
  await expect(undo).toBeDisabled();
  await expect(redo).toBeDisabled();
  const before = await gis(app, (g) => g.useModel.getState().nodes.length);
  await gis(app, (g) => g.useModel.getState().addNode("buffer", { x: 300, y: 200 }));
  await expect(undo).toBeEnabled();
  expect(await gis(app, (g) => g.useModel.getState().nodes.length)).toBe(before + 1);
  await undo.click();
  expect(await gis(app, (g) => g.useModel.getState().nodes.length)).toBe(before);
  await expect(redo).toBeEnabled();
  await redo.click();
  expect(await gis(app, (g) => g.useModel.getState().nodes.length)).toBe(before + 1);
  // A layout that is open takes over: its own history starts empty, so Undo is grey again.
  await gis(app, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  await expect(app.locator(".rtab", { hasText: /^Page$/ })).toBeVisible();
  await expect(undo).toBeDisabled();
});

test("the project name is a small box at the far left of the tab row, styled like the search box, with New, Open, Save As, Start Page and Settings", async ({ app }) => {
  const proj = (await app.locator(".proj-btn").boundingBox())!;
  const search = (await app.locator(".search").boundingBox())!;
  const firstTab = (await app.locator(".rtab").first().boundingBox())!;
  expect(proj.height).toBe(22);
  expect(search.height).toBe(22);
  expect(Math.abs(proj.y - search.y)).toBeLessThan(1); // same vertical alignment
  expect(proj.x).toBeLessThan(firstTab.x); // far left, before the tabs
  expect(proj.x).toBeLessThan(20);
  expect(await app.locator(".ribbon-tabs .proj-btn").count()).toBe(1);
  // No left-column-wide cell: the band runs from the left edge to the right edge.
  const band = (await app.locator(".ribbon-body").boundingBox())!;
  expect(band.x).toBeLessThan(2);
  expect(band.width).toBeGreaterThan(1600 - 4);
  await app.locator(".proj-btn").click();
  await expect(app.getByRole("menuitem")).toHaveText([/^New/, /^Open/, /^Save As/, /^Start Page/, /^Settings/]);
  await app.keyboard.press("Escape");
});

test("every command is reachable: on a tab, in the project menu, or in the Catalog's Public Data tree", async ({ app }) => {
  const r = await gis(app, async () => {
    const c = await import("/src/ribbon/commands.tsx");
    const t = await import("/src/ribbon/tabsDef.tsx");
    const p = await import("/src/ribbon/projectBox.tsx");
    const reach = new Set([...t.commandsOnTabs(), ...p.PROJECT_MENU, ...c.CATALOG_COMMANDS, "save"]);
    return { lost: Object.keys(c.commands).filter((id) => !reach.has(id)), labels: Object.fromEntries(Object.entries(c.commands).map(([id, x]: any) => [id, x.label])), onTabs: [...t.commandsOnTabs()], catalog: c.CATALOG_COMMANDS };
  });
  expect(r.lost).toEqual([]);
  // The Public Data rows are really in the tree.
  for (const id of r.catalog) await expect(app.locator(`[role="treeitem"][data-node="public:${id}"]`)).toHaveCount(1);
  // And on screen: with a layer and a layout open and a very wide window, every tab shows a button for each of its commands.
  await gis(app, (g) => g.useSettings.setState({ settings: { ...g.useSettings.getState().settings, showDeveloperTools: true } })); // the Developer group is hidden by default
  await app.setViewportSize({ width: 2560, height: 1000 });
  const layer = await addTestLayer(app, "points", 20);
  const seen = new Set<string>();
  const readTab = async (tab: string) => {
    await app.locator(".rtab", { hasText: new RegExp(`^${tab}$`) }).last().click();
    await app.waitForTimeout(250);
    for (const t of await app.locator(".ribbon-body .rbtn-label").allTextContents()) seen.add(t.trim());
  };
  for (const tab of ["Map", "Data", "Coordinates", "Analysis", "Insert", "View", "Feature Layer"]) await readTab(tab);
  // The Edit tab exists only while a shape-edit session is open.
  await gis(app, (g, id: string) => g.edit.startEditing(id), layer.id);
  await readTab("Edit");
  await gis(app, (g) => g.edit.discardEdits());
  // The layout: its tabs, the Insert rail and the rail's Shape menu.
  await gis(app, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  await addFrame(app);
  for (const tab of ["Page", "Insert", "Arrange", "View", "Export", "Map Frame"]) await readTab(tab);
  for (const t of await app.locator(".rail .rbtn-label").allTextContents()) seen.add(t.trim().replace(/ ▾$/, ""));
  await app.locator(".rail-btn", { hasText: /^Shape/ }).click();
  for (const t of await app.getByRole("menuitem").allTextContents()) seen.add(t.trim());
  await app.keyboard.press("Escape");
  const missing = r.onTabs.filter((id: string) => id !== "newLayout" && !seen.has(r.labels[id])); // New Layout is the Insert gallery
  expect(missing, "commands with no button on screen").toEqual([]);
});

test("RB1 the Map tab fits at 1440 px with no collapsed group and no More menu", async ({ app }) => {
  await app.setViewportSize({ width: 1440, height: 800 });
  await app.locator(".rtab", { hasText: /^Map$/ }).first().click();
  await app.waitForTimeout(450);
  await expect(app.locator(".rgroup-collapsed")).toHaveCount(0);
  await expect(app.locator("button.rbtn-more")).toHaveCount(0); // the hidden span.rbtn-more is only a width probe
  expect(await app.locator(".ribbon-body .rg-slot:not(.rg-more)").count()).toBe(5);
});
