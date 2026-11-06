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

test("UR-9 a public-data download runs in the background: the dialog closes at once, the app stays usable, the layer arrives, and New Project asks first", async ({ app }) => {
  // A stand-in for the network: a download that takes 4 s and then returns a real layer.
  await app.evaluate(() => {
    const g = (window as any).__gis;
    g.api.overtureImport = async () => {
      await new Promise((r) => setTimeout(r, 4000));
      return g.api.generateTestLayer("points", 5, [-100, 30, -90, 40]);
    };
  });
  await publicData(app, "overture");
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByRole("radio", { name: "Buildings" })).toBeChecked({ timeout: 10_000 });
  await dlg.getByRole("button", { name: "Download" }).click();
  await expect(app.getByRole("dialog")).toHaveCount(0, { timeout: 4000 }); // not held open by the download (it waits a moment in case the request is refused at once)
  await expect(app.locator(".statusbar")).toContainText("runs in the background");

  // The rest of the app works meanwhile.
  await app.locator(".rtab", { hasText: /^Analysis$/ }).click();
  await expect(app.locator(".rtab.active, .rtab[aria-selected=true]", { hasText: /^Analysis$/ }).first()).toBeVisible();
  expect(await app.evaluate(() => Object.keys((window as any).__gis.useApp.getState().layers).length)).toBe(0);

  // New Project asks; answering No keeps the project and the download.
  let asked = "";
  app.once("dialog", (d) => { asked = d.message(); void d.dismiss(); });
  await app.evaluate(() => void (window as any).__gis.actions.newProject());
  await expect.poll(() => asked).toContain("download is still running");
  await expect.poll(() => app.evaluate(() => Object.keys((window as any).__gis.useApp.getState().layers).length), { timeout: 15_000 }).toBe(1); // it landed
  await expect(app.locator(".statusbar")).toContainText(/Added/);

  // Saving while one runs asks as well; Cancel stops the save.
  await app.evaluate(() => {
    const g = (window as any).__gis;
    g.api.overtureImport = async () => { await new Promise((r) => setTimeout(r, 3000)); return g.api.generateTestLayer("points", 3, [-100, 30, -90, 40]); };
  });
  await publicData(app, "overture");
  await app.getByRole("dialog").getByRole("button", { name: "Download" }).click();
  let asked2 = "";
  app.once("dialog", (d) => { asked2 = d.message(); void d.dismiss(); });
  await app.evaluate(() => void (window as any).__gis.actions.saveProject());
  await expect.poll(() => asked2).toContain("Save now?");
  await expect.poll(() => app.evaluate(() => Object.keys((window as any).__gis.useApp.getState().layers).length), { timeout: 15_000 }).toBe(2);
});

test("UR-9 a request refused at once keeps the dialog open with the reason (every download dialog uses the same start)", async ({ app }) => {
  await app.evaluate(() => {
    (window as any).__gis.api.overtureImport = async () => { throw new Error("This area is too large for this theme. Choose a smaller area."); };
  });
  await publicData(app, "overture");
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByRole("radio", { name: "Buildings" })).toBeChecked({ timeout: 10_000 });
  await dlg.getByRole("button", { name: "Download" }).click();
  await expect(dlg).toContainText("This area is too large for this theme");
  await expect(dlg.getByRole("button", { name: "Download" })).toBeEnabled(); // can be corrected and tried again
  await dlg.getByRole("button", { name: "Cancel" }).click();
});
