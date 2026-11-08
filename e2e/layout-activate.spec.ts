import { test, expect, gis, ribbon, addTestLayer } from "./fixtures";

const frameOf = (page: any) => gis(page, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); return f && { id: f.id, center: f.center, scale: f.scale, crs: f.crs, locked: f.locked, w: f.w, h: f.h, x: f.x, y: f.y }; }) as Promise<any>;
const past = (page: any) => gis(page, (g) => g.useLayouts.temporal.getState().pastStates.length) as Promise<number>;

async function setup(app: any) {
  const layer = await addTestLayer(app, "polygons", 30, [-100, 30, -95, 35]);
  await gis(app, (g, id) => g.actions.zoomToLayer(id), layer.id);
  await app.waitForTimeout(600);
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await ribbon(app, "Layout", /^Map Frame$/);
  await expect.poll(async () => (await frameOf(app))?.id).toBeTruthy();
  await expect(app.locator(".layout-img")).toBeVisible({ timeout: 20_000 });
  // Put the frame at a known scale so the numbers are easy to check.
  const f = await frameOf(app);
  await gis(app, (g, id) => g.useLayouts.getState().update(id, { scale: 1_000_000, crs: "EPSG:26915", center: [500000, 4000000] }), f.id); // a metre grid, so 1 mm is 1 km of ground
  return f.id as string;
}

test("UR-18 double-click activates a map frame: drag pans it, the wheel zooms at the cursor, Esc ends it; all one undo step", async ({ app }) => {
  const id = await setup(app);
  const el = app.locator(`.layout-el[data-el="${id}"]`);
  const before = await frameOf(app);
  const undoBefore = await past(app);
  await el.dblclick();
  await expect(app.getByTestId("active-frame")).toBeVisible();
  await expect(app.locator(".statusbar")).toContainText("Map active");
  await expect(app.locator(".layout-stat")).not.toContainText("Rendering"); // the page settles (a warning bar can resize it) before it is measured
  await app.waitForTimeout(400);
  const box = (await app.getByTestId("active-frame").boundingBox())!;
  const z = box.width / before.w; // screen px per mm

  // Drag 20 mm right and 10 mm down: the centre moves left and up by 20 and 10 mm of ground (1 mm = 1 km at 1:1,000,000 in UTM/Albers metres).
  const cx = box.x + box.width / 2, cy = box.y + box.height / 2;
  await app.mouse.move(cx, cy);
  await app.mouse.down();
  await app.mouse.move(cx + 10 * z, cy + 5 * z, { steps: 5 });
  await app.mouse.move(cx + 20 * z, cy + 10 * z, { steps: 5 });
  await app.mouse.up();
  await expect.poll(async () => (await frameOf(app)).center[0]).not.toBe(before.center[0]);
  const panned = await frameOf(app);
  expect(panned.scale).toBe(before.scale);
  expect(Math.abs((before.center[0] - panned.center[0]) - 20_000)).toBeLessThan(200); // 20 mm at 1:1,000,000
  expect(Math.abs((panned.center[1] - before.center[1]) - 10_000)).toBeLessThan(200);

  // Wheel up over a point away from the centre: closer, and that point's ground stays put.
  const px = box.x + box.width * 0.75, py = box.y + box.height * 0.25;
  await app.mouse.move(px, py);
  for (let i = 0; i < 3; i++) await app.mouse.wheel(0, -100);
  await expect.poll(async () => (await frameOf(app)).scale, { timeout: 5000 }).toBeLessThan(panned.scale * 0.9);
  const zoomed = await frameOf(app);
  const factor = panned.scale / zoomed.scale;
  // The ground under the cursor before and after: cursor offset from the centre in mm, times ground per mm.
  const offX = (px - cx) / z, offY = (py - cy) / z;
  const groundBefore = [panned.center[0] + offX * panned.scale / 1000, panned.center[1] - offY * panned.scale / 1000];
  const groundAfter = [zoomed.center[0] + offX * zoomed.scale / 1000, zoomed.center[1] - offY * zoomed.scale / 1000];
  expect(Math.abs(groundAfter[0] - groundBefore[0])).toBeLessThan(300 / factor + 50);
  expect(Math.abs(groundAfter[1] - groundBefore[1])).toBeLessThan(300 / factor + 50);

  // Esc ends it; nothing changes after.
  await app.keyboard.press("Escape");
  await expect(app.getByTestId("active-frame")).toHaveCount(0);
  const done = await frameOf(app);
  await app.mouse.move(cx, cy);
  await app.mouse.down(); await app.mouse.move(cx + 30, cy + 30, { steps: 3 }); await app.mouse.up();
  const after = await frameOf(app); // an ordinary drag now moves the frame on the page, not its map
  expect([after.center, after.scale]).toEqual([done.center, done.scale]);
  expect(after.x).not.toBe(done.x);

  // The whole activation was one undo step (the later page drag is another).
  expect(await past(app)).toBe(undoBefore + 2);
  await gis(app, (g) => g.useLayouts.getState().undo()); // the page drag
  await gis(app, (g) => g.useLayouts.getState().undo());
  const undone = await frameOf(app);
  expect(undone.center).toEqual(before.center);
  expect(undone.scale).toBe(before.scale);
});

test("UR-18 a click elsewhere ends the activation; a locked frame refuses with a sentence; a rotated frame pans along its own axes", async ({ app }) => {
  const id = await setup(app);
  const el = app.locator(`.layout-el[data-el="${id}"]`);
  await el.dblclick();
  await expect(app.getByTestId("active-frame")).toBeVisible();
  await app.locator(".layout-canvas").click({ position: { x: 6, y: 6 } });
  await expect(app.getByTestId("active-frame")).toHaveCount(0);

  await gis(app, (g, i) => g.useLayouts.getState().update(i, { locked: true }), id);
  await el.dblclick();
  await expect(app.getByTestId("active-frame")).toHaveCount(0);
  await expect(app.locator(".statusbar")).toContainText(/locked, so its map can't be moved/);
  await gis(app, (g, i) => g.useLayouts.getState().update(i, { locked: false, rotation: 90 }), id);

  const before = await frameOf(app);
  await el.dblclick();
  await expect(app.locator(".layout-stat")).not.toContainText("Rendering");
  await app.waitForTimeout(400);
  const box = (await app.getByTestId("active-frame").boundingBox())!; // the rotated box
  const cx = box.x + box.width / 2, cy = box.y + box.height / 2;
  const z = box.height / before.w; // a 90° frame: its width lies along the screen's vertical
  await app.mouse.move(cx, cy);
  await app.mouse.down(); await app.mouse.move(cx + 10 * z, cy, { steps: 6 }); await app.mouse.up();
  await expect.poll(async () => (await frameOf(app)).center[1]).not.toBe(before.center[1]);
  const after = await frameOf(app);
  // Page-right on a frame turned 90° clockwise is its "up": the centre moves down by 10 mm of ground, and not sideways.
  expect(Math.abs(after.center[0] - before.center[0])).toBeLessThan(200);
  expect(Math.abs((before.center[1] - after.center[1]) - 10_000)).toBeLessThan(200);
});
