// UX1: wording for the recovery offer and the autosave interval. Pure, so it is tested without a window.
import type { RecoveryEntry } from "../ipc/api";

/** Minutes between autosaves: unset = 2, 0 = off, anything else is kept within 1–120. */
export function autosaveEveryMinutes(setting: number | null | undefined): number {
  if (setting === 0) return 0;
  if (typeof setting !== "number" || !Number.isFinite(setting)) return 2;
  return Math.min(120, Math.max(1, Math.round(setting)));
}

export const AUTOSAVE_CHOICES: [number, string][] = [[0, "Off"], [1, "Every minute"], [2, "Every 2 minutes"], [5, "Every 5 minutes"], [10, "Every 10 minutes"], [30, "Every 30 minutes"]];

/** "3 minutes ago", "2 hours ago", or the date for anything older than a day. */
export function ago(writtenAt: number, nowMs: number = Date.now()): string {
  const s = Math.max(0, Math.round(nowMs / 1000 - writtenAt));
  if (s < 90) return "a moment ago";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} minutes ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} hour${h === 1 ? "" : "s"} ago`;
  return new Date(writtenAt * 1000).toLocaleString();
}

/** One line telling the person what a copy is. */
export function describeEntry(e: RecoveryEntry, nowMs?: number): string {
  if (e.problem) return e.problem;
  return `${e.name || "Untitled project"}, last saved automatically ${ago(e.writtenAt, nowMs)}${e.origin ? `, from ${e.origin}` : ", never saved to a folder"}`;
}
