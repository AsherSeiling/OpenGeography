import { existsSync } from "node:fs";
import { defineConfig } from "@playwright/test";

// Ports and output come from scripts/e2e.sh. Browser: $E2E_BROWSER_PATH, else the installed Google Chrome,
// else a Chromium.app in /Applications, else Playwright's own Chromium (`npx playwright install chromium`).
const vite = process.env.E2E_VITE_PORT ?? "1420";
const chromePaths = ["/Applications/Google Chrome.app", "/usr/bin/google-chrome", "/opt/google/chrome/chrome", "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"];
const launch: Record<string, unknown> = {};
if (process.env.E2E_BROWSER_PATH) launch.executablePath = process.env.E2E_BROWSER_PATH;
else if (chromePaths.some(existsSync)) launch.channel = "chrome";
else if (existsSync("/Applications/Chromium.app/Contents/MacOS/Chromium")) launch.executablePath = "/Applications/Chromium.app/Contents/MacOS/Chromium";

// E2E_WEBKIT=1 runs in Playwright's WebKit (the engine of the macOS app's webview) instead of Chrome: `npx playwright install webkit` once.
const webkit = process.env.E2E_WEBKIT === "1";

export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  expect: { timeout: 10_000 },
  workers: 1,
  fullyParallel: false,
  reporter: [["list"], ["json", { outputFile: process.env.E2E_JSON ?? "test-results/e2e.json" }]],
  outputDir: process.env.E2E_ARTIFACTS ?? "test-results/e2e-artifacts",
  use: {
    baseURL: `http://localhost:${vite}`,
    viewport: { width: 1600, height: 950 },
    ...(webkit ? { browserName: "webkit" as const, launchOptions: process.env.E2E_BROWSER_PATH ? { executablePath: process.env.E2E_BROWSER_PATH } : {} } : { launchOptions: launch }),
    screenshot: "only-on-failure",
    trace: "off",
  },
});
