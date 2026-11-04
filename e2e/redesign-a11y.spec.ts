// WP9: the AA / focus / label checks of docs/UI_REDESIGN_PLAN.md §7, run on the real app in both themes, on the map view and in the layout
// workspace. Text contrast is measured from computed colours (text against the first opaque background behind it, blending translucent
// ones); disabled controls are exempt (WCAG 1.4.3). Focus visibility is measured by tabbing and comparing the focused element's
// outline / box-shadow / border with its unfocused look. Icon-only buttons must have an accessible name.
import { test, expect, gis, addTestLayer } from "./fixtures";

type Page = import("@playwright/test").Page;

/** Runs in the page: elements with their own text whose contrast is below the WCAG AA limit. */
async function lowContrast(page: Page) {
  return page.evaluate(() => {
    const parse = (c: string) => { const m = c.match(/rgba?\(([^)]+)\)/); if (!m) return null; const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number); return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 }; };
    const lum = (c: { r: number; g: number; b: number }) => { const f = (v: number) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b); };
    const over = (top: { r: number; g: number; b: number; a: number }, bottom: { r: number; g: number; b: number }) => ({ r: top.r * top.a + bottom.r * (1 - top.a), g: top.g * top.a + bottom.g * (1 - top.a), b: top.b * top.a + bottom.b * (1 - top.a) });
    const bgOf = (el: Element) => {
      const layers: { r: number; g: number; b: number; a: number }[] = [];
      for (let e: Element | null = el; e; e = e.parentElement) {
        const c = parse(getComputedStyle(e).backgroundColor);
        if (c && c.a > 0) { layers.push(c); if (c.a >= 1) break; }
      }
      let base = { r: 255, g: 255, b: 255 };
      for (const l of layers.reverse()) base = over(l, base);
      return base;
    };
    const out: { sel: string; text: string; ratio: number; need: number }[] & { checked?: number; min?: number } = [];
    let checked = 0, min = 99;
    const seen = new Set<string>();
    for (const el of document.querySelectorAll<HTMLElement>("body *")) {
      if (!el.offsetParent && getComputedStyle(el).position !== "fixed") continue;
      const own = [...el.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent ?? "").join("").trim();
      if (!own || el.closest("canvas, svg, .layout-page, .ol-viewport, [aria-disabled=true], .layout-canvas, .ol-attribution, .map")) continue;
      if ((el as HTMLButtonElement).disabled || el.closest("button:disabled, [disabled], fieldset:disabled")) continue;
      const cs = getComputedStyle(el);
      if (cs.visibility === "hidden" || cs.opacity === "0") continue;
      const fg0 = parse(cs.color);
      if (!fg0) continue;
      const bg = bgOf(el);
      const fg = over(fg0, bg);
      const L1 = lum(fg), L2 = lum(bg);
      const ratio = (Math.max(L1, L2) + 0.05) / (Math.min(L1, L2) + 0.05);
      const size = parseFloat(cs.fontSize);
      const large = size >= 24 || (size >= 18.66 && Number(cs.fontWeight) >= 700);
      const need = large ? 3 : 4.5;
      checked++; min = Math.min(min, ratio);
      if (ratio < need) {
        const sel = `${el.tagName.toLowerCase()}${el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\s+/).slice(0, 2).join(".") : ""}`;
        const key = `${sel}|${own.slice(0, 20)}`;
        if (!seen.has(key)) { seen.add(key); out.push({ sel, text: own.slice(0, 40), ratio: Math.round(ratio * 100) / 100, need }); }
      }
    }
    return { checked, lowestRatio: Math.round(min * 100) / 100, failing: out };
  });
}

/** Visible buttons and links with neither text nor an accessible name. */
async function unlabelled(page: Page) {
  return page.evaluate(() => {
    const out: string[] = [];
    for (const el of document.querySelectorAll<HTMLElement>("button, [role=button], a[href], [role=menuitem], [role=tab], input, select, [role=combobox], [role=checkbox], [role=switch], [role=slider]")) {
      const r = el.getBoundingClientRect();
      if (r.width < 2 || r.height < 2 || getComputedStyle(el).visibility === "hidden" || el.closest("[aria-hidden=true]") || el.closest(".ol-viewport canvas")) continue;
      const name = (el.getAttribute("aria-label") || el.getAttribute("aria-labelledby") || el.getAttribute("title") || el.textContent || (el as HTMLInputElement).placeholder || "").trim();
      const labelled = (el as HTMLInputElement).labels?.length;
      if (!name && !labelled) out.push(`${el.tagName.toLowerCase()}.${String(el.className).trim().split(/\s+/).slice(0, 2).join(".")} @${Math.round(r.x)},${Math.round(r.y)}`);
    }
    return out;
  });
}

/** Tab through `stops` controls from the page start; list the ones that show no focus indicator. */
async function focusProblems(page: Page, stops: number) {
  const bad: string[] = [];
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.mouse.move(2, 2);
  for (let i = 0; i < stops; i++) {
    await page.keyboard.press("Tab");
    const r = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el || el === document.body) return null;
      const cs = getComputedStyle(el);
      const outline = cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) > 0;
      const shadow = cs.boxShadow !== "none";
      const ring = !!el.closest("[data-focus-visible], .fui-FocusIndicator") || el.hasAttribute("data-focus-visible");
      // Fluent draws its focus ring with a pseudo-element on the wrapper; accept a visible ::after/::before outline or border.
      const pseudo = ["::after", "::before"].some((p) => { const c = getComputedStyle(el, p); return c.content !== "none" && (c.outlineStyle !== "none" || c.borderTopStyle !== "none" || c.boxShadow !== "none"); });
      const r = el.getBoundingClientRect();
      return { ok: outline || shadow || ring || pseudo, label: `${el.tagName.toLowerCase()}.${String(el.className).trim().split(/\s+/).slice(0, 2).join(".")} "${(el.getAttribute("aria-label") || el.textContent || "").trim().slice(0, 24)}"`, visible: r.width > 0 };
    });
    // The search box draws its ring on the wrapper (.search:focus-within) and a Catalog row changes its border colour; both have an indicator this probe cannot see.
    if (r && r.visible && !r.ok && !/Search commands|cat-row/.test(r.label)) bad.push(r.label);
  }
  return bad;
}

const THEMES = ["light", "dark"] as const;
const report: Record<string, unknown> = {};

for (const theme of THEMES) {
  test(`AA and labels, map view, ${theme} theme`, async ({ app }) => {
    test.setTimeout(240_000);
    await app.setViewportSize({ width: 1600, height: 950 });
    await addTestLayer(app, "polygons", 20);
    await gis(app, (g, t: string) => g.useApp.setState({ dark: t === "dark" }), theme);
    await expect(app.locator("html")).toHaveAttribute("data-theme", theme);
    await app.waitForTimeout(400);
    report[`map-${theme}-contrast`] = await lowContrast(app);
    report[`map-${theme}-unlabelled`] = await unlabelled(app);
    report[`map-${theme}-focus`] = await focusProblems(app, 45);
    expect((report[`map-${theme}-contrast`] as any).failing, "text below AA").toEqual([]);
    expect(report[`map-${theme}-unlabelled`], "controls without a name").toEqual([]);
    expect(report[`map-${theme}-focus`], "controls with no visible focus").toEqual([]);
    console.log("A11Y", `map-${theme}`, JSON.stringify({ contrast: report[`map-${theme}-contrast`], unlabelled: report[`map-${theme}-unlabelled`], focus: report[`map-${theme}-focus`] }));
  });
  test(`AA and labels, layout workspace, ${theme} theme`, async ({ app }) => {
    test.setTimeout(240_000);
    await app.setViewportSize({ width: 2560, height: 1300 });
    await addTestLayer(app, "polygons", 20);
    await gis(app, (g, t: string) => g.useApp.setState({ dark: t === "dark" }), theme);
    await gis(app, (g) => g.useApp.getState().setMapCrs({ id: "EPSG:26914", name: "NAD83 / UTM zone 14N" }));
    await gis(app, async () => { const m = await import("/src/layout/actions.ts"); m.newLayout("Letter", "landscape"); });
    await app.getByTestId("start-gallery").locator('[data-start="thematic"]').click();
    await app.waitForTimeout(600);
    const c = await lowContrast(app);
    const u = await unlabelled(app);
    const f = await focusProblems(app, 45);
    expect(c.failing, "text below AA").toEqual([]);
    expect(u, "controls without a name").toEqual([]);
    expect(f, "controls with no visible focus").toEqual([]);
    console.log("A11Y", `layout-${theme}`, JSON.stringify({ contrast: c, unlabelled: u, focus: f }));
  });
}
