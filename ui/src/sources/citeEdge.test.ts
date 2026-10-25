import { describe, expect, it } from "vitest";
import type { Citation, SourceEntry } from "../ipc/api";
import { annotation, bibliographyFiles, referenceText, sorted, toBibtex, toHtml, toMarkdown, toRis, usDate } from "./cite";

const blank: Citation = { key: "k", author: "", year: "", title: "", version: "", publisher: "", url: "", kind: "Local file", license: "", annotation: "", complete: false };
const entry = (c: Partial<Citation>, uses: string[] = []): SourceEntry => ({ citation: { ...blank, ...c }, firstUsed: "2026-01-02", uses: uses.map((n, i) => ({ layerId: `L${i}`, layerName: n, at: "2026-01-02" })) });

describe("citations with missing parts", () => {
  it("an all-empty citation still produces text in every style, without 'undefined' or double spaces", () => {
    for (const style of ["apa", "mla", "chicago"] as const) {
      const t = referenceText(blank, "2026-01-02", style);
      expect(t, style).toContain("[Untitled]");
      expect(t, style).not.toMatch(/undefined|null|NaN|\s{2}/);
    }
  });
  it("a bad access date doesn't throw", () => {
    expect(() => usDate("")).not.toThrow();
    expect(() => referenceText({ ...blank, url: "https://x.org" }, "not-a-date", "apa")).not.toThrow();
  });
  it("unicode and markup characters in titles survive and are escaped in HTML", () => {
    const e = entry({ title: "Zürich <b>& co</b>", author: "Müller" });
    const html = toHtml([e], "P");
    expect(html).toContain("Zürich");
    expect(html).not.toContain("<b>& co</b>");
    expect(toMarkdown([e], "P")).toContain("Müller");
  });
});

describe("collections", () => {
  it("empty list exports are valid and empty", () => {
    expect(toBibtex([])).toBe("");
    expect(toRis([]).trim()).toBe("");
    expect(toMarkdown([], "P")).toContain("P");
    const files = bibliographyFiles([], "P");
    expect(Object.keys(files).length).toBeGreaterThan(0);
  });
  it("BibTeX keys stay unique when two sources share a title", () => {
    const keys = toBibtex([entry({ title: "Roads", author: "A", key: "a" }), entry({ title: "Roads", author: "A", key: "b" })]).match(/@\w+\{([^,]+),/g)!;
    expect(new Set(keys).size).toBe(2);
  });
  it("sorting puts incomplete entries last and ignores leading articles", () => {
    const list = [entry({ title: "Zeta", author: "" }), entry({ title: "The Apple", author: "Zed" }), entry({ title: "B", author: "Anders" })];
    expect(sorted(list, "apa").map((e) => e.citation.title)).toEqual(["B", "The Apple", "Zeta"]);
  });
  it("annotation lists each layer once and omits empty parts", () => {
    expect(annotation(entry({}, ["A", "A", "B"]))).toBe("Used for: A; B.");
    expect(annotation(entry({ annotation: "Note.", license: "CC0" }, []))).toBe("Note. License: CC0");
    expect(annotation(entry({}, []))).toBe("");
  });
  it("hundreds of sources format quickly", () => {
    const many = Array.from({ length: 500 }, (_, i) => entry({ key: `k${i}`, title: `T${i}`, author: `A${i}` }, ["x"]));
    const t = Date.now();
    expect(toMarkdown(many, "P").length).toBeGreaterThan(5000);
    expect(Date.now() - t).toBeLessThan(1500);
  });
});
