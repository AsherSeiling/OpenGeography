// WP 8b: the Colours panel changes colours live, saves them per user, and Reset restores the defaults.
import { test, expect, gis } from "./fixtures";

const openPanel = (app: any) => gis(app, (g) => g.appearance.openAppearance());
const cssVar = (app: any, name: string) => app.evaluate((n: string) => getComputedStyle(document.documentElement).getPropertyValue(n).trim(), name);

test.beforeEach(async ({ app }) => {
  await app.evaluate(() => localStorage.removeItem("gis.appearance.v1"));
  await app.reload();
  await app.waitForFunction(() => !!(window as any).__gis?.appearance);
});

test("accent change applies live, survives a reload, and Reset undoes it", async ({ app }) => {
  await openPanel(app);
  const panel = app.getByRole("dialog", { name: "Colours" });
  await expect(panel).toBeVisible();
  await panel.getByRole("button", { name: "Coral" }).first().click();
  expect(await cssVar(app, "--accent")).toBe("#FF8A6B");
  const outline = await app.evaluate(() => getComputedStyle(document.querySelector(".apr")!).borderTopColor);
  expect(outline).toBe("rgb(255, 138, 107)");

  await app.reload();
  await app.waitForFunction(() => !!(window as any).__gis?.appearance);
  expect(await cssVar(app, "--accent")).toBe("#FF8A6B");

  await openPanel(app);
  await app.getByRole("button", { name: "Reset to default" }).click();
  expect(await cssVar(app, "--accent")).toBe("#4ADE80");
});

test("Dark sets the app's mode, Escape closes the panel, and it stays on screen at a narrow window", async ({ app }) => {
  await app.setViewportSize({ width: 420, height: 600 });
  await openPanel(app);
  const panel = app.getByRole("dialog", { name: "Colours" });
  await panel.getByRole("button", { name: "Dark" }).click();
  expect(await gis(app, (g) => g.useApp.getState().dark)).toBe(true);
  expect(await app.evaluate(() => document.documentElement.dataset.theme)).toBe("dark");
  const box = await panel.boundingBox();
  expect(box!.x).toBeGreaterThanOrEqual(0);
  expect(box!.x + box!.width).toBeLessThanOrEqual(420);
  expect(box!.y + box!.height).toBeLessThanOrEqual(600);
  await app.keyboard.press("Escape");
  await expect(panel).toBeHidden();
  await gis(app, (g) => g.useApp.setState({ dark: false }));
});

test("corrupt saved colours fall back to the defaults without an error", async ({ app }) => {
  await app.evaluate(() => localStorage.setItem("gis.appearance.v1", "{oops"));
  await app.reload();
  await app.waitForFunction(() => !!(window as any).__gis?.appearance);
  expect(await cssVar(app, "--accent")).toBe("#4ADE80");
});

test("screenshots of the panel in light and dark", async ({ app }, testInfo) => {
  test.skip(!process.env.APPEARANCE_SHOTS, "set APPEARANCE_SHOTS=<dir> to write them");
  await openPanel(app);
  await app.screenshot({ path: `${process.env.APPEARANCE_SHOTS}/colours-light.png` });
  await gis(app, (g) => { g.appearance.setAppearance({ palette: "Paper", accent: "#9B8CFF", hilite: "#C7BEFF" }); g.useApp.setState({ dark: true }); });
  await app.screenshot({ path: `${process.env.APPEARANCE_SHOTS}/colours-dark-paper-violet.png` });
  await gis(app, (g) => g.useApp.setState({ dark: false }));
  void testInfo;
});
