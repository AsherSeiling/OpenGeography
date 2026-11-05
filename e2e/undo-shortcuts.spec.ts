// Undo and redo from anywhere (user, 2026-11-04). ControlOrMeta is Cmd on the Mac this runs on and Ctrl on Windows and Linux; nodes/nodeEditor.test.ts covers the mapping.
import { test, expect, gis, addTestLayer } from "./fixtures";

const hasClip = (app: any) => gis(app, (g) => g.useModel.getState().nodes.some((n: any) => n.data.tool === "clip")) as Promise<boolean>;

async function addClip(app: any) {
  await addTestLayer(app, "points", 5);
  const box = app.getByPlaceholder("Search commands and tools");
  await box.fill("cut");
  await app.getByRole("option", { name: /^Clip\s*Tool/ }).click();
  await expect.poll(() => hasClip(app)).toBe(true);
  await app.locator(".tree-layer").first().click(); // focus and pointer leave the node editor
  await app.mouse.move(120, 300);
}

test("US-1 undo and redo work with the focus and pointer outside the node editor", async ({ app }) => {
  await addClip(app);
  await app.keyboard.press("ControlOrMeta+z");
  await expect.poll(() => hasClip(app)).toBe(false);
  await app.keyboard.press("ControlOrMeta+Shift+z");
  await expect.poll(() => hasClip(app)).toBe(true);
  await app.keyboard.press("ControlOrMeta+z");
  await expect.poll(() => hasClip(app)).toBe(false);
  await app.keyboard.press("ControlOrMeta+y");
  await expect.poll(() => hasClip(app)).toBe(true);
});

test("US-1 a text box keeps its own undo and an open dialog takes no shortcut", async ({ app }) => {
  await addClip(app);
  const box = app.getByPlaceholder("Search commands and tools");
  await box.fill("abc");
  await app.keyboard.press("ControlOrMeta+z");
  await app.waitForTimeout(200);
  expect(await hasClip(app)).toBe(true);
  await box.fill("");
  await app.locator(".tree-layer").first().click();
  await gis(app, async () => { const m = await import("/src/actions.ts"); m.useDialog.getState().show({ kind: "responsibleUse" }); });
  await expect(app.getByRole("dialog")).toBeVisible();
  await app.keyboard.press("ControlOrMeta+z");
  await app.waitForTimeout(200);
  expect(await hasClip(app)).toBe(true);
});

test("US-1 in a layout the shortcuts act on the layout with the focus outside the page", async ({ app }) => {
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await gis(app, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  await expect(app.locator(".layout-page")).toBeVisible();
  await app.getByTestId("start-gallery").locator('[data-start="thematic"]').click();
  const count = () => gis(app, (g) => g.useLayouts.getState().doc.elements.length) as Promise<number>;
  await expect.poll(count).toBeGreaterThan(0);
  const n = await count();
  await gis(app, () => (document.activeElement as HTMLElement | null)?.blur());
  await app.keyboard.press("ControlOrMeta+z");
  await expect.poll(count).toBe(0);
  await app.keyboard.press("ControlOrMeta+Shift+z");
  await expect.poll(count).toBe(n);
});
