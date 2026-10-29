import { test, expect, gis, ribbon } from "./fixtures";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

const state = (page: any) => gis(page, (g) => { const s = g.useLayouts.getState(); return { selected: s.selected, els: s.doc?.elements ?? [] }; }) as Promise<any>;

test("CG1 shapes, pictures and background/frame/shadow in a layout", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();

  for (const [label, shape] of [["Rectangle", "rectangle"], ["Ellipse", "ellipse"], ["Triangle", "triangle"], ["Line", "line"], ["Arrow", "arrow"]] as const) {
    await ribbon(app, "Layout", new RegExp(`^${label}$`));
    const s = await state(app);
    expect(s.els.find((e: any) => e.id === s.selected[0]).shape).toBe(shape);
  }

  // A picture from a file on the machine running the dev server (the browser build prompts for the path).
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cg1-"));
  const file = path.join(dir, "logo.svg");
  fs.writeFileSync(file, '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#2060c0"/></svg>');
  app.once("dialog", (d) => d.accept(file));
  await ribbon(app, "Layout", /^Picture$/);
  await expect.poll(async () => (await state(app)).els.find((e: any) => e.kind === "picture")?.source).toMatch(/^data:image\/svg\+xml;base64,/);

  // Decor on the selected picture: background, frame and shadow from Properties.
  await app.getByText("Background, frame and shadow").click();
  await app.getByRole("checkbox", { name: "Background" }).check();
  await app.getByRole("checkbox", { name: "Drop shadow" }).check();
  const pic = (await state(app)).els.find((e: any) => e.kind === "picture");
  expect(pic.decor.background).toBe("#ffffff");
  expect(pic.decor.shadow).toBe(true);

  // The engine accepts the whole layout and draws every element.
  const out = await gis(app, async (g) => { const s = g.useLayouts.getState(); return g.api.renderLayoutPreview({ layout: s.doc, styles: {}, order: [], visible: {}, project: "p", date: "2026-01-01" }, 96); }) as any;
  expect(out.warnings).toEqual([]);
});
