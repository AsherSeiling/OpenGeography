// LW9: the Topo Map Builder builds a land-navigation sheet from a position, scale and paper, and lists what it checked.
import { test, expect, gis } from "./fixtures";

test("LW9 build a 1:24,000 sheet from an MGRS reference and read the self-check", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /Topo Map/ }).click();
  const dlg = app.getByRole("dialog", { name: "Topo Map Builder" });
  await expect(dlg).toBeVisible();

  // A position that cannot be read is explained in words.
  await dlg.getByRole("combobox", { name: "Area" }).click();
  await app.getByRole("option", { name: "A position I type" }).click();
  await dlg.getByRole("textbox", { name: "Position" }).fill("somewhere nice");
  await dlg.getByRole("button", { name: "Build the sheet" }).click();
  await expect(dlg.getByText(/not a position I can read/)).toBeVisible();

  await dlg.getByRole("textbox", { name: "Position" }).fill("15T VG 9747");
  await dlg.getByRole("textbox", { name: "Sheet title" }).fill("Example Quad");
  await dlg.getByRole("button", { name: "Build the sheet" }).click();
  await expect(dlg.getByTestId("topo-checks")).toBeVisible();
  await expect(dlg.getByText(/Scale 1:24,000 on tabloid landscape/)).toBeVisible();
  await expect(dlg.getByText(/Declination .* at the centre/)).toBeVisible();
  await expect(dlg.getByText(/No elevation raster yet/)).toBeVisible();

  const doc = (await gis(app, (g) => g.useLayouts.getState().doc)) as any;
  expect(doc.name).toBe("Example Quad");
  const kinds = doc.elements.map((e: any) => e.kind);
  for (const k of ["map_frame", "declination", "scale_bar", "legend", "sources", "text"]) expect(kinds).toContain(k);
  const frame = doc.elements.find((e: any) => e.kind === "map_frame");
  expect(frame.crs).toBe("EPSG:26915");
  expect(frame.scale).toBe(24000);
  expect(frame.grid).toMatchObject({ interval: 1000, designation: true, labelFormat: "short" });

  // The page draws without complaints about its own elements.
  const warnings = (await gis(app, async (g) => (await g.api.renderLayoutPreview(g.layoutActions.layoutRequest(g.useLayouts.getState().doc), 72)).warnings)) as string[];
  expect(warnings.filter((w) => /Declination|Scale bar|North arrow|Legend|grid/i.test(w))).toEqual([]);
});
