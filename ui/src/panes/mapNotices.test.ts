import { describe, expect, it } from "vitest";
import { dismissNotices, visibleNotices, type MapNoticeLine } from "./mapNotices";

const warn = (id: string, text: string): MapNoticeLine => ({ id, text, kind: "warn" });
const busy = (id: string, text: string): MapNoticeLine => ({ id, text, kind: "busy" });

describe("map notices", () => {
  it("shows everything until something is cleared", () => {
    const lines = [busy("p", "Projecting…"), warn("a", "3 features outside"), warn("b", "Basemap offset")];
    expect(visibleNotices(lines, {})).toEqual(lines);
  });
  it("hides a cleared warning while its text is unchanged and brings it back when the text changes", () => {
    const a = warn("a", "3 features outside");
    const cleared = dismissNotices({}, [a]);
    expect(visibleNotices([a], cleared)).toEqual([]);
    expect(visibleNotices([warn("a", "5 features outside")], cleared)).toHaveLength(1);
  });
  it("clears one or all warnings, never a busy line", () => {
    const lines = [busy("p", "Projecting…"), warn("a", "x"), warn("b", "y")];
    expect(visibleNotices(lines, dismissNotices({}, [lines[1]])).map((l) => l.id)).toEqual(["p", "b"]);
    expect(visibleNotices(lines, dismissNotices({}, lines)).map((l) => l.id)).toEqual(["p"]);
  });
});
