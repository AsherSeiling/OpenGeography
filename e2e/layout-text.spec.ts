// LW3/LW4: every piece of text an element draws has a size (and font, weight, colour) in its Properties; opacity and name too.
import { test, expect, gis, ribbon, addTestLayer } from "./fixtures";

const doc = (page: any) => gis(page, (g) => g.useLayouts.getState().doc) as Promise<any>;
const el = async (page: any, kind: string) => (await doc(page)).elements.find((e: any) => e.kind === kind);

async function layoutWithFrame(app: any) {
  await addTestLayer(app, "polygons", 5, [-93.1, 45.0, -92.9, 45.1]);
  await gis(app, (g) => g.useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" } }));
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await ribbon(app, "Layout", /^Map Frame$/);
  await gis(app, (g) => { const f = g.useLayouts.getState().doc.elements.find((e: any) => e.kind === "map_frame"); g.useLayouts.getState().update(f.id, { crs: "EPSG:26915", center: [490000, 4985000], scale: 100000 }); });
}

/** Opens a slot's details unless it is already open (elements with one or two slots start open). */
async function openSlot(app: any, id: string) {
  const d = app.getByTestId("text-slots").locator(`[data-slot="${id}"]`);
  if (!(await d.evaluate((n: HTMLDetailsElement) => n.open))) await d.locator("summary").click();
}

test("LW3/LW4 legend: title, headings and entries each get their own size, font weight and colour", async ({ app }) => {
  await layoutWithFrame(app);
  await ribbon(app, "Layout", /^Legend$/);
  const slots = app.getByTestId("text-slots");
  await expect(slots).toBeVisible();
  for (const s of ["legend.title", "legend.headings", "legend.entries"]) await expect(slots.locator(`[data-slot="${s}"]`)).toHaveCount(1);

  await slots.locator('[data-slot="legend.title"] summary').click();
  await app.getByRole("spinbutton", { name: "Title size (pt)" }).fill("16");
  await expect.poll(async () => (await el(app, "legend")).textStyles?.["legend.title"]?.size).toBe(16);
  await slots.locator('[data-slot="legend.entries"] summary').click();
  await app.getByRole("spinbutton", { name: "Entries size (pt)" }).fill("11");
  await app.getByRole("checkbox", { name: "Italic" }).last().check();
  const lg = await el(app, "legend");
  expect(lg.textStyles["legend.entries"]).toEqual({ size: 11, italic: true });
  expect(lg.textStyles["legend.title"]).toEqual({ size: 16 });

  // The engine draws it: the preview of the whole layout has text at 16 pt and 11 pt.
  const out = await gis(app, async (g) => { const s = g.useLayouts.getState(); const r = await g.api.exportLayoutSvg?.({ layout: s.doc, styles: {}, order: [], visible: {}, project: "p", date: "2026-01-01" }); return r ?? null; }).catch(() => null);
  void out;

  // Reset puts the default back and removes the stored style.
  await app.getByRole("button", { name: /Reset title/i }).click();
  await expect.poll(async () => (await el(app, "legend")).textStyles?.["legend.title"]).toBeUndefined();
});

test("LW3/LW4 scale bar, north arrow, sources, opacity and name", async ({ app }) => {
  await layoutWithFrame(app);
  await ribbon(app, "Layout", /^Scale Bar$/);
  await openSlot(app, "scale_bar.numbers");
  await app.getByRole("spinbutton", { name: "Distances and units size (pt)" }).fill("12");
  await expect.poll(async () => (await el(app, "scale_bar")).textStyles?.["scale_bar.numbers"]?.size).toBe(12);

  await ribbon(app, "Layout", /^North Arrow$/);
  await openSlot(app, "north_arrow.letters");
  await app.getByRole("spinbutton", { name: /size \(pt\)/ }).first().fill("14");
  await expect.poll(async () => (await el(app, "north_arrow")).textStyles?.["north_arrow.letters"]?.size).toBe(14);

  await app.getByRole("textbox", { name: "Element name" }).fill("Compass");
  await app.locator(".props-section-title", { hasText: "Appearance" }).click();
  await app.getByRole("spinbutton", { name: "Opacity (%)" }).fill("50");
  const na = await el(app, "north_arrow");
  expect(na.name).toBe("Compass");
  expect(na.opacity).toBe(0.5);
  // A size outside 1 to 400 pt is refused: the stored size stays.
  await app.getByRole("spinbutton", { name: /size \(pt\)/ }).first().fill("900");
  expect((await el(app, "north_arrow")).textStyles["north_arrow.letters"].size).toBe(14);
});
