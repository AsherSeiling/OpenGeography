import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createReadout } from "./readout";

describe("engine-backed readout (R2 / P8b)", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("a 5 s sweep at 200 Hz sends at most 110 transforms and ends on the last position", async () => {
    let calls = 0;
    const shown: ({ x: number; y: number; result: number } | null)[] = [];
    const r = createReadout(
      async (x) => {
        calls++;
        await new Promise((res) => setTimeout(res, 8)); // engine latency
        return x * 2;
      },
      (v) => shown.push(v),
      50,
      () => Date.now(),
    );
    for (let t = 0; t < 5000; t += 5) {
      r.move(t, 0);
      await vi.advanceTimersByTimeAsync(5);
    }
    await vi.advanceTimersByTimeAsync(200);
    expect(calls).toBeLessThanOrEqual(110);
    expect(calls).toBeGreaterThan(50);
    const last = shown[shown.length - 1];
    expect(last).toEqual({ x: 4995, y: 0, result: 9990 });
  });

  it("drops a response that arrives after the pointer left", async () => {
    const shown: unknown[] = [];
    const r = createReadout(
      async (x) => {
        await new Promise((res) => setTimeout(res, 30));
        return x;
      },
      (v) => shown.push(v),
      50,
      () => Date.now(),
    );
    r.move(1, 1);
    await vi.advanceTimersByTimeAsync(5);
    r.leave();
    await vi.advanceTimersByTimeAsync(100);
    expect(shown).toEqual([null]);
  });
});
