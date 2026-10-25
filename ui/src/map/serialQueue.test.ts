import { describe, expect, it } from "vitest";
import { serialQueue } from "./serialQueue";

const later = <T>(v: T, ms: number) => new Promise<T>((res) => setTimeout(() => res(v), ms));

describe("serialQueue", () => {
  it("applies results in click order even when the engine answers out of order", async () => {
    const q = serialQueue();
    const seen: number[] = [];
    q.add(later(1, 30), (v) => seen.push(v));
    q.add(later(2, 1), (v) => seen.push(v));
    q.add(later(3, 10), (v) => seen.push(v));
    await new Promise<void>((res) => q.then(res));
    expect(seen).toEqual([1, 2, 3]);
  });

  it("a double-click's finish lands after the last vertex (measure regression)", async () => {
    const q = serialQueue();
    const points: number[] = [];
    let done = false;
    // The old code set `done` at once and the slow last vertex then restarted the line.
    q.add(later(1, 5), (v) => points.push(v));
    q.add(later(2, 40), (v) => points.push(v));
    q.then(() => { done = true; });
    await new Promise<void>((res) => q.then(res));
    expect(points).toEqual([1, 2]);
    expect(done).toBe(true);
  });

  it("a failed engine call is skipped and does not block later events", async () => {
    const q = serialQueue();
    const seen: number[] = [];
    q.add(Promise.reject(new Error("outside the CRS")), (v: number) => seen.push(v));
    q.add(later(2, 1), (v) => seen.push(v));
    await new Promise<void>((res) => q.then(res));
    expect(seen).toEqual([2]);
  });
});
