import { test, expect, gis } from "./fixtures";

test("UR-11 the coordinate-system picker can be browsed: kind → region → state → datum, and picking works; search is unchanged", async ({ app }) => {
  await gis(app, (g) => {
    (window as any).__picked = null;
    g.actions.useDialog.getState().show({ kind: "crs", title: "Pick", onPick: (id: string) => { (window as any).__picked = id; } });
  });
  const dlg = app.getByRole("dialog");
  await expect(dlg.locator(".crs-row").first()).toBeVisible({ timeout: 15_000 });
  await dlg.getByRole("button", { name: "Browse" }).click();
  const tree = dlg.getByRole("tree", { name: /by region/ });
  await expect(tree.getByRole("treeitem")).toHaveCount(2); // Projected, Geographic
  const open = async (key: string) => dlg.locator(`[data-crs-node="${key}"]`).click();
  await open("Projected");
  await open("Projected/USA");
  await open("Projected/USA/Texas");
  await open("Projected/USA/Texas/NAD83");
  await dlg.getByRole("option", { name: /EPSG:32138 / }).click();
  await dlg.getByRole("button", { name: "OK" }).click();
  await expect.poll(() => app.evaluate(() => (window as any).__picked)).toBe("EPSG:32138");
});

test("UR-11 typing a search while browsing shows the matches, and Browse can be turned off", async ({ app }) => {
  await gis(app, (g) => g.actions.useDialog.getState().show({ kind: "crs", title: "Pick", onPick: () => {} }));
  const dlg = app.getByRole("dialog");
  await expect(dlg.locator(".crs-row").first()).toBeVisible({ timeout: 15_000 });
  await dlg.getByRole("button", { name: "Browse" }).click();
  await expect(dlg.getByRole("tree")).toBeVisible();
  await dlg.getByPlaceholder(/Search by name/).fill("26914");
  await expect(dlg.getByRole("tree")).toHaveCount(0);
  await expect(dlg.getByRole("option", { name: /NAD83 \/ UTM zone 14N/ }).first()).toBeVisible();
  await dlg.getByPlaceholder(/Search by name/).fill("");
  await dlg.getByRole("button", { name: "Browse" }).click();
  await expect(dlg.getByRole("tree")).toHaveCount(0);
});
