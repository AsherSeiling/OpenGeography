// Map tool events arrive faster than the engine answers (each clicked position goes through
// `api.transformPoint`), so their effects must be applied in click order, and a double-click's
// "finish" has to come after the vertex of the click that started it.

export interface SerialQueue {
  /** Resolve `value`, then run `apply` with it — after every earlier `add`/`then` has finished. */
  add<T>(value: Promise<T>, apply: (v: T) => void): void;
  /** Run `fn` after everything queued so far. */
  then(fn: () => void): void;
}

export function serialQueue(): SerialQueue {
  let tail: Promise<void> = Promise.resolve();
  return {
    add(value, apply) {
      const mine = value.then((v) => ({ ok: true as const, v }), () => ({ ok: false as const }));
      tail = tail.then(() => mine).then((r) => {
        if (r.ok) apply(r.v);
      });
    },
    then(fn) {
      tail = tail.then(fn);
    },
  };
}
