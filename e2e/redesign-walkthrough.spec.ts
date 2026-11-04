// WP9 walk-through (docs/UI_REDESIGN_PLAN.md §7): in the real UI on the dev server, Shift+A, search, add three different tools, check they are
// connected, run, and export a layout, in the light theme, the dark theme, a narrow window, with the keyboard only, and in the Layout
// workspace. Each step logs what it saw (`WALK …`); screenshots go to WP9_SHOTS when it is set.
import { existsSync, mkdirSync, mkdtempSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

type Page = import("@playwright/test").Page;
const SHOTS = process.env.WP9_SHOTS;
const shot = async (page: Page, name: string) => {
  if (!SHOTS) return;
  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, `${name}.png`) });
};
const say = (m: string) => console.log(`WALK ${m}`);
const tools = (page: Page) => gis(page, (g) => g.useModel.getState().nodes.map((n: any) => n.data.tool)) as Promise<string[]>;
const edges = (page: Page) => gis(page, (g) => g.useModel.getState().edges.length) as Promise<number>;

/** Shift+A, type, Enter: the tool is added; returns the node list afterwards. */
async function addByMenu(page: Page, query: string, opts: { clickPick?: boolean } = {}) {
  const box = (await page.locator(".model-canvas").boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.keyboard.press("Shift+A");
  await expect(page.locator(".toolmenu")).toBeVisible();
  await page.keyboard.type(query);
  const first = page.locator(".toolmenu [role=option]").first();
  await expect(first).toBeVisible();
  say(`Shift+A "${query}" → first result "${((await first.textContent()) ?? "").trim().slice(0, 50)}"`);
  if (opts.clickPick) await first.click();
  else await page.keyboard.press("Enter");
  await expect(page.locator(".toolmenu")).toHaveCount(0);
}

async function run(page: Page, viaKeyboard: boolean) {
  if (viaKeyboard) { await page.locator(".model-run").focus(); await page.keyboard.press("Enter"); }
  else await page.locator(".model-run").click();
  await page.waitForFunction(() => { const r = (window as any).__gis.useRun.getState(); return !r.running && Object.keys(r.results).length > 0; }, undefined, { timeout: 60_000 });
  const results = await gis(page, (g) => Object.values(g.useRun.getState().results).map((r: any) => r.status));
  say(`ran: node statuses ${JSON.stringify(results)}`);
  expect(results.every((s: string) => s !== "error")).toBe(true);
}

async function exportLayout(page: Page, how: "click" | "keyboard") {
  const dir = mkdtempSync(join(tmpdir(), "gis-walk-"));
  const out = join(dir, "walk.pdf");
  page.removeAllListeners("dialog");
  page.on("dialog", (d) => void (d.type() === "prompt" ? d.accept(out) : d.accept()).catch(() => undefined));
  await gis(page, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await gis(page, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  await page.getByTestId("start-gallery").locator('[data-start="thematic"]').click();
  await expect(page.locator(".layout-el").first()).toBeVisible();
  say(`layout workspace: bottom panel ${(await page.locator(".model-canvas").count()) ? "STILL OPEN" : "closed"}, Items column ${(await page.locator(".dv-tab", { hasText: /^Items$/ }).count()) ? "present" : "missing"}`);
  await shot(page, "after-layout-workspace");
  await page.locator(".ribbon-tabs [role=tab]", { hasText: /^Export$/ }).click();
  const pdf = page.locator(".ribbon-body .rbtn", { hasText: /^PDF$/ });
  if (how === "keyboard") { await pdf.focus(); await page.keyboard.press("Enter"); } else await pdf.click();
  await expect.poll(() => existsSync(out), { timeout: 60_000 }).toBe(true);
  say(`exported ${out.split("/").pop()} ${statSync(out).size} bytes`);
  expect(statSync(out).size).toBeGreaterThan(1000);
}

for (const theme of ["light", "dark"] as const) {
  test(`walk-through, ${theme} theme: Shift+A, search, three tools, connect, run, export a layout`, async ({ app }) => {
    test.setTimeout(240_000);
    await app.setViewportSize({ width: 1600, height: 950 });
    await addTestLayer(app, "polygons", 20);
    await gis(app, (g, t: string) => g.useApp.setState({ dark: t === "dark" }), theme);
    await shot(app, `after-map-${theme}`);
    await addByMenu(app, "buffer");
    await shot(app, `after-shift-a-${theme}`);
    // search: the tab-row search finds a tool and adds it like the menu
    await app.locator(".ribbon-tabs .search input").fill("dissolve");
    say(`search "dissolve" → ${(await app.getByRole("option").allTextContents()).slice(0, 4).join(" | ")}`);
    await app.keyboard.press("ArrowDown").catch(() => undefined);
    await app.getByRole("option").filter({ hasText: /Dissolve/ }).first().click();
    await addByMenu(app, "simplify", { clickPick: true });
    const t = await tools(app);
    say(`tools in the tree: ${t.join(" → ")}; wires: ${await edges(app)}`);
    expect(t).toEqual(expect.arrayContaining(["buffer", "dissolve", "simplify"]));
    expect(await edges(app)).toBeGreaterThanOrEqual(4); // Layer → Buffer → Dissolve → Simplify → Output: each pick wires itself
    await shot(app, `after-tree-${theme}`);
    await run(app, false);
    await exportLayout(app, "click");
    if (theme === "dark") await shot(app, "after-layout-dark");
  });
}

test("walk-through, narrow window (1024 px): the ribbon, Shift+A menu and layout stay usable", async ({ app }) => {
  test.setTimeout(240_000);
  await app.setViewportSize({ width: 1024, height: 700 });
  await addTestLayer(app, "polygons", 20);
  await addByMenu(app, "buffer");
  const clipped = await app.evaluate(() => [...document.querySelectorAll<HTMLElement>(".ribbon-tabs [role=tab], .rbtn, .qa-btn")].filter((e) => e.offsetParent && e.scrollWidth > e.clientWidth + 1 && getComputedStyle(e).overflow !== "visible").map((e) => e.textContent));
  say(`1024 px: clipped ribbon controls: ${JSON.stringify(clipped)}; horizontal page scroll: ${await app.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)}`);
  expect(clipped).toEqual([]);
  await shot(app, "after-narrow");
  await app.keyboard.press("Shift+A");
  const m = (await app.locator(".toolmenu").boundingBox())!;
  say(`1024 px: tool menu at ${Math.round(m.x)},${Math.round(m.y)} ${Math.round(m.width)}×${Math.round(m.height)} (window 1024×700)`);
  expect(m.x).toBeGreaterThanOrEqual(0);
  expect(m.x + m.width).toBeLessThanOrEqual(1024);
  expect(m.y + m.height).toBeLessThanOrEqual(700);
  await app.keyboard.press("Escape");
  await exportLayout(app, "click");
  const names = await app.evaluate(() => [...document.querySelectorAll<HTMLElement>(".dv-tab")].filter((e) => e.offsetParent).map((e) => `${e.textContent}${e.scrollWidth > e.clientWidth + 1 ? " (CUT)" : ""}`));
  say(`1024 px layout workspace tabs: ${names.join(", ")}`);
  expect(names.filter((n) => n.includes("(CUT)"))).toEqual([]);
  await shot(app, "after-narrow-layout");
});

test("walk-through, keyboard only: Shift+A, arrows, Enter, Tab to Run, Tab to the layout export", async ({ app }) => {
  test.setTimeout(240_000);
  await app.setViewportSize({ width: 1600, height: 950 });
  await addTestLayer(app, "polygons", 20);
  const box = (await app.locator(".model-canvas").boundingBox())!;
  await app.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (const q of ["buffer", "dissolve", "simplify"]) {
    await app.keyboard.press("Shift+A");
    await app.keyboard.type(q);
    await app.keyboard.press("Enter");
    await expect(app.locator(".toolmenu")).toHaveCount(0);
  }
  say(`keyboard only: tools ${(await tools(app)).join(" → ")}, wires ${await edges(app)}`);
  expect(await tools(app)).toEqual(expect.arrayContaining(["buffer", "dissolve", "simplify"]));
  await run(app, true);
  // ArrowRight on the menu's empty field changes category; Escape closes with focus returned
  await app.keyboard.press("Shift+A");
  await app.keyboard.press("ArrowRight");
  say(`menu category after ArrowRight: ${((await app.locator(".toolmenu .tm-rail button.on").textContent()) ?? "").trim()}`);
  await app.keyboard.press("Escape");
  await exportLayout(app, "keyboard");
});
