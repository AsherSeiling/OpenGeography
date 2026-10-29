import { describe, expect, it } from "vitest";
import { FALLBACK_FONTS, matchFonts } from "./fonts";

describe("matchFonts", () => {
  it("filters by substring ignoring case and limits the result", () => {
    expect(matchFonts(FALLBACK_FONTS, "NEW").map((f) => f.name)).toEqual(["Times New Roman", "Courier New"]);
    expect(matchFonts(FALLBACK_FONTS, "", 2)).toHaveLength(2);
    expect(matchFonts(FALLBACK_FONTS, "zzz")).toEqual([]);
  });
});
