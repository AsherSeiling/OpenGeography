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

test("LW9 data step: contours (index lines labelled) and a hillshade are made from an elevation layer", async ({ app }) => {
  const { writeAsc } = await import("./pixels");
  const fs = await import("node:fs");
  const os = await import("node:os");
  const path = await import("node:path");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "topo-"));
  const dem = path.join(dir, "dem.asc");
  writeAsc(dem, 80, 80, -93.4, 41.6, 0.01, (c, r) => 100 + c * 2 + r); // a tilted plane, 100–259 m
  await gis(app, async (g, a: any) => { const [l] = await g.api.importFiles([a.f]); g.useApp.getState().addLayers([l]); }, { f: dem });

  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /Topo Map/ }).click();
  const dlg = app.getByRole("dialog", { name: "Topo Map Builder" });
  await dlg.getByRole("combobox", { name: "Area" }).click();
  await app.getByRole("option", { name: "A position I type" }).click();
  await dlg.getByRole("textbox", { name: "Position" }).fill("42.0, -93.0");
  await dlg.getByRole("textbox", { name: "Sheet title" }).fill("Plane Quad");
  await dlg.getByRole("button", { name: "Build the sheet" }).click();
  await expect(dlg.getByTestId("topo-checks")).toBeVisible();
  await expect(dlg.getByText(/^Prepare the data/)).toBeVisible();

  await dlg.getByRole("button", { name: /2\. Make contours and hillshade/ }).click();
  await expect(dlg.getByText(/contour lines every 40 ft .* and a hillshade/)).toBeVisible();
  const st = (await gis(app, (g) => {
    const s = g.useApp.getState();
    const byName = (p: string) => Object.values(s.layers as Record<string, any>).find((l: any) => l.name.startsWith(p)) as any;
    const c = byName("Contours — Plane Quad"), h = byName("Hillshade — Plane Quad"), d = byName("dem");
    return { c: !!c, h: !!h, style: s.styles[c.id], demVisible: s.visible[d.id], cTop: s.order.indexOf(c.id) < s.order.indexOf(h.id), hillshadeRenderer: s.styles[h.id].raster?.renderer, features: c.featureCount };
  })) as any;
  expect(st.c && st.h).toBe(true);
  expect(st.features).toBeGreaterThan(3);
  expect(st.style).toMatchObject({ kind: "categorized", field: "INDEX" });
  expect(st.style.label).toMatchObject({ enabled: true, field: "ELEV", indexOnly: true });
  expect(st.hillshadeRenderer).toBe("gray");
  expect(st.demVisible).toBe(false);
  expect(st.cTop).toBe(true);
  if (process.env.TOPO_PNG) {
    // Look at it: TOPO_PNG=/some/file.png writes the layout page as drawn.
    await dlg.getByRole("button", { name: "Close" }).click();
    await app.waitForTimeout(2500);
    await app.locator(".layout-page").screenshot({ path: process.env.TOPO_PNG });
  }
});
