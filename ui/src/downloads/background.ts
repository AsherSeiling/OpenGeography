// Public-data downloads run in the background (UR-9): the dialog closes at once, the status bar shows the progress with a cancel
// button (the engine's own task list), and the rest of the app keeps working. When the download ends its layers are added to the project.
// Save, New and Open ask first while one is running, so a half-finished download never lands in the wrong project.
import { create } from "zustand";
import { api, errorText } from "../ipc/api";
import { reportError } from "../errors/store";
import { isCancelled, useProgress } from "../progress/progress";
import { useApp } from "../store/store";

interface Job {
  id: number;
  label: string;
  /** Prefixes of the engine task labels that belong to this download (to cancel it). */
  match: string[];
  settled: Promise<void>;
}

export const useDownloads = create<{ jobs: Job[] }>(() => ({ jobs: [] }));
let next = 1;

export const downloadsRunning = () => useDownloads.getState().jobs.length > 0;
export const downloadLabels = () => useDownloads.getState().jobs.map((j) => j.label).join(", ");

/**
 * Start `run` in the background. `done` gets the result and returns the message to show (it adds the layers); a rejection is
 * shown as "<label> failed" with the error pop-up, and a cancel as "<label> cancelled".
 */
export function startDownload<T>(opts: { label: string; match: string[]; run: () => Promise<T>; done: (r: T) => string | void }): void {
  const set = useApp.getState().setStatus;
  const id = next++;
  const settled = (async () => {
    try {
      const r = await opts.run();
      const msg = opts.done(r);
      if (msg) set(msg);
    } catch (e) {
      if (isCancelled(e)) set(`${opts.label} cancelled`);
      else {
        set(`${opts.label} failed: ${errorText(e)}`);
        reportError(e, `download:${opts.label}`, { title: `${opts.label} failed`, context: opts.label });
      }
    } finally {
      useDownloads.setState((s) => ({ jobs: s.jobs.filter((j) => j.id !== id) }));
    }
  })();
  useDownloads.setState((s) => ({ jobs: [...s.jobs, { id, label: opts.label, match: opts.match, settled }] }));
  set(`${opts.label} started — it runs in the background; progress and Cancel are in the status bar`);
}

/** Cancel every running download and wait until each has ended. */
export async function cancelDownloads(): Promise<void> {
  const jobs = useDownloads.getState().jobs;
  const tasks = useProgress.getState().tasks;
  for (const j of jobs) for (const t of tasks) if (t.cancellable && j.match.some((m) => t.label.startsWith(m))) await api.cancelTask(t.id).catch(() => {});
  await Promise.all(jobs.map((j) => j.settled));
}

/**
 * Before a project action that would lose or misplace a running download. `action` completes "A download is still running (…). ".
 * Returns false when the user keeps the download and stops the action.
 */
export async function confirmDownloads(action: "save" | "leave"): Promise<boolean> {
  if (!downloadsRunning()) return true;
  const what = `A download is still running (${downloadLabels()}).`;
  if (action === "save") {
    return confirm(`${what}\n\nSave now? The layers it is fetching are not in this save: they are added when it finishes, and the project is then unsaved again.`);
  }
  if (!confirm(`${what}\n\nCancel it and continue? Nothing it fetched so far is kept.`)) return false;
  await cancelDownloads();
  return true;
}
