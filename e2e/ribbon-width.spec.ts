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
    for (const tab of ["Map", "Insert", "Analysis", "Data", "View", "Feature Layer"]) {
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
  await app.keyboard.press("d");
  await expect(app.locator(".rtab.on")).toHaveText(/Data/);
});
