// Engine-backed pointer readout (R2): one transform in flight, at most ~20 per second, and a
// response is shown only if nothing newer was shown already and the pointer hasn't left.

export interface Readout {
  /** Pointer moved to (x, y) in map CRS units. */
  move(x: number, y: number): void;
  /** Pointer left the map: clear at once and ignore anything still in flight. */
  leave(): void;
}

export function createReadout<T>(
  transform: (x: number, y: number) => Promise<T>,
  show: (value: { x: number; y: number; result: T } | null) => void,
  intervalMs = 50,
  now: () => number = () => performance.now(),
): Readout {
  let latest: [number, number] | null = null;
  let inFlight = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let lastSent = -Infinity;
  let generation = 0;
  let seq = 0;
  let applied = 0;

  const pump = () => {
    if (inFlight || !latest || timer) return;
    const wait = lastSent + intervalMs - now();
    if (wait > 0) {
      timer = setTimeout(() => {
        timer = null;
        pump();
      }, wait);
      return;
    }
    const [x, y] = latest;
    latest = null;
    inFlight = true;
    lastSent = now();
    const gen = generation;
    const my = ++seq;
    transform(x, y)
      .then((result) => {
        if (gen === generation && my > applied) {
          applied = my;
          show({ x, y, result });
        }
      })
      .catch(() => {
        if (gen === generation) show(null);
      })
      .finally(() => {
        inFlight = false;
        pump();
      });
  };

  return {
    move(x, y) {
      latest = [x, y];
      pump();
    },
    leave() {
      generation++;
      latest = null;
      show(null);
    },
  };
}
