import doc from "../../../docs/RESPONSIBLE_USE.md?raw";
import { describe, expect, it } from "vitest";
import { RESPONSIBLE_USE, RESPONSIBLE_USE_TITLE } from "./responsibleUse";

describe("responsible use statement", () => {
  it("covers the topics the plan requires", () => {
    const all = RESPONSIBLE_USE.map((s) => s.text).join(" ");
    for (const w of ["datum grids", "Heights", "survey instrument", "Sensitive", "estimates"]) {
      expect(all.toLowerCase() + RESPONSIBLE_USE.map((s) => s.heading).join(" ").toLowerCase()).toContain(w.toLowerCase());
    }
    expect(RESPONSIBLE_USE.length).toBeGreaterThanOrEqual(5);
  });

  it("is the same text as docs/RESPONSIBLE_USE.md", () => {
    expect(doc).toContain(`# ${RESPONSIBLE_USE_TITLE}`);
    for (const s of RESPONSIBLE_USE) {
      expect(doc).toContain(`## ${s.heading}`);
      expect(doc).toContain(s.text);
    }
  });
});
