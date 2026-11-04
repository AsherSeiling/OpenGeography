// WP13: the layout editor, option B — Insert rail, start gallery, Properties tabs, right-click and Shift+A menus, Maps on the rail,
// pre-flight, named text styles and the ribbon's layout tabs. Screenshots go to WP13_SHOTS when it is set.
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { test, expect, gis, addTestLayer } from "./fixtures";

const SHOTS = process.env.WP13_SHOTS;
const shot = async (page: any, name: string) => {
  if (!SHOTS) return;
  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, `${name}.png`) });
};
const doc = (page: any) => gis(page, (g) => g.useLayouts.getState().doc) as Promise<any>;
const kinds = async (page: any) => (await doc(page)).elements.map((e: any) => e.kind) as string[];

async function emptyLayout(app: any) {
  await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
  await gis(app, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  await expect(app.locator(".layout-page")).toBeVisible();
}
const blank = (app: any) => app.getByTestId("start-gallery").locator('[data-start="blank"]').click();

test.beforeEach(async ({ app }) => {
  await app.setViewportSize({ width: 1600, height: 950 });
});

test("an empty page offers the start gallery, and its hint tells the truth once the gallery is closed", async ({ app }) => {
  await emptyLayout(app);
  const gallery = app.getByTestId("start-gallery");
  await expect(gallery).toBeVisible();
  await expect(gallery.locator(".tile")).toHaveCount(6);
  await expect(gallery.locator(".tile b")).toHaveText(["Thematic map", "Land navigation", "USGS quadrangle", "Two maps side by side", "Map with inset", "Blank page"]);
  await shot(app, "gallery-light");
  await gis(app, (g) => g.useApp.getState().set({ dark: true }));
  await app.waitForTimeout(400);
  await shot(app, "gallery-dark");
  await gis(app, (g) => g.useApp.getState().set({ dark: false }));
  await blank(app);
  await expect(gallery).toHaveCount(0);
  const hint = app.getByTestId("empty-hint");
  await expect(hint).toContainText("Insert rail");
  await expect(hint).not.toContainText("Elements tab");
  // The commands the hint names really are where it says.
  await expect(app.locator(".rail-btn", { hasText: /^Map Frame/ })).toBeVisible();
  await hint.getByRole("button", { name: "Show the starts" }).click();
  await expect(gallery).toBeVisible();
});

test("the Thematic start adds a title, a map frame, legend, scale bar, north arrow and sources", async ({ app }) => {
  await addTestLayer(app, "polygons", 10, [-100, 30, -95, 35]);
  await emptyLayout(app);
  await app.getByTestId("start-gallery").locator('[data-start="thematic"]').click();
  await expect.poll(async () => (await kinds(app)).sort().join()).toBe(["legend", "map_frame", "north_arrow", "scale_bar", "sources", "text"].join());
  await expect(app.getByTestId("start-gallery")).toHaveCount(0);
  // One undo step takes the whole start back out.
  await app.locator(".layout-canvas").focus();
  await app.keyboard.press("ControlOrMeta+z");
  await expect.poll(async () => (await kinds(app)).length).toBe(0);
});

test("Map tabs leave while a layout is open, the layout tabs follow the doc, Back to Map returns", async ({ app }) => {
  await emptyLayout(app);
  const tabs = await app.locator(".rtabs .rtab").allTextContents();
  expect(tabs.map((t: string) => t.trim())).toEqual(["Page", "Insert", "Arrange", "View", "Export"]);
  await expect(app.locator(".rtab", { hasText: /^(Map|Analysis|Feature Layer)$/ })).toHaveCount(0);
  await app.locator(".rtab", { hasText: /^Page$/ }).click();
  for (const g of ["Sheet", "Look", "Template", "Map Series"]) await expect(app.locator(`.rg-slot[data-group="${g}"]`)).toHaveCount(1);
  await app.locator(".rtab", { hasText: /^Arrange$/ }).click();
  for (const g of ["Order", "Line Up", "Group"]) await expect(app.locator(`.rg-slot[data-group="${g}"]`)).toHaveCount(1);
  await app.locator(".rtab", { hasText: /^Export$/ }).click();
  for (const g of ["This Page", "Quality", "Map Series"]) await expect(app.locator(`.rg-slot[data-group="${g}"]`)).toHaveCount(1);
  // Insert > Maps stays on the ribbon.
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await expect(app.locator(".rbtn", { hasText: /^New Map$/ })).toBeVisible();
  await expect(app.locator(".rbtn", { hasText: /^Duplicate Map$/ })).toBeVisible();
  // Map Frame comes with a selected frame and goes with it.
  await expect(app.locator(".rtab", { hasText: /^Map Frame$/ })).toHaveCount(0);
  await blank(app);
  await app.locator(".rail-btn", { hasText: /^Map Frame/ }).click();
  await expect(app.locator(".rtab", { hasText: /^Map Frame$/ })).toHaveCount(1);
  await app.locator(".rtab", { hasText: /^Map Frame$/ }).click();
  await expect(app.locator(".rbtn", { hasText: /^Set from Map View$/ })).toBeVisible();
  await expect(app.locator(".rbtn", { hasText: /^Frame CRS$/ })).toBeVisible();
  await gis(app, (g) => g.useLayouts.getState().select([]));
  await expect(app.locator(".rtab", { hasText: /^Map Frame$/ })).toHaveCount(0);
  await app.locator(".rtab-back").click();
  await expect(app.locator(".rtab", { hasText: /^Map$/ })).toBeVisible();
  await expect(app.locator(".rtab-back")).toHaveCount(0);
});

test("Properties follow the selection in tabs, what it is first and Position last; the page shows Sheet, Guides, Info", async ({ app }) => {
  await emptyLayout(app);
  await blank(app);
  const tabs = () => app.getByRole("tablist", { name: "Properties" }).getByRole("tab").allTextContents();
  await expect.poll(tabs).toEqual(["Sheet", "Guides", "Text Styles", "Info"]);
  await app.getByRole("tab", { name: "Sheet" }).click();
  await expect(app.getByRole("combobox", { name: "Page size" })).toBeVisible();
  await app.locator(".rail-btn", { hasText: /^Text$/ }).click();
  await expect.poll(tabs).toEqual(["Text", "Style", "Position"]);
  await app.getByRole("tab", { name: "Position" }).click();
  await expect(app.getByRole("spinbutton", { name: "X (mm)" })).toBeVisible();
  await app.locator(".rail-btn", { hasText: /^Map Frame/ }).click();
  await expect.poll(tabs).toEqual(["Map", "Layers", "Grid", "Style", "Position"]);
  await app.getByRole("tablist", { name: "Properties" }).getByRole("tab", { name: "Map", exact: true }).click();
  await expect(app.getByTestId("frame-map-picker")).toBeVisible(); // "Layers from" stays
  await shot(app, "properties-frame-light");
  await app.locator(".rail-btn", { hasText: /^Legend$/ }).click();
  await expect.poll(tabs).toEqual(["Entries", "Text", "Style", "Position"]);
});

test("right-click: the page offers Paste, Insert and Page Setup; an element Cut to Lock; a frame adds Set from Map View and Which Map", async ({ app }) => {
  await emptyLayout(app);
  await blank(app);
  const canvas = app.locator(".layout-canvas");
  const box = (await app.locator(".layout-page").boundingBox())!;
  await app.mouse.click(box.x + 20, box.y + 20, { button: "right" });
  await expect(app.getByRole("menuitem")).toHaveText(["PasteCtrl+V", "Insert…Shift+A", "Page Setup…"]);
  await app.keyboard.press("Escape");
  await app.locator(".rail-btn", { hasText: /^Text$/ }).click();
  const el = app.locator(".layout-el").first();
  await el.click({ button: "right" });
  await expect(app.getByRole("menuitem")).toHaveText(["CutCtrl+X", "CopyCtrl+C", "DuplicateCtrl+D", "DeleteDel", "Order▸", "Align▸", "Group", "Lock"]);
  await app.getByRole("menuitem", { name: /^Duplicate/ }).click();
  await expect.poll(async () => (await kinds(app)).length).toBe(2);
  // Copy, then Paste from the page's own menu.
  await canvas.focus();
  await app.keyboard.press("ControlOrMeta+c");
  await app.mouse.click(box.x + 20, box.y + box.height - 20, { button: "right" });
  await app.getByRole("menuitem", { name: /^Paste/ }).click();
  await expect.poll(async () => (await kinds(app)).length).toBe(3);
  // Lock from the menu.
  await app.locator(".layout-el.on").first().click({ button: "right" });
  await app.getByRole("menuitem", { name: "Lock" }).click();
  expect((await doc(app)).elements.filter((e: any) => e.locked).length).toBeGreaterThan(0);
  await gis(app, (g) => g.useLayouts.getState().select([]));
  await app.locator(".rail-btn", { hasText: /^Map Frame/ }).click();
  await app.locator(".layout-el.on").first().click({ button: "right" });
  await expect(app.getByRole("menuitem").nth(0)).toHaveText("Set from Map View");
  await expect(app.getByRole("menuitem").nth(1)).toHaveText("Which Map▸");
});

test("Shift+A finds an element and inserts it where the pointer is", async ({ app }) => {
  await emptyLayout(app);
  await blank(app);
  const box = (await app.locator(".layout-page").boundingBox())!;
  await app.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await app.locator(".layout-canvas").focus();
  await app.keyboard.press("Shift+A");
  await expect(app.getByRole("dialog", { name: "Insert an element" })).toBeVisible();
  await app.keyboard.type("ellip");
  await expect(app.getByRole("dialog", { name: "Insert an element" }).getByRole("option")).toHaveCount(1);
  await app.keyboard.press("Enter");
  await expect.poll(async () => (await kinds(app)).join()).toBe("shape");
  const e = (await doc(app)).elements[0];
  expect(e.x).toBeGreaterThan(80);
  expect(e.x).toBeLessThan(200);
  // Escape closes it without inserting.
  await app.locator(".layout-canvas").focus();
  await app.keyboard.press("Shift+A");
  await app.keyboard.press("Escape");
  await expect(app.getByRole("dialog", { name: "Insert an element" })).toHaveCount(0);
});

test("Maps on the rail: drag a map onto the page to make a frame that draws it; Which Map changes it", async ({ app }) => {
  await addTestLayer(app, "polygons", 6);
  const second = (await app.evaluate(`(async () => { const m = await import("/src/maps/mapActions.ts"); return m.newMap({ side: true }); })()`)) as any;
  await emptyLayout(app);
  await blank(app);
  const chips = app.locator(".rail-map");
  await expect(chips).toHaveCount(2);
  await chips.nth(1).dragTo(app.locator(".layout-canvas"), { targetPosition: { x: 200, y: 120 } });
  await expect.poll(async () => (await doc(app)).elements.find((e: any) => e.kind === "map_frame")?.layers).toEqual({ map: second.id });
  // Which Map on the Map Frame tab.
  await app.locator(".rtab", { hasText: /^Map Frame$/ }).click();
  await app.locator(".rbtn", { hasText: /Which Map/ }).click();
  await app.getByRole("menuitemradio", { name: "The active map" }).click();
  await expect.poll(async () => (await doc(app)).elements.find((e: any) => e.kind === "map_frame")?.layers).toBe("map");
  // The Map Frame button asks which map once there is more than one.
  await app.locator(".rail-btn", { hasText: /^Map Frame/ }).click();
  await expect(app.getByRole("menuitem", { name: second.name })).toBeVisible();
  await app.keyboard.press("Escape");
});

test("Check Page lists what the export would not tell; the buttons still export", async ({ app }) => {
  await emptyLayout(app);
  await blank(app);
  await app.locator(".rtab", { hasText: /^Export$/ }).click();
  await app.locator(".rbtn", { hasText: /^Check Page$/ }).click();
  const panel = app.getByTestId("preflight");
  await expect(panel).toBeVisible();
  await expect(panel.locator('[data-check="frames"]')).toHaveAttribute("data-ok", "false");
  await app.locator(".rail-btn", { hasText: /^Map Frame/ }).click();
  await expect(panel.locator('[data-check="crs"]')).toHaveAttribute("data-ok", "true");
  await expect(panel.locator('[data-check="sources"]')).toHaveAttribute("data-ok", "true");
  await app.locator(".rail-btn", { hasText: /^Legend$/ }).click();
  await app.locator(".rail-btn", { hasText: /^Scale Bar$/ }).click();
  await expect(panel.locator('[data-check="legend"]')).toHaveAttribute("data-ok", "true");
  await expect(panel.locator('[data-check="scale"]')).toHaveAttribute("data-ok", "true");
  // Take the frame away: the page no longer has a map, and the buttons stay.
  await gis(app, (g) => { const s = g.useLayouts.getState(); s.removeElements(s.doc.elements.filter((e: any) => e.kind === "map_frame").map((e: any) => e.id)); });
  await expect(panel.locator('[data-check="frames"]')).toHaveAttribute("data-ok", "false");
  await expect(panel.getByRole("button", { name: "Export PDF" })).toBeEnabled();
  await panel.getByRole("button", { name: "Close the check" }).click();
  await expect(panel).toHaveCount(0);
});

test("a named text style changes once, everywhere", async ({ app }) => {
  await emptyLayout(app);
  await blank(app);
  await app.locator(".rail-btn", { hasText: /^Text$/ }).click();
  await app.locator(".rail-btn", { hasText: /^Text$/ }).click();
  const ids = (await doc(app)).elements.map((e: any) => e.id) as string[];
  // Both take "Title" from the bar over the selected element.
  for (const id of ids) {
    await gis(app, (g, i: string) => g.useLayouts.getState().select([i]), id);
    await app.getByRole("toolbar", { name: "Selected element" }).getByLabel("Text style").selectOption("Title");
  }
  expect((await doc(app)).elements.map((e: any) => e.size)).toEqual([21, 21]);
  await gis(app, (g) => g.useLayouts.getState().select([]));
  await app.getByRole("tab", { name: "Text Styles" }).click();
  const title = app.locator('[data-style="Title"]');
  await expect(title).toContainText("2 in use");
  const size = title.getByRole("spinbutton", { name: "Title size" });
  await size.fill("30");
  await size.blur();
  await expect.poll(async () => (await doc(app)).elements.map((e: any) => e.size)).toEqual([30, 30]);
  await gis(app, (g) => g.useLayouts.getState().undo());
  await expect.poll(async () => (await doc(app)).elements.map((e: any) => e.size)).toEqual([21, 21]);
});

test("Lock on the Arrange tab, the zoom buttons on the View tab, Cut and Paste on the keyboard", async ({ app }) => {
  await emptyLayout(app);
  await blank(app);
  await app.locator(".rail-btn", { hasText: /^Text$/ }).click();
  await app.locator(".rtab", { hasText: /^Arrange$/ }).click();
  await app.locator(".rbtn", { hasText: /^Lock$/ }).click();
  expect((await doc(app)).elements[0].locked).toBe(true);
  await app.locator(".rbtn", { hasText: /^Lock$/ }).click();
  expect((await doc(app)).elements[0].locked).toBe(false);
  await app.locator(".rtab", { hasText: /^View$/ }).click();
  await app.locator(".rbtn", { hasText: /^100%$/ }).click();
  await expect(app.locator(".layout-toolbar .muted").first()).toHaveText("100%");
  await app.locator(".rbtn", { hasText: /^Zoom In$/ }).click();
  await expect(app.locator(".layout-toolbar .muted").first()).toHaveText("125%");
  await app.locator(".rbtn", { hasText: /^Fit Page$/ }).click();
  await app.locator(".layout-canvas").focus();
  await app.keyboard.press("ControlOrMeta+x");
  await expect.poll(async () => (await kinds(app)).length).toBe(0);
  await app.keyboard.press("ControlOrMeta+v");
  await expect.poll(async () => (await kinds(app)).length).toBe(1);
});

test("the page stays white in the dark theme, and the editor draws in both themes", async ({ app }) => {
  await addTestLayer(app, "polygons", 10, [-100, 30, -95, 35]);
  await emptyLayout(app);
  await app.getByTestId("start-gallery").locator('[data-start="thematic"]').click();
  await expect.poll(async () => (await kinds(app)).length).toBeGreaterThan(4);
  await app.locator(".layout-img").waitFor({ timeout: 20_000 });
  await app.locator(".layout-el").nth(1).click();
  await shot(app, "editor-light");
  await gis(app, (g) => g.useApp.getState().set({ dark: true }));
  await app.waitForTimeout(500);
  expect(await app.locator(".layout-page").evaluate((e: HTMLElement) => getComputedStyle(e).backgroundColor)).toBe("rgb(255, 255, 255)");
  await shot(app, "editor-dark");
  await app.locator(".layout-el").nth(2).click({ button: "right" });
  await shot(app, "context-menu-dark");
  await app.keyboard.press("Escape");
  await app.locator(".rtab", { hasText: /^Export$/ }).click();
  await app.locator(".rbtn", { hasText: /^Check Page$/ }).click();
  await app.waitForTimeout(600);
  await shot(app, "preflight-dark");
  await gis(app, (g) => g.useApp.getState().set({ dark: false }));
});

test("keyboard: the rail, the tabs of Properties and the menus work without a mouse", async ({ app }) => {
  await emptyLayout(app);
  await blank(app);
  // Every rail button has a name.
  for (const b of await app.locator(".rail button").all()) expect(((await b.innerText()).trim() || (await b.getAttribute("aria-label")) || "")).not.toBe("");
  await app.locator(".rail-btn", { hasText: /^Text$/ }).focus();
  await app.keyboard.press("Enter");
  await expect.poll(async () => (await kinds(app)).length).toBe(1);
  await app.getByRole("tablist", { name: "Properties" }).getByRole("tab", { name: "Text", exact: true }).focus();
  await app.keyboard.press("ArrowRight");
  await expect(app.getByRole("tab", { name: "Style", selected: true })).toBeFocused();
  // The context menu opens from the keyboard and walks with the arrow keys.
  await app.locator(".layout-canvas").focus();
  await app.keyboard.press("Shift+F10");
  await expect(app.getByRole("menu")).toBeVisible();
  await app.keyboard.press("ArrowDown");
  await app.keyboard.press("Escape");
  await expect(app.getByRole("menu")).toHaveCount(0);
});
