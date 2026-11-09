import { describe, expect, it } from "vitest";
import { looksLikeAngle, splitPasted } from "./split";

const ok = (s: string) => {
  const r = splitPasted(s);
  if (!r || "error" in r) throw new Error(`expected a split of '${s}', got ${JSON.stringify(r)}`);
  return r;
};

describe("splitPasted", () => {
  it("fills latitude then longitude from a comma, semicolon, tab or new line", () => {
    for (const s of ["34.05, -118.25", "34.05,-118.25", "34.05; -118.25", "34.05\t-118.25", "34.05\n-118.25", "  34.05 , -118.25  "]) {
      const r = ok(s);
      expect([r.lat, r.lon]).toEqual(["34.05", "-118.25"]);
    }
  });
  it("splits two plain numbers separated by a space, in the order latitude then longitude", () => {
    const r = ok("34.05 -118.25");
    expect([r.lat, r.lon]).toEqual(["34.05", "-118.25"]);
  });
  it("lets N, S, E and W override the order, never swapping silently", () => {
    let r = ok("118.25W 34.05N");
    expect([r.lat, r.lon]).toEqual(["34.05N", "118.25W"]);
    expect(r.note).toMatch(/letters/);
    r = ok("118.25 W, 34.05 N");
    expect([r.lat, r.lon]).toEqual(["34.05 N", "118.25 W"]);
    r = ok("34.05N 118.25W");
    expect([r.lat, r.lon]).toEqual(["34.05N", "118.25W"]);
    expect(r.note).toMatch(/^Filled as latitude, then longitude/);
  });
  it("splits degrees, minutes and seconds with letters before or after", () => {
    let r = ok("34 03 08 N 118 15 00 W");
    expect([r.lat, r.lon]).toEqual(["34 03 08 N", "118 15 00 W"]);
    r = ok("N 34 03 08 W 118 15 00");
    expect([r.lat, r.lon]).toEqual(["N 34 03 08", "W 118 15 00"]);
    r = ok("34°03′08″N 118°15′00″W");
    expect([r.lat, r.lon]).toEqual(["34°03′08″N", "118°15′00″W"]);
    r = ok("118 15 00 W 34 03 08 N");
    expect([r.lat, r.lon]).toEqual(["34 03 08 N", "118 15 00 W"]);
  });
  it("splits letterless groups of equal size", () => {
    expect(ok("34 03 08 -118 15 00")).toMatchObject({ lat: "34 03 08", lon: "-118 15 00" });
    expect(ok("34 3.5 -118 15.25")).toMatchObject({ lat: "34 3.5", lon: "-118 15.25" });
  });
  it("leaves a single coordinate alone", () => {
    expect(splitPasted("34.05")).toBeNull();
    expect(splitPasted("   ")).toBeNull();
  });
  it("reports what it cannot split instead of guessing", () => {
    for (const s of ["34.05, -118.25, 100", "34 03 08 118 15", "34.05N 40N", "118W 34E", "1N 2E 3W"]) {
      const r = splitPasted(s);
      expect(r && "error" in r, s).toBe(true);
    }
  });
  it("does not check ranges: the engine refuses a latitude beyond 90 and says when the values look swapped", () => {
    const r = ok("-118.25, 34.05");
    expect([r.lat, r.lon]).toEqual(["-118.25", "34.05"]);
  });
});

describe("looksLikeAngle", () => {
  it("accepts decimal and DMS text and rejects words", () => {
    for (const s of ["34.05", "-118.25", "34°03′08″N", "N 34 03 08", "34:03:08"]) expect(looksLikeAngle(s), s).toBe(true);
    for (const s of ["", "abc", "N", "34 apples"]) expect(looksLikeAngle(s), s).toBe(false);
  });
});
