import { test, expect, gis, ribbon } from "./fixtures";

test("D2 Census dialog guesses the GEOID field, checks variable codes, and the browser build explains it has no keychain", async ({ app, errors }) => {
  await gis(app, async (g) => {
    const info = await g.api.createFeatures("Tracts", [{ wkt: "POINT(-97 35)", props: { GEOID: "40109100100", name: "a" } }, { wkt: "POINT(-96.9 35.1)", props: { GEOID: "40109100200", name: "b" } }]);
    g.useApp.getState().addLayers([info]);
  });
  await ribbon(app, "Data", /Census Data/);
  const dlg = app.getByRole("dialog");
  await expect(dlg).toContainText("Census data — join to Tracts");
  await expect(dlg.getByRole("combobox").first()).toContainText("GEOID");
  await expect(dlg.getByRole("checkbox", { name: /Total population \(B01003_001E\)/ })).toBeChecked();
  const join = dlg.getByRole("button", { name: "Join" });
  await expect(join).toBeEnabled();
  await dlg.getByPlaceholder(/Other variable codes/).fill("B19013-001E");
  await expect(dlg).toContainText(/not Census variable codes/);
  await expect(join).toBeDisabled();
  await dlg.getByPlaceholder(/Other variable codes/).fill("B19013_001E");
  await expect(join).toBeEnabled();
  // Dataset change swaps the presets and the years.
  await dlg.getByRole("combobox").nth(1).click();
  await app.getByRole("option", { name: /Decennial Census, Demographic/ }).click();
  await expect(dlg.getByRole("checkbox", { name: /Total population \(2020 Census\)/ })).toBeVisible();
  // No keychain in the browser build: saying so beats silently dropping the key.
  await dlg.getByPlaceholder("Paste your key").fill("not-a-real-key");
  await dlg.getByRole("button", { name: "Save in keychain" }).click();
  await expect(dlg).toContainText(/no system keychain/);
  await dlg.getByRole("button", { name: "Cancel" }).click();
  void errors;
});
