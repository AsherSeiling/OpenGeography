import { test, expect, publicData } from "./fixtures";

test("D4 Overture dialog lists the themes and the area it will read", async ({ app }) => {
  await publicData(app, "overture");
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByRole("radio", { name: "Buildings" })).toBeChecked({ timeout: 10_000 });
  for (const t of [/Places of interest/, /Roads, paths and railways/, /^Water$/, /Land use/, /Administrative areas/]) await expect(dlg.getByRole("radio", { name: t })).toBeVisible();
  await expect(dlg).toContainText(/can take a few minutes/);
  await expect(dlg).toContainText(/ODbL/);
  await expect(dlg.getByRole("button", { name: "Download" })).toBeEnabled();
  await dlg.getByRole("button", { name: "Cancel" }).click();
});
