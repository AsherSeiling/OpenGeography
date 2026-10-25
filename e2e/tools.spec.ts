import proj4 from "proj4";
import { test, expect, gis, ribbon, pixelOf, addKnownPoints, click } from "./fixtures";

const selection = (page: any, id: string) => gis(page, (g, id) => g.useApp.getState().selection[id] ?? [], id) as Promise<number[]>;

test("M-10 map tools: select (Shift/Ctrl), rectangle, identify, measure, measure area", async ({ app }) => {
  const info = await addKnownPoints(app);
  await app.waitForTimeout(1000);
  const P = { a: [-97, 35], c: [-96.9, 35.1], d: [-96.8, 35.2] };
  const at = async (k: "a" | "c" | "d") => pixelOf(app, P[k][0], P[k][1]);
  let [cx, cy] = await at("c"), [dx, dy] = await at("d"), [ax, ay] = await at("a");

  await ribbon(app, "Map", /^Select$/);
  await click(app, cx, cy);
  await expect.poll(() => selection(app, info.id)).toHaveLength(1);
  await app.keyboard.down("Shift");
  await click(app, dx, dy);
  await app.keyboard.up("Shift");
  await expect.poll(() => selection(app, info.id)).toHaveLength(2);
  await app.keyboard.down("Control");
  await click(app, dx, dy);
  await app.keyboard.up("Control");
  await expect.poll(() => selection(app, info.id)).toHaveLength(1);
  await ribbon(app, "Map", /^Clear$/);
  expect(await selection(app, info.id)).toHaveLength(0);

  await ribbon(app, "Map", "Select by Rectangle");
  [dx, dy] = await at("d"); [ax, ay] = await at("a");
  await app.mouse.move(Math.min(ax, dx) - 30, Math.min(ay, dy) - 30);
  await app.mouse.down();
  await app.mouse.move(Math.max(ax, dx) + 30, Math.max(ay, dy) + 30, { steps: 6 });
  await app.mouse.up();
  await expect.poll(() => selection(app, info.id)).toHaveLength(4);
  await ribbon(app, "Map", /^Clear$/);

  await ribbon(app, "Map", /^Identify$/);
  [cx, cy] = await at("c"); [ax, ay] = await at("a");
  await click(app, cx, cy);
  await expect(app.locator(".identify")).toContainText("name");
  await expect(app.locator(".identify")).toContainText("c");
  await app.locator(".identify .x").click();
  await click(app, ax, ay);
  await expect(app.locator(".identify-more")).toContainText("+1 more feature");
  await app.locator(".identify .x").click();

  // Measure: three clicks, double-click finishes; readout has distance and azimuth; Esc clears.
  await ribbon(app, "Map", /^Measure$/);
  [cx, cy] = await at("c"); [dx, dy] = await at("d"); [ax, ay] = await at("a");
  await click(app, ax, ay);
  await click(app, cx, cy);
  await app.mouse.dblclick(dx, dy);
  await expect(app.locator(".mp-total")).toContainText(/km|mi|m/);
  await expect(app.locator(".mp-segs")).toContainText("°");
  await app.keyboard.press("Escape");
  await expect(app.locator(".mp-total")).toHaveCount(0);

  await ribbon(app, "Map", "Measure Area");
  await click(app, ax, ay);
  await click(app, cx, cy);
  await click(app, dx, dy - 40);
  await expect(app.locator(".mp-total")).toContainText(/ac|km²|mi²|m²|ha/);
  await app.keyboard.press("Escape");
});

test("M-10 Resection and Intersection panels match the geodesy test examples", async ({ app }) => {
  const target: [number, number] = [-97.45, 35.25];
  const pts = { a: [-97.5, 35.2], b: [-97.38, 35.21], c: [-97.46, 35.31] } as Record<string, [number, number]>;
  const utm = "+proj=utm +zone=14 +datum=WGS84";
  const gridAz = (from: number[], to: number[]) => {
    const [x0, y0] = proj4("EPSG:4326", utm, from), [x1, y1] = proj4("EPSG:4326", utm, to);
    return (((Math.atan2(x1 - x0, y1 - y0) * 180) / Math.PI) + 360) % 360;
  };
  const near = (s: string) => { const m = s.match(/(-?\d+\.\d+)°, (-?\d+\.\d+)°/)!; return [Number(m[2]), Number(m[1])]; };
  await addKnownPoints(app);

  await ribbon(app, "Map", /^Resection$/);
  const panel = app.locator(".nav-panel");
  await panel.locator("select").first().selectOption("grid");
  const rows = panel.locator(".nav-row");
  await expect(rows).toHaveCount(2);
  const lm = [pts.a, pts.b, pts.c];
  // third row: the panel starts with two; add one if there is an Add button
  const add = panel.getByRole("button", { name: /Add (landmark|point|row)|\+/ });
  if (await add.count()) await add.first().click();
  const n = await rows.count();
  for (let i = 0; i < Math.min(n, 3); i++) {
    await rows.nth(i).locator(".nav-pos").fill(`${lm[i][1]}, ${lm[i][0]}`);
    await rows.nth(i).locator(".nav-az").fill(gridAz(target, lm[i]).toFixed(4));
  }
  await panel.getByRole("button", { name: "Solve" }).click();
  const fix = near(await panel.locator(".nav-result").innerText());
  expect(Math.abs(fix[0] - target[0]) + Math.abs(fix[1] - target[1])).toBeLessThan(0.003);

  await panel.getByRole("button", { name: "Add point to map" }).click();
  await expect.poll(() => gis(app, (g) => Object.values<any>(g.useApp.getState().layers).some((l) => l.name === "Resection fix"))).toBe(true);

  // Intersection from two known points toward the target
  await panel.locator(".mp-tabs button", { hasText: "Intersection" }).click();
  await panel.locator("select").first().selectOption("grid");
  const rows2 = panel.locator(".nav-row");
  const kp = [pts.a, pts.b];
  for (let i = 0; i < 2; i++) {
    await rows2.nth(i).locator(".nav-pos").fill(`${kp[i][1]}, ${kp[i][0]}`);
    await rows2.nth(i).locator(".nav-az").fill(gridAz(kp[i], target).toFixed(4));
  }
  await panel.getByRole("button", { name: "Solve" }).click();
  const ifix = near(await panel.locator(".nav-result").innerText());
  expect(Math.abs(ifix[0] - target[0]) + Math.abs(ifix[1] - target[1])).toBeLessThan(0.002);
});
