import { describe, expect, it } from "vitest";
import { parsePageSetup } from "./PageSetupDialog";

const page = { width: 279.4, height: 215.9, units: "mm" as const, margin: 12.7, background: "#ffffff" };

describe("page setup parsing", () => {
  it("accepts sensible values", () => {
    const r = parsePageSetup({ width: "300", height: "200", margin: "10", bleed: "3" }, page);
    expect("page" in r && r.page).toMatchObject({ width: 300, height: 200, margin: 10, bleed: 3, background: "#ffffff" });
  });
  it("explains what is wrong", () => {
    expect(parsePageSetup({ width: "20000", height: "14000", margin: "10", bleed: "0" }, page)).toMatchObject({ page: { width: 20000, height: 14000 } });
    expect(parsePageSetup({ width: "20001", height: "100", margin: "10", bleed: "0" }, page)).toEqual({ error: expect.stringContaining("20,000 mm") });
    expect(parsePageSetup({ width: "19", height: "100", margin: "0", bleed: "0" }, page)).toEqual({ error: expect.stringContaining("between 20 mm") });
    expect(parsePageSetup({ width: "abc", height: "200", margin: "10", bleed: "0" }, page)).toEqual({ error: expect.stringContaining("width and height") });
    expect(parsePageSetup({ width: "100", height: "100", margin: "60", bleed: "0" }, page)).toEqual({ error: expect.stringContaining("margin") });
    expect(parsePageSetup({ width: "100", height: "100", margin: "5", bleed: "11" }, page)).toEqual({ error: expect.stringContaining("Bleed") });
    expect(parsePageSetup({ width: "100", height: "100", margin: "5", bleed: "" }, page)).toEqual({ error: expect.stringContaining("Bleed") });
  });
});
