import { test, expect, ribbon } from "./fixtures";

test("D5 OGC API dialog lists the catalog with its citation and explains a bad address", async ({ app }) => {
  await ribbon(app, "Data", /OGC API Features/);
  const dlg = app.getByRole("dialog");
  await expect(dlg.getByRole("radio", { name: /Water monitoring locations/ })).toBeChecked({ timeout: 10_000 });
  await expect(dlg).toContainText(/U\.S\. Geological Survey/);
  await dlg.getByRole("radio", { name: /A service address I paste/ }).check();
  await expect(dlg.getByRole("button", { name: "Download" })).toBeDisabled();
  await dlg.getByPlaceholder(/example\.gov/).fill("example.org/api");
  await dlg.getByRole("button", { name: "Find collections" }).click();
  await expect(dlg).toContainText(/Enter the API address starting with https/, { timeout: 10_000 });
  await dlg.getByRole("button", { name: "Cancel" }).click();
});
