// WP9 control audit (docs/UI_REDESIGN_PLAN.md §7): click every ribbon button on every tab (contextual and layout tabs too), every menu item,
// the top-left cell, the quick-access buttons, the search box, the Insert rail, and the controls of each pane, in the real app on the dev server.
// A control FAILS when it does nothing observable (no change in the page text, control states, dock, stores, map view, requests, native-dialog
// request) or when it throws (page error / console error). Disabled controls are not clicked: they are listed per context, and a control that is
// disabled in every context fails too (nothing could ever use it).
//
// Native file dialogs cannot run in a browser. In a browser `ipc/dialogs` falls back to `window.prompt`; the audit answers every `prompt`
// with Cancel, `confirm` with OK and `alert` with OK, and records each in the "stubbed dialogs" list below. That is the same stub the other
// specs rely on; nothing is written to disk.
//
// Results: a table in the test output, and `audit.json` in the test's output folder (and E2E_AUDIT_JSON when set).
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Locator, Page } from "@playwright/test";
import { test, expect, gis } from "./fixtures";

type Result = "ok" | "menu" | "dead" | "threw" | "disabled" | "gone";
interface Rec { ctx: string; area: string; name: string; result: Result; changed?: string; note?: string }
const REPORT: Rec[] = [];
const STUBBED: { type: string; message: string; control: string }[] = [];
let currentControl = "";

/** A control that legitimately changes nothing in its context, with the reason. Everything else that changes nothing is a failure. */
const NO_CHANGE_OK: { match: RegExp; ctx?: RegExp; why: string }[] = [
  { match: /^Explore$/, why: "the Explore tool is already the active tool; clicking it again keeps it" },
  { match: /(^|▸ )Align (Left|Centre|Right)$/, ctx: /^layout-(frame|text|page|multi)$/, why: "with several selected the common box is used, and these elements share x 12.7 and width 254, so Left and Right change nothing (Centre moves by rounding). A single element aligns to the page inside its margins, and a new frame or text already spans margin to margin (x 12.7, w 254 on a 279.4 mm page), so aligning it horizontally changes nothing; Top, Middle and Bottom do move it" },
  { match: /^Distribute Horizontally$/, ctx: /^layout-multi$/, why: "the three selected elements share x 12.7 and width 254, so there is nothing to space out horizontally; Vertically moves them" },
  { match: /^Same Width as First$/, ctx: /^layout-multi$/, why: "the text, map frame and sources elements are all 254 mm wide already; Same Height moves them" },
  { match: /^Fit Page$/, why: "Fit Page is a toggle that is already on when the layout opens, and the page is already fitted" },
  { match: /EPSG:4326/, ctx: /^map-/, why: "the test map is already in WGS 84, so picking it in the Coordinate System list changes nothing" },
  { match: /^Attribute Table$/, why: "the Attribute Table is already the active tab of the bottom panel (the fixture opens it)" },
  { match: /^Map$/, why: "View > Windows > Map shows the Map pane, which is already the visible one" },
];

// ---------------------------------------------------------------- page plumbing

const CLICKABLE = [
  "button", "[role=button]", "[role=menuitem]", "[role=menuitemcheckbox]", "[role=menuitemradio]", "[role=tab]", "[role=checkbox]", "[role=switch]", "[role=radio]",
  "[role=combobox]", "select", "summary", "input[type=checkbox]", "input[type=radio]", "input[type=number]", "input[type=text]", "input:not([type])", "input[type=search]", "textarea",
  "[role=spinbutton]", ".row[data-element]", ".cat-row", ".tree-row", ".tree-layer", ".tree-result", ".rail-map", "[data-start]", "[role=option]",
].join(",");
/** Never exercised by the generic sweep: the dock's own tab close buttons (closing a pane breaks the scope; View › Windows closes and reopens each), resize sashes, the map canvas. */
const EXCLUDE = ".dv-default-tab-action, .dv-tab, .dv-sash, canvas, .react-flow__handle, .react-flow__controls, .react-flow__minimap, .react-flow__attribution, [data-audit-skip]";

async function installHooks(page: Page, errors: string[]) {
  page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() !== "error") return;
    const t = m.text();
    if (/Failed to load resource|net::ERR|favicon|ResizeObserver loop/.test(t)) return;
    errors.push(`console.error: ${t}`);
  });
  page.on("dialog", async (d) => {
    try {
      if (d.type() === "beforeunload") return void (await d.accept()); // the audit's own page reloads between contexts
      STUBBED.push({ type: d.type(), message: d.message().slice(0, 120), control: currentControl });
      if (d.type() === "prompt") await d.dismiss(); // a native file dialog in the app: the user cancels
      else await d.accept();
    } catch { /* already handled by Playwright's own default */ }
  });
  await page.addInitScript(() => {
    (window as any).__netCalls = [];
    const f = window.fetch.bind(window);
    window.fetch = (...a: Parameters<typeof fetch>) => { const u = String(typeof a[0] === "string" ? a[0] : (a[0] as Request).url ?? a[0]); if (!/progress|\/@vite|\/src\/|hot-update|tile/.test(u)) (window as any).__netCalls.push(u); return f(...a); };
  });
}

/** Everything a click could change, one string per area, so a diff says what changed. */
async function parts(page: Page): Promise<Record<string, string>> {
  const p = await partsInPage(page);
  return { ...p, nativeDialogs: String(STUBBED.length) }; // a request for a native file dialog (answered with Cancel) counts as the control doing something
}
async function partsInPage(page: Page): Promise<Record<string, string>> {
  return page.evaluate(async () => {
    const g = (window as any).__gis;
    const a = g.useApp.getState();
    const L = g.useLayouts.getState();
    const d = (await import("/src/panes/panes.ts")).getDock();
    const clone = document.body.cloneNode(true) as HTMLElement;
    // Open menus and lists are left out of everything below: choosing an item always closes its menu, which must not count as the item doing something.
    clone.querySelectorAll("[role=tooltip], [role=menu], [role=listbox], script, style, canvas, .ol-viewport, .react-flow__edges").forEach((e) => e.remove());
    const view = g.getMap?.()?.getView?.();
    const states = [...document.querySelectorAll(".rbtn, .qa-btn, .rtab, [aria-pressed], [aria-checked], [aria-selected], input[type=checkbox]")]
      .filter((e) => !e.closest("[role=menu], [role=listbox]"))
      .map((e) => `${e.className}|${e.getAttribute("aria-pressed")}${e.getAttribute("aria-checked")}${e.getAttribute("aria-selected")}${(e as HTMLInputElement).checked ?? ""}${(e as HTMLButtonElement).disabled ?? ""}`).join(",");
    return {
      text: clone.textContent ?? "",
      values: [...document.querySelectorAll("input, textarea, select")].filter((v) => !v.closest("[role=menu], [role=listbox]")).map((v) => (v as HTMLInputElement).value + ((v as HTMLInputElement).checked ? "!" : "")).join("|"),
      states,
      popups: String(document.querySelectorAll("[role=dialog],[role=alertdialog],.toolmenu").length),
      dock: (d?.panels ?? []).map((p) => `${p.id}${p.api.isActive ? "*" : ""}${p.group.api.width | 0}x${p.group.api.height | 0}`).join(","),
      app: JSON.stringify({ s: a.status, t: a.tool, al: a.activeLayer, sel: a.selection, ly: Object.keys(a.layers), st: a.styles, maps: a.maps, am: a.activeMapId, dark: a.dark, fps: a.showFps }),
      layout: JSON.stringify({ id: L.id, doc: L.doc, sel: L.selected }),
      model: JSON.stringify({ n: g.useModel.getState().nodes?.length, e: g.useModel.getState().edges?.length }),
      view: view ? `${view.getCenter()}|${view.getResolution()}` : "",
      net: String((window as any).__netCalls.filter((u: string) => /\/invoke\//.test(u) && !/progress|list_|_list|tile|extent|layer_info|get_|specs|describe/.test(u)).length),
      theme: `${document.documentElement.dataset.theme}|${document.documentElement.getAttribute("style")?.length ?? 0}`,
    };
  });
}
const diff = (a: Record<string, string>, b: Record<string, string>) => Object.keys(a).filter((k) => a[k] !== b[k] && !(k === "net" && false));

/** Close whatever is open: error pop-ups, dialogs, menus, the tool menu. */
async function settle(page: Page) {
  for (let i = 0; i < 6; i++) {
    const open = await page.locator("[role=dialog]:visible, [role=alertdialog]:visible, [role=menu]:visible, [role=listbox]:visible, .toolmenu:visible, .ctxmenu:visible").count();
    if (!open) break;
    const dlg = page.locator("[role=dialog]:visible, [role=alertdialog]:visible").last();
    if (await dlg.count()) {
      const b = dlg.getByRole("button", { name: /^(Cancel|Close|Dismiss|OK|Done|Not now)$/i }).first();
      if (await b.count()) { await b.click({ timeout: 2000 }).catch(() => undefined); continue; }
    }
    await page.keyboard.press("Escape");
    await page.waitForTimeout(80);
  }
}

// ---------------------------------------------------------------- contexts

type Ctx = "map-empty" | "map-layer" | "map-model" | "map-run" | "layout-multi" | "layout-page" | "layout-frame" | "layout-text";
const BBOX = [-100, 30, -90, 40];

async function hardReset(page: Page) {
  const t0 = Date.now();
  // Let a pending layout save finish first: a reload in the middle of one logs "could not be saved: Failed to fetch", which is the harness, not the app.
  await page.evaluate(async () => { const m = await import("/src/store/layout.ts"); await m.flushLayoutSaves(); }).catch(() => undefined);
  await page.goto("/");
  await page.waitForFunction(() => !!(window as any).__gis);
  await gis(page, async (g) => { await g.api.newProject(); g.useApp.setState({ basemap: "none" }); });
  await page.locator(".dv-tab").first().waitFor();
  await gis(page, async () => { const m = await import("/src/panes/panes.ts"); m.showPane("table"); m.showPane("model"); });
  await page.locator(".model-canvas").waitFor({ state: "visible" });
  process.stderr.write(`AUDIT-REBUILD ${Date.now() - t0} ms (${currentControl})\n`);
}

async function invariants(page: Page): Promise<string> {
  const ready = await page.waitForFunction(() => !!(window as any).__gis, undefined, { timeout: 4000 }).then(() => true, () => false);
  if (!ready) return "page not ready"; // a control reloaded or broke the page: the context is rebuilt
  return page.evaluate(() => {
    const g = (window as any).__gis;
    const a = g.useApp.getState();
    const L = g.useLayouts.getState();
    return JSON.stringify({ layers: Object.keys(a.layers).length, maps: a.maps.length, layout: !!L.id, layouts: L.list().length, back: !!document.querySelector(".rtab-back"), active: !!a.activeLayer });
  });
}

const WANT: Record<Ctx, string> = {
  "map-empty": JSON.stringify({ layers: 0, maps: 1, layout: false, layouts: 0, back: false, active: false }),
  "map-layer": JSON.stringify({ layers: 2, maps: 1, layout: false, layouts: 0, back: false, active: true }),
  "map-model": JSON.stringify({ layers: 2, maps: 1, layout: false, layouts: 0, back: false, active: true }),
  "map-run": JSON.stringify({ layers: 1, maps: 1, layout: false, layouts: 0, back: false, active: true }),
  "layout-multi": JSON.stringify({ layers: 2, maps: 1, layout: true, layouts: 1, back: true, active: true }),
  "layout-page": JSON.stringify({ layers: 2, maps: 1, layout: true, layouts: 1, back: true, active: true }),
  "layout-frame": JSON.stringify({ layers: 2, maps: 1, layout: true, layouts: 1, back: true, active: true }),
  "layout-text": JSON.stringify({ layers: 2, maps: 1, layout: true, layouts: 1, back: true, active: true }),
};

async function build(page: Page, ctx: Ctx) {
  await settle(page);
  await hardReset(page);
  if (ctx === "map-empty") return;
  if (ctx === "map-run") {
    // A real import (so there is a citation for the Sources pane), a Buffer node, and a finished run (so Preview on map and Save as new layer are live).
    const file = join(mkdtempSync(join(tmpdir(), "gis-audit-")), "pts.geojson");
    writeFileSync(file, JSON.stringify({ type: "FeatureCollection", features: [[-97, 32], [-96, 33], [-95, 31]].map(([x, y], i) => ({ type: "Feature", properties: { id: i, name: `p${i}` }, geometry: { type: "Point", coordinates: [x, y] } })) }));
    const id = await gis(page, async (g, f: string) => { const infos = await g.api.importFiles([f]); g.useApp.getState().addLayers(infos); g.useApp.getState().setActiveLayer(infos[0].id); g.useApp.getState().setSelection(infos[0].id, [0, 1]); return infos[0].id; }, file);
    void id;
    const box = (await page.locator(".model-canvas").boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.keyboard.press("Shift+A");
    await page.keyboard.type("buffer");
    await page.keyboard.press("Enter");
    await page.waitForFunction(() => (window as any).__gis.useModel.getState().nodes.length >= 3, undefined, { timeout: 8000 }).catch(() => undefined);
    await page.locator(".model-run").click();
    await page.waitForFunction(() => Object.keys((window as any).__gis.useRun.getState().results).length > 0 && !(window as any).__gis.useRun.getState().running, undefined, { timeout: 30000 }).catch(() => undefined);
    return;
  }
  const first = await gis(page, async (g, bbox: number[]) => {
    const a = await g.api.generateTestLayer("polygons", 8, bbox);
    const b = await g.api.generateTestLayer("points", 8, bbox);
    g.useApp.getState().addLayers([a, b]);
    g.useApp.getState().setActiveLayer(a.id);
    g.useApp.getState().setSelection(a.id, [0, 1, 2]);
    return a.id;
  }, BBOX);
  void first;
  if (ctx === "map-layer") return;
  if (ctx === "map-model") {
    // A Buffer node on the active layer's tree, so the node cards' own controls exist.
    const box = (await page.locator(".model-canvas").boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.keyboard.press("Shift+A");
    await page.keyboard.type("buffer");
    await page.keyboard.press("Enter");
    await page.waitForFunction(() => (window as any).__gis.useModel.getState().nodes.length >= 3, undefined, { timeout: 8000 }).catch(() => undefined);
    return;
  }
  await gis(page, async (g) => { g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }); const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
  await page.locator(".layout-page").waitFor();
  await page.getByTestId("start-gallery").locator('[data-start="blank"]').click();
  await page.locator(".rail-btn", { hasText: /^Text$/ }).click();
  await page.locator(".rail-btn", { hasText: /^Legend$/ }).click().catch(() => undefined);
  await page.locator(".rail-btn", { hasText: /^Map Frame/ }).click();
  const menu = page.getByRole("menuitem").first();
  if (await menu.isVisible().catch(() => false)) await menu.click();
  await selectKind(page, ctx === "layout-text" ? "text" : ctx === "layout-frame" ? "map_frame" : ctx === "layout-multi" ? "ALL" : null);
}

async function selectKind(page: Page, kind: string | null) {
  await gis(page, (g, k: string | null) => {
    const L = g.useLayouts.getState();
    if (k === "ALL") return L.select(L.doc.elements.map((e: any) => e.id));
    const el = k ? L.doc.elements.find((e: any) => e.kind === k) : null;
    L.select(el ? [el.id] : []);
  }, kind);
}

async function ensure(page: Page, ctx: Ctx) {
  await ensureContext(page, ctx);
  // The status line starts each control from the same text, so a refusal that repeats the previous message still shows as "said something".
  await gis(page, (g) => g.useApp.getState().setStatus("Ready")).catch(() => undefined);
}
async function ensureContext(page: Page, ctx: Ctx) {
  if ((await invariants(page)) === WANT[ctx]) {
    if (ctx.startsWith("layout")) await selectKind(page, ctx === "layout-text" ? "text" : ctx === "layout-frame" ? "map_frame" : ctx === "layout-multi" ? "ALL" : null);
    return;
  }
  await build(page, ctx);
}

// ---------------------------------------------------------------- the crawler

interface Ctl { index: number; name: string; key: string; kind: string }

/** Visible, not-excluded controls inside `scope`, with a stable name and a key that tells equal names apart. */
async function listControls(page: Page, scope: string): Promise<Ctl[]> {
  return page.evaluate(([scopeSel, sel, excl]) => {
    const root = document.querySelector(scopeSel);
    if (!root) return [];
    const seen: Record<string, number> = {};
    const out: { index: number; name: string; key: string; kind: string }[] = [];
    [...root.querySelectorAll<HTMLElement>(sel)].forEach((el) => {
      const r = el.getBoundingClientRect();
      const cs = getComputedStyle(el);
      if (r.width < 2 || r.height < 2 || cs.visibility === "hidden" || cs.display === "none" || el.closest(excl) || el.matches(excl)) return;
      if (el.closest("[aria-hidden=true]")) return;
      const name = (el.getAttribute("aria-label") || el.textContent || el.getAttribute("title") || el.getAttribute("placeholder") || el.className || el.tagName).replace(/\s+/g, " ").trim().slice(0, 70);
      seen[name] = (seen[name] ?? 0) + 1;
      out.push({ index: out.length, name, key: `${name}#${seen[name]}`, kind: el.getAttribute("role") || (el as HTMLInputElement).type || el.tagName.toLowerCase() });
    });
    return out;
  }, [scope, CLICKABLE, EXCLUDE] as const);
}

/** Tag the control with this key (so it can be clicked by locator); false if it is gone. */
async function tag(page: Page, scope: string, key: string): Promise<boolean> {
  return page.evaluate(([scopeSel, sel, excl, k]) => {
    document.querySelectorAll("[data-audit-target]").forEach((e) => e.removeAttribute("data-audit-target"));
    const root = document.querySelector(scopeSel);
    if (!root) return false;
    const seen: Record<string, number> = {};
    for (const el of [...root.querySelectorAll<HTMLElement>(sel)]) {
      const r = el.getBoundingClientRect();
      const cs = getComputedStyle(el);
      if (r.width < 2 || r.height < 2 || cs.visibility === "hidden" || cs.display === "none" || el.closest(excl) || el.matches(excl) || el.closest("[aria-hidden=true]")) continue;
      const name = (el.getAttribute("aria-label") || el.textContent || el.getAttribute("title") || el.getAttribute("placeholder") || el.className || el.tagName).replace(/\s+/g, " ").trim().slice(0, 70);
      seen[name] = (seen[name] ?? 0) + 1;
      if (`${name}#${seen[name]}` === k) { el.setAttribute("data-audit-target", "1"); return true; }
    }
    return false;
  }, [scope, CLICKABLE, EXCLUDE, key] as const);
}

const target = (page: Page): Locator => page.locator("[data-audit-target]");
const TEXTY = "input[type=number], input[type=text], input:not([type]), input[type=search], textarea, [role=spinbutton]";

/** Perform the control's action; returns what changed. Text fields get a changed value; everything else is clicked. */
async function act(page: Page, loc: Locator, kind: string): Promise<void> {
  const isText = await loc.evaluate((e, s) => (e as HTMLElement).matches(s), TEXTY);
  if (isText) {
    const cur = await loc.inputValue().catch(() => "");
    const num = kind === "number" || kind === "spinbutton";
    await loc.fill(num ? String((Number(cur) || 0) + 1) : `${cur}1`, { timeout: 3000 });
    await page.keyboard.press("Tab");
  } else if (await loc.evaluate((e) => (e as HTMLElement).tagName === "SELECT")) {
    const opts = await loc.locator("option").evaluateAll((os) => os.map((o) => (o as HTMLOptionElement).value));
    const cur = await loc.inputValue();
    const next = opts.find((o) => o !== cur);
    if (next !== undefined) await loc.selectOption(next, { timeout: 3000 });
  } else await loc.click({ timeout: 4000 });
}

async function effect(page: Page, before: Record<string, string>, errors: string[], errs0: number): Promise<{ changed: string[]; threw: string[] }> {
  const deadline = Date.now() + 1800;
  let changed: string[] = [];
  do {
    await page.waitForTimeout(140);
    changed = diff(before, await parts(page));
  } while (!changed.length && Date.now() < deadline && errors.length === errs0);
  return { changed, threw: errors.slice(errs0) };
}

const allowed = (name: string, ctx: string) => NO_CHANGE_OK.find((a) => a.match.test(name) && (!a.ctx || a.ctx.test(ctx)));

function record(r: Rec) {
  REPORT.push(r);
}

interface Env { page: Page; errors: string[]; ctx: Ctx; /** Start every control from a freshly built context (panes keep state, like an expanded tree or a picked table, that the invariants cannot see). */ fresh?: boolean }

const stalled = Symbol("stalled");
const within = <T,>(p: Promise<T>, ms: number): Promise<T | typeof stalled> => Promise.race([p, new Promise<typeof stalled>((r) => setTimeout(() => r(stalled), ms))]);

/** Exercise one control. A control after which the page stops answering for 60 s is recorded as "threw" (unresponsive) and the page is reloaded. */
async function exerciseControl(env: Env, area: string, scope: string, c: Ctl, reveal: () => Promise<void>) {
  const r = await within(exerciseControlInner(env, area, scope, c, reveal), 300_000);
  if (r === stalled) {
    record({ ctx: env.ctx, area, name: c.name, result: "threw", note: "the page stopped answering for 300 s after this control" });
    await env.page.reload({ timeout: 30_000 }).catch(() => undefined);
    await build(env.page, env.ctx);
  }
}

/** Exercise one control. `reveal` makes the control reachable (selects the ribbon tab, activates the pane). */
async function exerciseControlInner(env: Env, area: string, scope: string, c: Ctl, reveal: () => Promise<void>) {
  const { page, errors, ctx } = env;
  currentControl = `${ctx} › ${area} › ${c.name}`;
  process.stderr.write(`AUDIT ${currentControl}\n`);
  await settle(page);
  if (env.fresh) { await build(page, ctx); await gis(page, (g) => g.useApp.getState().setStatus("Ready")); }
  else await ensure(page, ctx);
  await reveal();
  if (!(await tag(page, scope, c.key))) {
    const now = await listControls(page, scope);
    return record({ ctx, area, name: c.name, result: "gone", note: `scope has ${now.length} controls now; key ${c.key}; first: ${now.slice(0, 3).map((x) => x.key).join(" / ")}` });
  }
  const loc = target(page);
  if (await loc.isDisabled().catch(() => false)) return record({ ctx, area, name: c.name, result: "disabled" });
  const before = await parts(page);
  const errs0 = errors.length;
  try {
    await act(page, loc, c.kind);
  } catch (e) {
    const msg = String(e).split("\n")[0];
    // Could not click (covered or detached after the page reacted): the page reacted, so look at what changed.
    if (!/Timeout|intercepts|detached|not visible/.test(msg)) return record({ ctx, area, name: c.name, result: "threw", note: msg });
  }
  const { changed, threw } = await effect(page, before, errors, errs0);
  if (threw.length) return record({ ctx, area, name: c.name, result: "threw", note: threw.join(" | ").slice(0, 300) });
  const opened = await page.locator("[role=menu]:visible, [role=listbox]:visible").count();
  if (opened > 0 && c.kind !== "option" && !c.kind.startsWith("menuitem")) {
    record({ ctx, area, name: c.name, result: "menu", changed: changed.join(",") });
    await exerciseMenu(env, area, scope, c, reveal);
    return;
  }
  if (changed.length) return record({ ctx, area, name: c.name, result: "ok", changed: changed.join(",") });
  const ok = allowed(c.name, ctx);
  if (ok) return record({ ctx, area, name: c.name, result: "ok", changed: "-", note: `no change allowed: ${ok.why}` });
  record({ ctx, area, name: c.name, result: "dead" });
}

/** The menu a control opened: choose each item (and each submenu's items) in turn, reopening the menu for each. */
async function exerciseMenu(env: Env, area: string, scope: string, c: Ctl, reveal: () => Promise<void>, depth = 0, via: number[] = []) {
  const { page, errors, ctx } = env;
  const ITEM = "[role=menuitem], [role=menuitemcheckbox], [role=menuitemradio], [role=option]";
  const menuLoc = () => page.locator("[role=menu]:visible, [role=listbox]:visible").last();
  const names = (await menuLoc().locator(ITEM).evaluateAll((els) => els.map((e) => (e.textContent ?? "").replace(/\s+/g, " ").trim().slice(0, 60)))).slice(0, 14);
  await settle(page);
  for (let k = 0; k < names.length; k++) {
    const label = `${c.name} ▸ ${[...via.map(String), k].length > 1 ? "…▸ " : ""}${names[k] || `item ${k + 1}`}`;
    currentControl = `${ctx} › ${area} › ${label}`;
    await settle(page);
    if (env.fresh) { await build(page, ctx); await gis(page, (g) => g.useApp.getState().setStatus("Ready")); }
    else await ensure(page, ctx);
    await reveal();
    // reopen the chain
    if (!(await tag(page, scope, c.key))) return record({ ctx, area, name: label, result: "gone" });
    await target(page).click({ timeout: 4000 }).catch(() => undefined);
    for (const v of via) {
      await page.waitForTimeout(120);
      await menuLoc().locator(ITEM).nth(v).click({ timeout: 3000 }).catch(() => undefined);
    }
    await page.waitForTimeout(160);
    const item = menuLoc().locator(ITEM).nth(k);
    if (!(await item.count())) { record({ ctx, area, name: label, result: "gone" }); continue; }
    if (await item.isDisabled().catch(() => false) || (await item.getAttribute("aria-disabled")) === "true") { record({ ctx, area, name: label, result: "disabled" }); await settle(page); continue; }
    const hasSub = (await item.getAttribute("aria-haspopup")) !== null;
    const before = await parts(page);
    const errs0 = errors.length;
    const menusBefore = await page.locator("[role=menu]:visible").count();
    await item.click({ timeout: 4000 }).catch(() => undefined);
    const { changed, threw } = await effect(page, before, errors, errs0);
    if (threw.length) { record({ ctx, area, name: label, result: "threw", note: threw.join(" | ").slice(0, 300) }); continue; }
    if (hasSub && depth < 2 && (await page.locator("[role=menu]:visible").count()) > menusBefore) {
      record({ ctx, area, name: label, result: "menu" });
      await exerciseMenu(env, area, scope, c, reveal, depth + 1, [...via, k]);
      continue;
    }
    if (changed.length) record({ ctx, area, name: label, result: "ok", changed: changed.join(",") });
    else if (allowed(names[k], ctx)) record({ ctx, area, name: label, result: "ok", changed: "-", note: "allowed" });
    else record({ ctx, area, name: label, result: "dead" });
  }
  await settle(page);
}

async function sweep(env: Env, area: string, scope: string, reveal: () => Promise<void>, skip?: RegExp, only?: RegExp) {
  await reveal();
  const list = await listControls(env.page, scope);
  for (const c of list) {
    if (skip?.test(c.name) || (only && !only.test(c.name))) continue;
    await exerciseControl(env, area, scope, c, reveal);
  }
}

async function ribbonTabs(page: Page): Promise<string[]> {
  return page.locator(".ribbon-tabs [role=tab]").evaluateAll((els) => els.map((e) => (e.textContent ?? "").trim()));
}
const gotoTab = (page: Page, name: string) => async () => { await settle(page); await page.locator(".ribbon-tabs [role=tab]", { hasText: new RegExp(`^${name}$`) }).first().click(); await page.waitForTimeout(120); };

/** Every tab of the ribbon in this context: each tab itself, then each control of its band. */
async function ribbonSweep(env: Env) {
  const { page, ctx } = env;
  await ensure(page, ctx);
  const tabs = await ribbonTabs(page);
  for (const t of tabs) {
    const reveal = gotoTab(page, t);
    // the tab button itself
    currentControl = `${ctx} › tab ${t}`;
    await reveal();
    record({ ctx, area: "tabs", name: `tab ${t}`, result: (await page.locator(".ribbon-tabs [role=tab][aria-selected=true]").first().textContent())?.trim() === t ? "ok" : "dead" });
    await sweep(env, `ribbon ▸ ${t}`, ".ribbon-body", reveal);
  }
}

// ---------------------------------------------------------------- the tests

test.describe.configure({ mode: "serial" });
test.beforeEach(async ({ app, errors }) => {
  // The audit reloads the page between contexts (to start each from a known state); a layout save that a reload cuts off logs this. Not an app error.
  errors.known.push(/Layout '.*' could not be saved: Failed to fetch/);
  await app.setViewportSize({ width: 2560, height: 1300 });
});
test.afterEach(async ({}, testInfo) => {
  writeFileSync(`${process.env.E2E_AUDIT_PARTIAL ?? "/tmp/audit-partial"}-${testInfo.title.replace(/\W+/g, "_").slice(0, 40)}.json`, JSON.stringify({ report: REPORT, stubbed: STUBBED }));
});

async function mk(app: Page, ctx: Ctx, fresh = false): Promise<Env> {
  const errors: string[] = [];
  app.setDefaultTimeout(6000);
  await installHooks(app, errors);
  await build(app, ctx);
  return { page: app, errors, ctx, fresh };
}

const REPORT_FAIL = () => REPORT.filter((r) => r.result === "dead" || r.result === "threw");

test("audit: map ribbon with an empty project", async ({ app }) => {
  test.setTimeout(900_000);
  const env = await mk(app, "map-empty");
  await ribbonSweep(env);
});

test("audit: map ribbon with layers, a selection and an active layer (Feature Layer tab included)", async ({ app }) => {
  test.setTimeout(1_500_000);
  const env = await mk(app, "map-layer");
  await ribbonSweep(env);
  // top-left cell, quick access and the search box
  await sweep(env, "top-left cell", ".proj-box", async () => undefined);
  await sweep(env, "quick access", ".qa", async () => undefined);
  await sweep(env, "search", ".ribbon-tabs .search, .ribbon-tabs [role=search], .ribbon-tabs input[type=search], .ribbon-tabs :has(> input[placeholder*=Search])", async () => undefined);
});

test("audit: layout ribbon, Insert rail and layout surfaces (page, frame, text selected)", async ({ app }) => {
  test.setTimeout(1_800_000);
  for (const ctx of ["layout-page", "layout-frame", "layout-text"] as Ctx[]) {
    const env = await mk(app, ctx);
    await ribbonSweep(env);
    await sweep(env, "Insert rail", ".rail", async () => undefined);
    await sweep(env, "layout toolbar and context bar", ".layout-view", async () => undefined, /^(Page|Map|Layout \d)$/);
  }
});

test("audit: panes (Map Layers, Catalog, Properties, Sources, Items, Attribute Table, Node Editor), map chrome and status bar", async ({ app }) => {
  test.setTimeout(2_400_000);
  const env = await mk(app, "map-model", true);
  const pane = (id: string) => async () => {
    await app.evaluate(async (pid) => {
      const p = (await import("/src/panes/panes.ts")).getDock()!.getPanel(pid);
      p?.api.setActive();
      (p?.view as any)?.content?.element?.setAttribute("data-audit-pane", pid);
    }, id);
    await app.waitForTimeout(150);
  };
  for (const id of ["contents", "catalog", "properties", "sources", "table", "model"]) {
    await sweep(env, `pane ▸ ${id}`, `[data-audit-pane="${id}"]`, pane(id));
  }
  await sweep(env, "map chrome", ".map-wrap", async () => undefined);
  await sweep(env, "status bar", ".statusbar", async () => undefined);
});

test("audit: layout panes (Items, Properties) with an element selected, and the right-click menus", async ({ app }) => {
  test.setTimeout(1_200_000);
  const env = await mk(app, "layout-text", true);
  const pane = (id: string) => async () => {
    await app.evaluate(async (pid) => {
      const p = (await import("/src/panes/panes.ts")).getDock()!.getPanel(pid);
      (p?.view as any)?.content?.element?.setAttribute("data-audit-pane", pid);
    }, id);
  };
  await sweep(env, "layout pane ▸ items", `[data-audit-pane="items"]`, pane("items"));
  await sweep(env, "layout pane ▸ properties", `[data-audit-pane="properties"]`, pane("properties"));
  // right-click menus: an empty spot of the page, then an element (submenus included)
  await rightClickAudit(env, "layout-text", "page", ".layout-page");
  await rightClickAudit(env, "layout-text", "element", ".layout-el");
});

/** Open the context menu at `selector`, and choose each item (and each submenu item) from a freshly built context. */
async function rightClickAudit(env: Env, ctx: Ctx, where: string, selector: string, only?: RegExp, prep?: () => Promise<void>) {
  const app = env.page;
  const open = async () => {
    await build(app, ctx);
    if (prep) await prep();
    const box = (await app.locator(selector).first().boundingBox())!;
    await app.mouse.click(box.x + (where === "page" ? 12 : box.width / 2), box.y + (where === "page" ? 12 : box.height / 2), { button: "right" });
    await app.waitForTimeout(150);
  };
  const labelsOf = () => app.locator("[role=menu]:visible").last().getByRole("menuitem").evaluateAll((els) => els.map((e) => (e.textContent ?? "").replace(/\s+/g, " ").trim()));
  await open();
  const labels = await labelsOf();
  await settle(app);
  for (let k = 0; k < labels.length; k++) {
    if (only && !only.test(labels[k])) continue;
    const area = `right-click ${where}`;
    currentControl = `${ctx} › ${area} › ${labels[k]}`;
    await open();
    const item = app.locator("[role=menu]:visible").last().getByRole("menuitem").nth(k);
    if (await item.isDisabled().catch(() => false)) { record({ ctx, area, name: labels[k], result: "disabled" }); await settle(app); continue; }
    const before = await parts(app);
    const errs0 = env.errors.length;
    const hasSub = (await item.getAttribute("aria-haspopup")) !== null;
    await item.click({ timeout: 3000 }).catch(() => undefined);
    const { changed, threw } = await effect(app, before, env.errors, errs0);
    if (threw.length) { record({ ctx, area, name: labels[k], result: "threw", note: threw.join("|").slice(0, 200) }); await settle(app); continue; }
    if (hasSub && (await app.locator("[role=menu]:visible").count()) > 1) {
      record({ ctx, area, name: labels[k], result: "menu" });
      const subs = await labelsOf();
      await settle(app);
      for (let j = 0; j < subs.length; j++) {
        currentControl = `${ctx} › ${area} › ${labels[k]} ▸ ${subs[j]}`;
        await open();
        await app.locator("[role=menu]:visible").last().getByRole("menuitem").nth(k).click({ timeout: 3000 }).catch(() => undefined);
        await app.waitForTimeout(200);
        const sub = app.locator("[role=menu]:visible").last().getByRole("menuitem").nth(j);
        if (await sub.isDisabled().catch(() => false)) { record({ ctx, area, name: `${labels[k]} ▸ ${subs[j]}`, result: "disabled" }); await settle(app); continue; }
        const b2 = await parts(app);
        const e2 = env.errors.length;
        await sub.click({ timeout: 3000 }).catch(() => undefined);
        const r2 = await effect(app, b2, env.errors, e2);
        const nm = `${labels[k]} ▸ ${subs[j]}`;
        record({ ctx, area, name: nm, result: r2.threw.length ? "threw" : r2.changed.length || allowed(nm, ctx) ? "ok" : "dead", changed: r2.changed.join(",") || "-", note: r2.threw.join("|").slice(0, 200) || (r2.changed.length ? undefined : allowed(nm, ctx)?.why) });
        await settle(app);
      }
      continue;
    }
    record({ ctx, area, name: labels[k], result: changed.length ? "ok" : "dead", changed: changed.join(",") });
    await settle(app);
  }
}

test("audit: controls that need more state (several items selected, a finished run, history, citations)", async ({ app }) => {
  test.setTimeout(1_500_000);
  // Several elements selected: Distribute, Same Size, Group, Ungroup, Lock; Items pane Group/Ungroup; right-click Group; Paste after a copy.
  const multi = await mk(app, "layout-multi", true);
  await sweep(multi, "ribbon ▸ Arrange", ".ribbon-body", async () => { await settle(app); await app.locator(".ribbon-tabs [role=tab]", { hasText: /^Arrange$/ }).click(); await app.waitForTimeout(120); }, undefined, /Distribute|Same Size|Group|Ungroup|Lock|Align|Order/);
  const itemsPane = async () => { await app.evaluate(async () => { const p = (await import("/src/panes/panes.ts")).getDock()!.getPanel("items"); (p?.view as any)?.content?.element?.setAttribute("data-audit-pane", "items"); }); };
  await sweep(multi, "layout pane ▸ items", `[data-audit-pane="items"]`, itemsPane, undefined, /^(Group|Ungroup)$/);
  await rightClickAudit(multi, "layout-multi", "element", ".layout-el", /Group|Align|Order/);
  await rightClickAudit(multi, "layout-multi", "page", ".layout-page", /Paste/, async () => { await app.locator(".layout-canvas").focus(); await app.keyboard.press("ControlOrMeta+c"); });
  // Ungroup is live only for a group: group the selection, then ungroup it, from the ribbon and from the Items pane.
  for (const where of ["ribbon ▸ Arrange", "layout pane ▸ items"]) {
    currentControl = `layout-multi › ${where} › Ungroup`;
    await build(app, "layout-multi");
    await gis(app, (g) => g.useLayouts.getState().group());
    if (where.startsWith("ribbon")) await app.locator(".ribbon-tabs [role=tab]", { hasText: /^Arrange$/ }).click();
    else await itemsPane();
    const btn = where.startsWith("ribbon") ? app.locator(".ribbon-body .rbtn", { hasText: /^Ungroup$/ }) : app.locator('[data-audit-pane="items"] button', { hasText: /^Ungroup$/ });
    if (await btn.isDisabled()) { record({ ctx: "layout-multi", area: where, name: "Ungroup", result: "disabled" }); continue; }
    const before = await parts(app);
    const errs0 = multi.errors.length;
    await btn.click();
    const r = await effect(app, before, multi.errors, errs0);
    record({ ctx: "layout-multi", area: where, name: "Ungroup (after Group)", result: r.threw.length ? "threw" : r.changed.length ? "ok" : "dead", changed: r.changed.join(",") });
  }
  // A finished run on an imported point layer: Export Table, Preview on map, Save as new layer, Sources Copy / Export.
  const run = await mk(app, "map-run", true);
  const tab = (name: string) => async () => { await settle(app); await app.locator(".ribbon-tabs [role=tab]", { hasText: new RegExp(`^${name}$`) }).first().click(); await app.waitForTimeout(120); };
  await sweep(run, "ribbon ▸ Feature Layer", ".ribbon-body", tab("Feature Layer"), undefined, /^Export Table$/);
  const pane = (id: string) => async () => { await app.evaluate(async (pid) => { const p = (await import("/src/panes/panes.ts")).getDock()!.getPanel(pid); p?.api.setActive(); (p?.view as any)?.content?.element?.setAttribute("data-audit-pane", pid); }, id); await app.waitForTimeout(150); };
  await sweep(run, "pane ▸ model", `[data-audit-pane="model"]`, pane("model"), undefined, /Preview on map|Save as new layer|Redo/);
  await sweep(run, "pane ▸ properties", `[data-audit-pane="properties"]`, pane("properties"), undefined, /Preview on map/);
  await sweep(run, "pane ▸ sources", `[data-audit-pane="sources"]`, pane("sources"), undefined, /^(Export|Copy all citations)/);
  // History: Undo and Redo after an edit to the node tree.
  const hist = await mk(app, "map-model", true);
  for (const which of ["Undo", "Redo"]) {
    currentControl = `map-model › quick access › ${which}`;
    await build(app, "map-model");
    if (which === "Redo") await app.locator('.qa-btn[data-qa="undo"]').click();
    const btn = app.locator(`.qa-btn[data-qa="${which.toLowerCase()}"]`);
    if (await btn.isDisabled()) { record({ ctx: "map-model", area: "quick access", name: which, result: "disabled" }); continue; }
    const before = await parts(app);
    const errs0 = hist.errors.length;
    await btn.click();
    const r = await effect(app, before, hist.errors, errs0);
    record({ ctx: "map-model", area: "quick access", name: which, result: r.threw.length ? "threw" : r.changed.length ? "ok" : "dead", changed: r.changed.join(",") });
  }
});

test("audit: totals; no control is dead, throws, or stays disabled in every context", async ({}, testInfo) => {
  const total = REPORT.length;
  if (!total) test.skip(true, "run the whole file: this test summarises the others");
  const by = (r: Result) => REPORT.filter((x) => x.result === r).length;
  // Disabled everywhere: a name that never had a result other than disabled/gone.
  const names = new Map<string, Rec[]>();
  for (const r of REPORT) names.set(`${r.area}|${r.name}`, [...(names.get(`${r.area}|${r.name}`) ?? []), r]);
  const neverUsable = [...names.entries()].filter(([, rs]) => rs.every((r) => r.result === "disabled")).map(([k]) => k);
  const notExercised = REPORT.filter((r) => r.result === "gone").map((r) => `${r.ctx} › ${r.area} › ${r.name}`);
  const stoppedAtDialog = [...new Set(STUBBED.filter((d) => d.type === "prompt").map((d) => d.control))];
  const summary = { notExercised, stoppedAtDialog, clicked: total - by("disabled") - by("gone"), ok: by("ok"), menus: by("menu"), dead: by("dead"), threw: by("threw"), gone: by("gone"), disabledInContext: by("disabled"), neverUsable, stubbedDialogs: STUBBED };
  console.log("AUDIT TOTALS", JSON.stringify({ ...summary, stubbedDialogs: STUBBED.length }));
  const out = JSON.stringify({ summary, report: REPORT }, null, 1);
  writeFileSync(testInfo.outputPath("audit.json"), out);
  if (process.env.E2E_AUDIT_JSON) writeFileSync(process.env.E2E_AUDIT_JSON, out);
  expect(REPORT_FAIL().map((r) => `${r.ctx} › ${r.area} › ${r.name}: ${r.result}${r.note ? ` (${r.note})` : ""}`), "controls that do nothing or throw").toEqual([]);
  expect(neverUsable, "controls disabled in every context the audit tried").toEqual([]);
});
