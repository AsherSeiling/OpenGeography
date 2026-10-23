// Progress bars for long engine work. The engine keeps a list of running
// tasks (downloads, imports, model runs, tile building); this polls it —
// quickly while a tracked request is in flight, slowly otherwise — so any
// long operation shows a real bar.
import { Button, ProgressBar } from "@fluentui/react-components";
import { useEffect } from "react";
import { create } from "zustand";
import { api, errorText, type TaskInfo } from "../ipc/api";

interface ProgressState {
  tasks: TaskInfo[];
  /** Requests started with `track` that haven't finished. */
  pending: number;
}

export const useProgress = create<ProgressState>(() => ({ tasks: [], pending: 0 }));

let timer: ReturnType<typeof setTimeout> | null = null;
let polling = false;

async function poll() {
  timer = null;
  if (polling) return;
  polling = true;
  try {
    useProgress.setState({ tasks: await api.progress() });
  } catch {
    // Engine busy or unreachable: keep the last state.
  }
  polling = false;
  const { tasks, pending } = useProgress.getState();
  schedule(tasks.length || pending ? 250 : 1500);
}

function schedule(ms: number) {
  if (timer) clearTimeout(timer);
  timer = setTimeout(poll, ms);
}

/** Start polling (once, at app start). */
export function startProgressPolling() {
  if (!timer && !polling) schedule(500);
}

/** Track a long request so its progress is polled quickly until it ends. */
export async function track<T>(p: Promise<T>): Promise<T> {
  useProgress.setState((s) => ({ pending: s.pending + 1 }));
  schedule(150);
  try {
    return await p;
  } finally {
    useProgress.setState((s) => ({ pending: s.pending - 1 }));
    schedule(50);
  }
}

/** The newest engine task whose label starts with one of `prefixes` (any task if none given). */
function pickTask(tasks: TaskInfo[], prefixes?: string[]) {
  const list = prefixes ? tasks.filter((t) => prefixes.some((p) => t.label.startsWith(p))) : tasks;
  return list[0];
}

/**
 * A labeled progress bar for the engine task matching `match` (label prefixes),
 * shown while `active`. Before the engine reports, the bar is indeterminate.
 */
export function TaskProgress({ active, label, match }: { active: boolean; label: string; match?: string[] }) {
  const task = useProgress((s) => pickTask(s.tasks, match));
  useEffect(() => {
    if (active) schedule(100);
  }, [active]);
  if (!active) return null;
  const pct = task?.fraction != null ? Math.round(task.fraction * 100) : null;
  return (
    <div className="task-progress">
      <div className="tp-head">
        <span>{task?.label ?? label}</span>
        {pct !== null && <span className="tp-pct">{pct}%</span>}
      </div>
      <ProgressBar thickness="large" value={task?.fraction ?? undefined} max={1} />
      <div className="tp-foot">
        <span className="tp-detail muted">{task?.detail || "Starting…"}</span>
        {task?.cancellable && (
          <Button size="small" appearance="subtle" disabled={task.detail === "Cancelling…"} onClick={() => api.cancelTask(task.id)}>
            Cancel
          </Button>
        )}
      </div>
    </div>
  );
}

/** Compact bar for the status bar: the oldest running task. */
export function StatusProgress() {
  const tasks = useProgress((s) => s.tasks);
  const pending = useProgress((s) => s.pending);
  const t = tasks[0];
  if (!t && !pending) return null;
  const more = tasks.length > 1 ? ` (+${tasks.length - 1} more)` : "";
  return (
    <span className="sb-progress" title={t ? `${t.label}${t.detail ? ` — ${t.detail}` : ""}` : "Working…"}>
      <span className="sb-progress-label">{t ? `${t.label}${more}` : "Working…"}</span>
      <ProgressBar className="sb-progress-bar" value={t?.fraction ?? undefined} max={1} />
      {t?.fraction != null && <span className="sb-progress-pct">{Math.round(t.fraction * 100)}%</span>}
      {t?.cancellable && (
        <button className="sb-progress-x" title="Cancel" onClick={() => api.cancelTask(t.id)}>
          ✕
        </button>
      )}
    </span>
  );
}

/** True for the error a cancelled engine task returns. */
export const isCancelled = (e: unknown) => errorText(e).trim().endsWith("Cancelled");

/** A bar for work the engine doesn't report (local loading). */
export function WaitBar({ label }: { label: string }) {
  return (
    <div className="task-progress">
      <div className="tp-head"><span>{label}</span></div>
      <ProgressBar thickness="large" />
    </div>
  );
}
