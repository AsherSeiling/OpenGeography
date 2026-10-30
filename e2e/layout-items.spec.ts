// LW1/LW2: opening a layout turns the window into a layout workspace with an Items catalog (search, eye, lock, rename, reorder, group).
import { test, expect, gis, ribbon } from "./fixtures";

const doc = (page: any) => gis(page, (g) => g.useLayouts.getState().doc) as Promise<any>;

test("LW1/LW2 the Items catalog lists every element, renames, locks, searches and reorders", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await ribbon(app, "Layout", /^Rectangle$/);
  await ribbon(app, "Layout", /^Ellipse$/);
  await ribbon(app, "Layout", /^Triangle$/);

  const items = app.getByLabel("Layout items");
  await expect(items).toBeVisible();
  await expect(app.getByRole("region", { name: "Properties" })).toBeVisible();
  await expect(items.getByTestId("items-page")).toContainText("mm");
  await expect(items.locator(".row[data-element]")).toHaveCount(3);

  const ids: string[] = (await doc(app)).elements.map((e: any) => e.id);
  // Rename by double-click.
  const first = items.locator(`[data-element="${ids[0]}"]`);
  await first.dblclick();
  await items.getByRole("textbox", { name: /^Name of/ }).fill("Box A");
  await items.getByRole("textbox", { name: /^Name of/ }).press("Enter");
  expect((await doc(app)).elements.find((e: any) => e.id === ids[0]).name).toBe("Box A");

  // Search narrows the list.
  await items.getByRole("textbox", { name: "Search items" }).fill("box a");
  await expect(items.locator(".row[data-element]")).toHaveCount(1);
  await items.getByRole("textbox", { name: "Search items" }).fill("");

  // Lock from the row.
  await items.getByRole("button", { name: `Lock ${ids[1]}` }).click();
  expect((await doc(app)).elements.find((e: any) => e.id === ids[1]).locked).toBe(true);

  // Drag the bottom-most item (ids[0]) above the top one: it becomes the last drawn.
  const top = items.locator(`[data-element="${ids[2]}"]`);
  await first.dragTo(top, { targetPosition: { x: 20, y: 2 } });
  expect((await doc(app)).elements.map((e: any) => e.id)).toEqual([ids[1], ids[2], ids[0]]);

  // Shift-click a range, then Group.
  await items.locator(`[data-element="${ids[0]}"]`).click();
  await items.locator(`[data-element="${ids[1]}"]`).click({ modifiers: ["Shift"] });
  expect(((await gis(app, (g) => g.useLayouts.getState().selected)) as string[]).length).toBe(3);
  await items.getByRole("button", { name: "Group", exact: true }).click();
  expect(new Set((await doc(app)).elements.map((e: any) => e.group)).size).toBe(1);
  await items.getByRole("button", { name: "Ungroup" }).click();
  expect((await doc(app)).elements.every((e: any) => !e.group)).toBe(true);
});

test("LW1/LW4 layout ribbon tabs, pointer readout and the Character panel", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();

  // Four contextual tabs under the Layout band, each with its own groups and no scroll bar.
  for (const [tab, group] of [["Layout", "Page"], ["Elements", "Graphics"], ["Format", "Arrange"], ["Export", "Export"]]) {
    await app.locator(".rtab.ctx", { hasText: new RegExp(`^${tab}$`) }).click();
    await expect(app.locator(`.rg-slot[data-group="${group}"]`)).toHaveCount(1);
  }

  // Pointer position over the page, in mm.
  const box = (await app.locator(".layout-page").boundingBox())!;
  await app.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await expect(app.getByTestId("layout-pointer")).toContainText(/\d+(\.\d)?, \d+(\.\d)? mm/);

  // Character panel on a text element: underline, letter spacing and halo are stored as the text slot's style.
  await ribbon(app, "Layout", /^Text$/);
  await expect(app.locator(".props-section-title", { hasText: "Character" })).toBeVisible();
  await app.getByRole("checkbox", { name: "Underline" }).check();
  await app.getByRole("spinbutton", { name: "Halo width (mm)" }).fill("0.4");
  const t = (await doc(app)).elements.find((e: any) => e.kind === "text");
  expect(t.textStyles["text.text"].underline).toBe(true);
  expect(t.textStyles["text.text"].haloWidth).toBe(0.4);
  await expect(app.getByTestId("layout-selsize")).toContainText("mm");
});
