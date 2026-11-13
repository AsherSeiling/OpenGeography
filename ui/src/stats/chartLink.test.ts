// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { applySelection, cover, modeFor, sanitizeSvg, selectableMarks } from "./chartLink";
import type { ChartSpec } from "./types";

const spec: ChartSpec = {
  kind: "scatter",
  title: "t",
  x: { label: "x" },
  y: { label: "y" },
  notes: [],
  series: [
    { name: "points", marks: [{ fids: [1] }, { fids: [2, 3] }, { bin: "sc:1,1", value: 40 }] },
    { name: "trend", style: "line", marks: [{ x: 0, y: 0 }, { x: 1, y: 1 }] },
    { name: "more", marks: [{ fids: [9] }] },
  ],
};

describe("chart to map linking", () => {
  it("indexes marks the way the engine numbers data-mark: every selectable mark of every series, lines skipped", () => {
    const m = selectableMarks(spec);
    expect(m).toHaveLength(4);
    expect(m[3].fids).toEqual([9]);
  });

  it("says how much of a mark a selection covers; a mark with no listed features is never covered", () => {
    const sel = new Set([2, 9]);
    expect(cover({ fids: [2, 3] }, sel)).toBe("some");
    expect(cover({ fids: [9] }, sel)).toBe("all");
    expect(cover({ fids: [1] }, sel)).toBe("none");
    expect(cover({ bin: "sc:1,1" }, sel)).toBe("none");
    expect(cover({ fids: [1] }, new Set())).toBe("none");
  });

  it("clicks mean what they mean on the map: plain replaces, Shift adds, Ctrl or Cmd toggles", () => {
    expect(modeFor({ shiftKey: false, ctrlKey: false, metaKey: false })).toBe("new");
    expect(modeFor({ shiftKey: true, ctrlKey: false, metaKey: false })).toBe("add");
    expect(modeFor({ shiftKey: false, ctrlKey: false, metaKey: true })).toBe("toggle");
  });

  it("strips scripts, handlers and javascript: links, and refuses text that is not SVG", () => {
    const svg = sanitizeSvg(
      `<svg xmlns="http://www.w3.org/2000/svg" onload="x()"><script>alert(1)</script><foreignObject><div/></foreignObject><a href="javascript:alert(1)"><circle class="mark" data-mark="0" onclick="y()" r="2"/></a></svg>`,
    )!;
    expect(svg.querySelector("script, foreignObject")).toBeNull();
    expect(svg.getAttribute("onload")).toBeNull();
    expect(svg.querySelector("circle")!.getAttribute("onclick")).toBeNull();
    expect(svg.querySelector("a")!.getAttribute("href")).toBeNull();
    expect(svg.querySelector("circle")!.getAttribute("data-mark")).toBe("0");
    expect(sanitizeSvg("<html><body>no</body></html>")).toBeNull();
    expect(sanitizeSvg("<svg")).toBeNull();
  });

  it("a map selection only toggles classes on the marks that are there", () => {
    const root = sanitizeSvg(`<svg xmlns="http://www.w3.org/2000/svg"><circle class="mark" data-mark="0"/><circle class="mark" data-mark="1"/><circle class="mark" data-mark="2"/><circle class="mark" data-mark="3"/></svg>`)!;
    const before = Array.from(root.querySelectorAll("circle"));
    expect(applySelection(root, spec, new Set([2, 9]))).toBe(2);
    expect(Array.from(root.querySelectorAll("circle"))).toEqual(before); // same nodes: nothing was redrawn
    const cls = (i: number) => before[i].getAttribute("class");
    expect([cls(0), cls(1), cls(2), cls(3)]).toEqual(["mark", "mark part", "mark", "mark sel"]);
    expect(root.classList.contains("has-sel")).toBe(true);
    applySelection(root, spec, new Set());
    expect(before.map((c) => c.getAttribute("class"))).toEqual(["mark", "mark", "mark", "mark"]);
    expect(root.classList.contains("has-sel")).toBe(false);
  });
});
