import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// CP3 control audit: the operating system's colour panel is never used; every colour setting goes through ColorField.
const SRC = fileURLToPath(new URL("..", import.meta.url));
const walk = (d: string, out: string[] = []): string[] => {
  for (const n of readdirSync(d)) {
    const p = join(d, n);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.tsx$/.test(n) && !/\.test\.tsx$/.test(n)) out.push(p);
  }
  return out;
};

describe("colour control audit", () => {
  it("no <input type=\"color\"> remains in the UI", () => {
    const bad = walk(SRC).filter((p) => /<input[^>]*type=["']color["']/.test(readFileSync(p, "utf8")));
    expect(bad).toEqual([]);
  });
  it("ColorField is used by the layer, label, class, layout and tool controls", () => {
    for (const f of ["panes/PropertiesPane.tsx", "panes/UniqueClasses.tsx", "panes/LabelSettings.tsx", "layout/ElementProperties.tsx", "layout/TextSection.tsx", "layout/LayoutMenus.tsx", "nodes/ParamControl.tsx"]) {
      expect(readFileSync(join(SRC, f), "utf8"), f).toContain("<ColorField");
    }
  });
});
