// Bug U-5 (layout Properties pane blank, edits lost). Cases LP-4, LP-5 of docs/TEST_PLAN.md §9A.
import { test, expect, gis, ribbon } from "./fixtures";

const layoutState = (page: any) => gis(page, (g) => { const s = g.useLayouts.getState(); return { open: s.id, selected: s.selected, props: g.useApp.getState().properties, text: s.doc?.elements.find((e: any) => e.kind === "text") }; }) as Promise<any>;

test("LP-5 clicking into Properties and editing a value keeps the pane and stores the edit", async ({ app }) => {
  await app.locator(".rtab", { hasText: /^Insert$/ }).click();
  await app.locator(".rbtn", { hasText: /New Layout/ }).click();
  await app.getByRole("menuitem", { name: /Letter landscape/ }).click();
  await expect(app.locator(".layout-page")).toBeVisible();
  await ribbon(app, "Layout", /^Text$/);
  const before = await layoutState(app);
  expect(before.selected).toHaveLength(1);

  const x = app.getByLabel("X (mm)");
  await expect(x).toBeVisible();
  for (const v of ["20", "25", "30"]) {
    await x.click(); // activates the Properties dock panel
    await x.fill(v);
    await app.waitForTimeout(150);
    const s = await layoutState(app);
    expect(s.open).not.toBeNull();
    expect(s.selected).toHaveLength(1);
    expect(s.props?.kind).toBe("layoutElement");
    expect(s.text.x).toBe(Number(v));
    await expect(app.getByLabel("X (mm)")).toHaveValue(v);
  }
});
