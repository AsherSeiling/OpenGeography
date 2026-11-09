// Go To Coordinates (RB4). NOT YET RUN: written while another e2e run was in progress (2026-11-09); the lead runs it before merging.
import { test, expect, gis, ribbon } from "./fixtures";

const UTM14 = { id: "EPSG:32614", name: "WGS 84 / UTM zone 14N" };
const MERC = { id: "EPSG:3857", name: "WGS 84 / Pseudo-Mercator" };

async function useCrs(app: any, crs: { id: string; name: string }) {
  await gis(app, (g, c: any) => g.useApp.getState().setMapCrs(c), crs);
  await app.waitForFunction((id: string) => (window as any).__gis.mapController.getActiveCrs()?.id === id, crs.id, { timeout: 20_000 });
}
const still = (app: any) => app.waitForFunction(() => !(window as any).__gis.getMap().getView().getAnimating());
const viewCentre = (app: any) => gis(app, (g) => g.getMap().getView().getCenter()) as Promise<[number, number]>;
const expected = (app: any, lon: number, lat: number, crs: string) => gis(app, (g, a: any) => g.api.transformPoint(a[0], a[1], "EPSG:4326", a[2]), [lon, lat, crs]) as Promise<[number, number]>;
const markers = (app: any) => app.locator(".goto-marker");

test("RB4 typing a position on a UTM map and on a Web Mercator map centres it and puts the marker there; Clear removes it", async ({ app }) => {
  await useCrs(app, UTM14);
  const dirtyBefore = await gis(app, (g) => g.useApp.getState().dirty);

  await ribbon(app, "Map", "Go To Coordinates");
  const panel = app.getByTestId("goto-panel");
  await expect(panel).toBeVisible();
  await panel.getByLabel("Format of the position").selectOption("latlon");
  await panel.getByLabel("Latitude").fill("35");
  await panel.getByLabel("Longitude").fill("99 00 00 W");
  // How it was read is shown before going, with the other formats and the datum.
  await expect(app.getByTestId("goto-reading")).toContainText("latitude 35.000000° N, longitude 99.000000° W on WGS 84");
  await expect(panel).toContainText("MGRS (WGS 84)");
  await expect(panel.getByRole("button", { name: "Copy Latitude, longitude, decimal degrees (WGS 84)" })).toBeVisible();
  await app.keyboard.press("Enter");
  await expect(markers(app)).toHaveCount(1);
  await still(app);
  let want = await expected(app, -99, 35, UTM14.id);
  expect(want[0]).toBeCloseTo(500000, 0);
  let c = await viewCentre(app);
  expect(Math.abs(c[0] - want[0])).toBeLessThan(1);
  expect(Math.abs(c[1] - want[1])).toBeLessThan(1);
  // The marker sits on the centre of the view (the pixel of the point in the map's own CRS).
  const px = (await gis(app, (g) => g.mapController.markerPixel())) as [number, number];
  const size = (await gis(app, (g) => g.getMap().getSize())) as [number, number];
  expect(Math.abs(px[0] - size[0] / 2)).toBeLessThan(1.5);
  expect(Math.abs(px[1] - size[1] / 2)).toBeLessThan(1.5);

  // The marker is temporary: no layer, project not marked changed.
  expect(await gis(app, (g) => Object.keys(g.useApp.getState().layers).length)).toBe(0);
  expect(await gis(app, (g) => g.useApp.getState().dirty)).toBe(dirtyBefore);

  // A change of the map's coordinate system removes the marker cleanly (it is drawn in the map's CRS).
  await useCrs(app, MERC);
  await expect(markers(app)).toHaveCount(0);

  // Web Mercator: the same panel, another place.
  await panel.getByLabel("Latitude").fill("34.05");
  await panel.getByLabel("Longitude").fill("-118.25");
  await expect(app.getByTestId("goto-reading")).toContainText("latitude 34.050000° N");
  await app.keyboard.press("Enter");
  await expect(markers(app)).toHaveCount(1);
  await still(app);
  want = await expected(app, -118.25, 34.05, MERC.id);
  c = await viewCentre(app);
  expect(Math.abs(c[0] - want[0])).toBeLessThan(1);
  expect(Math.abs(c[1] - want[1])).toBeLessThan(1);
  const px2 = (await gis(app, (g) => g.mapController.markerPixel())) as [number, number];
  expect(Math.abs(px2[0] - size[0] / 2)).toBeLessThan(1.5);

  // Clear removes the marker; nothing else changes.
  await panel.getByRole("button", { name: "Clear", exact: true }).click();
  await expect(markers(app)).toHaveCount(0);
  expect(await gis(app, (g) => g.useApp.getState().dirty)).toBe(dirtyBefore);

  // Escape closes the panel.
  await panel.getByLabel("Latitude").focus();
  await app.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
});

test("RB4 a bad or swapped input shows a sentence and goes nowhere; pasting one string fills latitude then longitude", async ({ app }) => {
  await useCrs(app, MERC);
  await ribbon(app, "Map", "Go To Coordinates");
  const panel = app.getByTestId("goto-panel");
  await panel.getByLabel("Format of the position").selectOption("latlon");
  // Typing a pair with a comma into one box splits it as latitude then longitude.
  await panel.getByLabel("Latitude").fill("34.05, -118.25");
  await expect(panel.getByLabel("Latitude")).toHaveValue("34.05");
  await expect(panel.getByLabel("Longitude")).toHaveValue("-118.25");
  // Swapped values are refused, and never swapped for the user.
  await panel.getByLabel("Latitude").fill("-118.25");
  await panel.getByLabel("Longitude").fill("34.05");
  await expect(app.getByTestId("goto-error")).toContainText("look swapped");
  await expect(panel.getByRole("button", { name: "Go", exact: true })).toBeDisabled();
  await app.keyboard.press("Enter");
  await expect(markers(app)).toHaveCount(0);
  await expect(panel.getByLabel("Latitude")).toHaveValue("-118.25");
  // Beyond 90 degrees.
  await panel.getByLabel("Latitude").fill("95");
  await panel.getByLabel("Longitude").fill("200");
  await expect(app.getByTestId("goto-error")).toContainText("beyond 90");
  // A place Web Mercator cannot draw: a sentence, no jump.
  await panel.getByLabel("Latitude").fill("89");
  await panel.getByLabel("Longitude").fill("10");
  await expect(app.getByTestId("goto-map-message")).toContainText("can only draw");
  await expect(panel.getByRole("button", { name: "Go", exact: true })).toBeDisabled();
  await panel.getByRole("button", { name: "Close", exact: true }).click();
});
