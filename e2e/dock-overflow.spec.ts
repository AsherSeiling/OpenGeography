import { expect, test } from "@playwright/test";

// The rule between panes must not add size: no dockview view may scroll or overflow its box (WP 1 regression).
for (const width of [1024, 1440]) {
  for (const theme of ["light", "dark"]) {
    test(`dock panes do not overflow at ${width}px, ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 900 });
      await page.goto("/");
      await page.waitForSelector(".dv-groupview");
      await page.evaluate((t) => (document.documentElement.dataset.theme = t), theme);
      await page.waitForTimeout(300);
      const bad = await page.evaluate(() =>
        [...document.querySelectorAll<HTMLElement>(".dv-view")].map((v) => ({ cls: v.className, dw: v.scrollWidth - v.clientWidth, dh: v.scrollHeight - v.clientHeight, ow: v.offsetWidth, oh: v.offsetHeight })),
      );
      console.log(JSON.stringify(bad));
      for (const v of bad) {
        expect(v.dw, v.cls).toBeLessThanOrEqual(0);
        expect(v.dh, v.cls).toBeLessThanOrEqual(0);
      }
    });
  }
}
