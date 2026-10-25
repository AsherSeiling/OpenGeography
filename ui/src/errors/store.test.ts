import { beforeEach, describe, expect, it } from "vitest";
import { errorMessage, friendly, reportError, useErrors } from "./store";

beforeEach(() => useErrors.getState().clear());
const q = () => useErrors.getState().queue;

describe("error store", () => {
  it("collapses repeats into one entry with a count", () => {
    for (let i = 0; i < 3; i++) reportError(new Error("boom"), "window");
    expect(q()).toHaveLength(1);
    expect(q()[0].count).toBe(3);
  });
  it("queues different errors in order and dismisses the head", () => {
    reportError("first", "promise");
    reportError("second", "promise");
    expect(q().map((e) => e.message)).toEqual(["first", "second"]);
    useErrors.getState().dismiss();
    expect(q()[0].message).toBe("second");
  });
  it("keeps the stack in details, not the message", () => {
    reportError(new Error("x"), "window", { context: "testing" });
    expect(q()[0].details).toContain("While: testing");
    expect(q()[0].message).toBe("x");
  });
  it("turns raw JS errors into plain language and ignores ResizeObserver noise", () => {
    reportError(new TypeError("Cannot read properties of undefined (reading 'w')"), "window");
    expect(q()[0].message).not.toContain("properties of undefined");
    useErrors.getState().clear();
    reportError("ResizeObserver loop completed with undelivered notifications.", "window");
    expect(q()).toHaveLength(0);
  });
  it("reads engine error shapes", () => {
    expect(errorMessage({ message: "bad crs" })).toBe("bad crs");
    expect(errorMessage("plain")).toBe("plain");
    expect(friendly("Failed to fetch").title).toMatch(/engine/);
  });
});
