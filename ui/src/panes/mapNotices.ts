// The messages drawn over the map ("N features aren't drawn…", "Basemap may be offset…") and how the user clears them.

export interface MapNoticeLine {
  id: string;
  text: string;
  /** "busy" lines ("Projecting…") go away by themselves; "warn" lines stay until cleared. */
  kind: "busy" | "warn";
}

/**
 * The lines to show: a cleared warning stays hidden while its text is the same, and comes back if the engine says something different
 * (a new count, another layer), so clearing never hides news.
 */
export function visibleNotices(lines: MapNoticeLine[], dismissed: Record<string, string>): MapNoticeLine[] {
  return lines.filter((l) => l.kind === "busy" || dismissed[l.id] !== l.text);
}

/** Dismiss one warning, or every warning in `lines` at once. */
export function dismissNotices(dismissed: Record<string, string>, lines: MapNoticeLine[]): Record<string, string> {
  const next = { ...dismissed };
  for (const l of lines) if (l.kind === "warn") next[l.id] = l.text;
  return next;
}
