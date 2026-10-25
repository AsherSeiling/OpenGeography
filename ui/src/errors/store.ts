// One queue of user-facing errors. Repeats of the same error collapse into one
// entry with a count; the dialog shows the head of the queue.
import { create } from "zustand";

export interface AppError {
  id: number;
  /** Dedupe key: same title + message + where. */
  key: string;
  title: string;
  message: string;
  /** Stack and context, shown collapsed under "Details". */
  details: string;
  /** Where it came from: "window", "promise", "pane:map", "action:Saving project"… */
  source: string;
  count: number;
  /** Pane to reopen, when a pane crashed. */
  pane?: string;
}

interface ErrorState {
  queue: AppError[];
  push: (e: Omit<AppError, "id" | "key" | "count">) => void;
  /** Close the current error. */
  dismiss: () => void;
  clear: () => void;
}

let nextErrorId = 1;

export const useErrors = create<ErrorState>((set, get) => ({
  queue: [],
  push: (e) => {
    const key = `${e.source}|${e.title}|${e.message}`;
    const q = get().queue;
    const same = q.find((x) => x.key === key);
    if (same) set({ queue: q.map((x) => (x === same ? { ...x, count: x.count + 1 } : x)) });
    else set({ queue: [...q, { ...e, id: nextErrorId++, key, count: 1 }].slice(-20) });
  },
  dismiss: () => set({ queue: get().queue.slice(1) }),
  clear: () => set({ queue: [] }),
}));

/** Text of anything thrown: strings, Errors, and the engine's JSON errors. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === "object") {
    const m = (e as { message?: unknown; error?: unknown }).message ?? (e as { error?: unknown }).error;
    if (typeof m === "string") return m;
  }
  try {
    return JSON.stringify(e) ?? String(e);
  } catch {
    return String(e);
  }
}

export function errorStack(e: unknown): string {
  return e instanceof Error && e.stack ? e.stack : errorMessage(e);
}

/** Plain-language title and message for a raw error. */
export function friendly(raw: string): { title: string; message: string } {
  const m = raw.trim();
  if (/Failed to fetch|NetworkError|Load failed/i.test(m)) return { title: "Can't reach the engine", message: "The GIS engine did not answer. If you run the UI in a browser, start the dev server; otherwise restart the app." };
  if (/ResizeObserver loop/i.test(m)) return { title: "", message: "" }; // harmless browser notice
  if (/Cannot read properties of (undefined|null)|is not a function|is undefined|is not defined|Maximum update depth/i.test(m))
    return { title: "Something went wrong in the interface", message: "A part of the window hit an unexpected state. Your data is not changed. You can keep working, or reload if things look wrong." };
  if (/out of memory|allocation/i.test(m)) return { title: "Out of memory", message: m };
  return { title: "Something went wrong", message: m || "Unknown error." };
}

/** Queue an error for the dialog. `context` says what the user was doing. */
export function reportError(e: unknown, source: string, opts?: { title?: string; message?: string; context?: string; pane?: string }) {
  const raw = errorMessage(e);
  const f = friendly(raw);
  if (!f.title && !opts?.title) return; // ignorable
  const details = [opts?.context && `While: ${opts.context}`, `Source: ${source}`, errorStack(e)].filter(Boolean).join("\n\n");
  logSink?.(`${opts?.title ?? f.title}: ${raw}`);
  useErrors.getState().push({ title: opts?.title ?? f.title, message: opts?.message ?? (opts?.title ? raw : f.message), details, source, pane: opts?.pane });
}

let logSink: ((message: string) => void) | null = null;
/** main.tsx hands over its app-log writer so errors also reach the log. */
export function setErrorLog(f: (message: string) => void) {
  logSink = f;
}
