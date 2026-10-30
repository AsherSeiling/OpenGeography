// Shared setup: open the app against the devserver, no basemap (no internet needed), and fail any test that
// logs an uncaught console error or leaves an error pop-up open (unless the test sets `errors.expected`).
import { expect, test as base, type Page } from "@playwright/test";

type Gis = Record<string, any>;
export const gis = <T,>(page: Page, fn: (g: Gis) => T | Promise<T>, arg?: unknown): Promise<T> =>
  page.evaluate(`(async () => { const g = window.__gis; return (${fn.toString()})(g, ${JSON.stringify(arg ?? null)}); })()`) as Promise<T>;

export const test = base.extend<{ app: Page; errors: { expected: boolean; known: RegExp[] } }>({
  /** Set `errors.expected = true` in a test that deliberately provokes an error pop-up. */
  /** `errors.known`: console errors of a bug already listed in docs/BUGS.md ("Found by e2e"); anything else still fails. */
  errors: async ({}, use) => { await use({ expected: false, known: [] }); },
  app: async ({ page, errors: errState }, use, testInfo) => {
    // The Layout tab needs about 2,300 px to show every group at full size; the specs that drive it get a wide window, the rest keep the
    // usual 1600 x 950 (their node and map positions depend on it). The ribbon's own width spec sets its own sizes.
    if (/layout|series|templates|labels/.test(testInfo.file)) await page.setViewportSize({ width: 2560, height: 1300 });
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
    page.on("console", (m) => {
      if (m.type() !== "error") return;
      const t = m.text();
      if (/Failed to load resource|net::ERR|favicon/.test(t)) return; // network noise (tiles, offline)
      if (errState.known.some((r) => r.test(t))) return;
      errors.push(`console.error: ${t}`);
    });
    await page.goto("/");
    await page.waitForFunction(() => !!(window as any).__gis);
    await gis(page, async (g) => { await g.api.newProject(); g.useApp.setState({ basemap: "none" }); }); // the devserver's engine outlives the page: start every test empty
    await use(page);
    // A test that stopped early (a timeout) must not leave a long job running on the shared devserver for the tests after it.
    await Promise.race([
      page.evaluate(async () => { const g = (window as any).__gis; for (const t of await g.api.progress()) await g.api.cancelTask(t.id); }).catch(() => {}),
      new Promise((r) => setTimeout(r, 5000)),
    ]);
    expect(errors, "uncaught console errors").toEqual([]);
    if (!errState.expected) await expect(page.getByRole("alertdialog"), "unexpected error pop-up").toHaveCount(0);
  },
});
export { expect };

/** Click a ribbon button by its label (switching tab first when given). */
export async function ribbon(page: Page, tab: string | null, label: string | RegExp) {
  const btn = page.locator(".rbtn", { hasText: label }).first();
  if (tab && tab !== "Layout") await page.locator(".rtab", { hasText: new RegExp(`^${tab}$`) }).click();
  else if (tab === "Layout" || (await page.locator(".rtab.ctx", { hasText: /^Elements$/ }).count()) > 0) {
    // The layout workspace splits its ribbon into Layout / Elements / Format / Export: find the tab that holds the button.
    for (const t of ["Layout", "Elements", "Format", "Export"]) {
      if (tab === null && t === "Layout" && (await btn.isVisible())) break;
      await page.locator(".rtab", { hasText: new RegExp(`^${t}$`) }).click();
      // The tab body renders a moment after the click: give the button a short chance before trying the next tab.
      if (await btn.waitFor({ timeout: 700 }).then(() => true, () => false)) break;
    }
  }
  await btn.click();
}

/** Generate a test layer through the engine (what Data → Generate Test Data does) and add it to the app. */
export async function addTestLayer(page: Page, kind: "points" | "polygons", count: number, bbox: number[] = [-100, 30, -90, 40]) {
  return gis(page, async (g, a: any) => {
    const info = await g.api.generateTestLayer(a.kind, a.count, a.bbox);
    g.useApp.getState().addLayers([info]);
    return info;
  }, { kind, count, bbox }) as Promise<any>;
}

/** Page coordinates of a lon/lat when the map CRS is geographic (EPSG:4326). */
export async function pixelOf(page: Page, lon: number, lat: number): Promise<[number, number]> {
  await page.waitForFunction(() => !(window as any).__gis.getMap().getView().getAnimating());
  return gis(page, (g, p: any) => {
    const m = g.getMap(); const r = m.getTargetElement().getBoundingClientRect();
    const px = m.getPixelFromCoordinate([p[0], p[1]]);
    return [r.left + px[0], r.top + px[1]];
  }, [lon, lat]) as Promise<[number, number]>;
}

/** A small layer with known points: two coincident at (-97,35), one at (-96.9,35.1), one at (-96.8,35.2). */
export async function addKnownPoints(page: Page) {
  return gis(page, async (g) => {
    const f = (x: number, y: number, name: string, pop: number) => ({ wkt: `POINT(${x} ${y})`, props: { name, pop } });
    const info = await g.api.createFeatures("Known", [f(-97, 35, "a", 10), f(-97, 35, "b", 30), f(-96.9, 35.1, "c", 25), f(-96.8, 35.2, "d", 5)]);
    g.useApp.getState().addLayers([info]);
    await g.actions.zoomToLayer(info.id);
    return info;
  }) as Promise<any>;
}

/** A click followed by a pause: OpenLayers treats two clicks within 250 ms as a double-click wherever they are. */
export async function click(page: Page, x: number, y: number) {
  await page.mouse.click(x, y);
  await page.waitForTimeout(350);
}

// ---- Model (node editor) helpers ----
export const canvas = (page: Page) => page.locator(".model-canvas");
export const hoverCanvas = async (page: Page, dx = 400, dy = 120) => {
  const b = (await canvas(page).boundingBox())!;
  await page.mouse.move(b.x + dx, b.y + dy);
};
export const modelState = (page: Page) =>
  gis(page, (g) => ({
    nodes: g.useModel.getState().nodes.map((n: any) => ({ id: n.id, tool: n.data.tool, muted: !!n.data.muted, collapsed: !!n.data.collapsed, selected: !!n.selected, x: n.position.x, y: n.position.y })),
    edges: g.useModel.getState().edges.map((e: any) => [e.source, e.target, e.targetHandle]),
  })) as Promise<{ nodes: { id: string; tool: string; muted: boolean; collapsed: boolean; selected: boolean; x: number; y: number }[]; edges: [string, string, string][] }>;
export const nodeOf = async (page: Page, tool: string, nth = 0) => (await modelState(page)).nodes.filter((n) => n.tool === tool)[nth];

/** Shift+A at the pointer, type the tool's name, Enter. Returns the new node's id. */
export async function addNode(page: Page, search: string, tool: string, dx = 400, dy = 120) {
  const before = (await modelState(page)).nodes.length;
  await hoverCanvas(page, dx, dy);
  await page.keyboard.press("Shift+A");
  await page.keyboard.type(search);
  await page.keyboard.press("Enter");
  await expect.poll(async () => (await modelState(page)).nodes.length).toBe(before + 1);
  return (await modelState(page)).nodes.filter((n) => n.tool === tool).pop()!.id;
}
export async function frameAll(page: Page) {
  await hoverCanvas(page);
  await page.keyboard.press("Home");
  await page.waitForTimeout(700);
}
export async function handle(page: Page, nodeId: string, kind: "source" | "target", nth = 0): Promise<[number, number]> {
  const bb = (await page.locator(`.react-flow__handle[data-nodeid="${nodeId}"].${kind}`).nth(nth).boundingBox())!;
  return [bb.x + bb.width / 2, bb.y + bb.height / 2];
}
export async function drag(page: Page, a: [number, number], b: [number, number]) {
  await page.mouse.move(a[0], a[1]);
  await page.mouse.down();
  await page.mouse.move((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, { steps: 5 });
  await page.mouse.move(b[0], b[1], { steps: 5 });
  await page.mouse.up();
}
/** Wire source node → target node's nth input by dragging between the two sockets. */
export async function connect(page: Page, from: string, to: string, input = 0) {
  await drag(page, await handle(page, from, "source"), await handle(page, to, "target", input));
}
