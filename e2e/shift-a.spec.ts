// LW-K: Shift+A opens the Add menu with the pointer over the Model canvas, even when a text box elsewhere still has focus.
import { test, expect, addTestLayer, hoverCanvas, expectAddMenuOnScreen } from "./fixtures";

test("Shift+A works after a text box outside the editor was used, once the pointer is over the canvas", async ({ app }) => {
  await addTestLayer(app, "points", 20);
  const box = app.getByPlaceholder(/Command search/);
  await box.click();
  await hoverCanvas(app);
  await app.waitForTimeout(1400); // typing wins for a moment, then the pointer decides
  await app.keyboard.press("Shift+A");
  await expectAddMenuOnScreen(app);
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
  await expect(app.locator(".addmenu")).toHaveCount(0);
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
