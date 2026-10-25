import { describe, expect, it } from "vitest";
import { errorText } from "./api";

describe("errorText", () => {
  it("passes strings through, unwraps Errors, and stringifies anything else", () => {
    expect(errorText("plain")).toBe("plain");
    expect(errorText(new Error("boom"))).toBe("boom");
    expect(errorText({ message: "from rust" })).toBe('{"message":"from rust"}');
    expect(errorText(42)).toBe("42");
    expect(errorText(null)).toBe("null");
    expect(errorText("")).toBe("");
  });
  it("returns something printable for values JSON cannot represent", () => {
    expect(typeof errorText(undefined)).not.toBe("object");
  });
});
