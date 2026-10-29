// LW-K: Shift+A opens the Add menu with the pointer over the Model canvas, even when a text box elsewhere still has focus.
import { test, expect, addTestLayer, hoverCanvas } from "./fixtures";

test("Shift+A works after a text box outside the editor was used, once the pointer is over the canvas", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  const box = app.getByPlaceholder(/Command search/);
  await box.click();
  await hoverCanvas(app);
  await app.waitForTimeout(1400); // typing wins for a moment, then the pointer decides
  await app.keyboard.press("Shift+A");
  await expect(app.locator(".addmenu")).toBeVisible();
  await app.keyboard.press("Escape");
  await expect(app.locator(".addmenu")).toHaveCount(0);
});

test("keys still go to a text box while it is being typed in, with the pointer over the canvas", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  const box = app.getByPlaceholder(/Command search/);
  await box.click();
  await hoverCanvas(app);
  await app.keyboard.type("ab"); // typing starts: a shortcut now would corrupt it
  await app.keyboard.press("Shift+A");
  await expect(app.locator(".addmenu")).toHaveCount(0);
  await expect(box).toHaveValue(/^abA?$/i);
});

test("Shift+A on a plain canvas (nothing focused) still opens the Add menu", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  await hoverCanvas(app);
  await app.keyboard.press("Shift+A");
  await expect(app.locator(".addmenu")).toBeVisible();
});
