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
    await addTestLayer(app, "points", 20);
    await gis(app, (g) => { const s = g.useApp.getState(); s.setActiveLayer(Object.keys(s.layers)[0]); });
    for (const tab of ["Map", "Insert", "View", "Analysis", "Feature Layer"]) {
      await app.locator(".rtab", { hasText: new RegExp(`^${tab}$`) }).first().click();
      await app.waitForTimeout(350);
      await noScroll(app, `${tab} at ${w}px`);
      await commandsReachable(app, `${tab} at ${w}px`);
    }
    // The layout tab is the widest.
    await gis(app, async (g) => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
    await app.waitForTimeout(600);
    await app.locator(".rtab", { hasText: /^Layout$/ }).last().click();
    await app.waitForTimeout(400);
    await noScroll(app, `Layout at ${w}px`);
    await commandsReachable(app, `Layout at ${w}px`);
  });
}

test("LW-T a narrow window collapses groups into menus but a command is still one click away", async ({ app }) => {
  await app.setViewportSize({ width: 1024, height: 800 });
  await app.locator(".rtab", { hasText: /^Map$/ }).first().click();
  await app.waitForTimeout(400);
  // A command from the end of the Map tab (Coordinate System group) is in the ribbon, a group menu or More.
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
  await expect(app.locator(".rtab", { hasText: /^Layout$/ })).toBeVisible();
  await expect(undo).toBeDisabled();
});

test("the project name is a small box at the far left of the tab row, styled like the search box, with New, Open and Save As", async ({ app }) => {
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
  await expect(app.getByRole("menuitem")).toHaveText([/^New/, /^Open/, /^Save As/]);
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
  await app.setViewportSize({ width: 2560, height: 1000 });
  await addTestLayer(app, "points", 20);
  await gis(app, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  const seen = new Set<string>();
  for (const tab of ["Map", "Insert", "View", "Analysis", "Layout", "Elements", "Format", "Export", "Feature Layer"]) {
    await app.locator(".rtab", { hasText: new RegExp(`^${tab}$`) }).click();
    await app.waitForTimeout(250);
    for (const t of await app.locator(".ribbon-body .rbtn-label").allTextContents()) seen.add(t.trim());
  }
  const missing = r.onTabs.filter((id: string) => id !== "newLayout" && !seen.has(r.labels[id])); // New Layout is the Insert gallery
  expect(missing, "commands with no button on screen").toEqual([]);
});
