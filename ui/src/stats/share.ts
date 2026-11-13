// Sharing a sheet (ST7): send a card's chart or table to a layout, export tables as CSV and charts as SVG or PNG, and a report of the whole sheet.
// Each export asks where to save (the native dialog; a path prompt in the browser build) unless a path is given.
import { api, errorText } from "../ipc/api";
import { reportError } from "../errors/store";
import { save } from "../ipc/dialogs";
import { newLayout } from "../layout/actions";
import { addStatCardToLayout, useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { useStats } from "./store";
import type { StatCard, StatSheet } from "./types";

const status = (s: string) => useApp.getState().setStatus(s);

/** The open sheet and its selected card (or the only card there is). */
export function pickedForShare(): { sheet: StatSheet; card: StatCard } | null {
  const s = useStats.getState();
  const sheet = s.id ? s.docs[s.id] : undefined;
  if (!sheet) return null;
  const card = sheet.cards.find((c) => c.id === s.selected) ?? (sheet.cards.length === 1 ? sheet.cards[0] : undefined);
  return card ? { sheet, card } : null;
}

/** A name that is safe as a file name on every platform. */
export function fileStem(name: string): string {
  const t = name.replace(/[\\/:*?"<>|\x00-\x1f]+/g, " ").replace(/\s+/g, " ").trim().replace(/[. ]+$/, "");
  return t.slice(0, 80) || "statistics";
}

/** `base.csv` for the first, `base-2.csv` for the next: one file per table or chart of a card. */
export function numberedPath(path: string, i: number): string {
  if (i === 0) return path;
  return path.replace(/(\.[^./\\]+)?$/, (ext) => `-${i + 1}${ext}`);
}

const cardName = (card: StatCard, sheet: StatSheet) => fileStem(`${sheet.name} ${card.title || card.tool}`);

async function ask(title: string, name: string, ext: string, label: string, path?: string): Promise<string | null> {
  if (path) return path;
  try {
    return await save({ title, defaultPath: name, filters: [{ name: label, extensions: [ext] }] });
  } catch (e) {
    reportError(e, "action:Export statistics", { title: "Export could not start" });
    return null;
  }
}

function failed(what: string, e: unknown) {
  status(`${what} failed: ${errorText(e)}`);
  reportError(e, "action:Export statistics", { title: `${what} failed`, context: "exporting from a statistics sheet" });
}

/** Every table of the selected card as CSV. Returns the files written. */
export async function exportCardTable(path?: string): Promise<string[]> {
  const p = pickedForShare();
  if (!p) return [];
  const n = p.card.result?.tables.length ?? 0;
  if (!n) {
    status(p.card.result ? "This card has no table to export." : "This card has not been run yet. Run it, then export.");
    return [];
  }
  const target = await ask("Export table as CSV", `${cardName(p.card, p.sheet)}.csv`, "csv", "CSV", path);
  if (!target) return [];
  const written: string[] = [];
  try {
    for (let i = 0; i < n; i++) {
      await api.statExportTable(p.card, i, numberedPath(target, i));
      written.push(numberedPath(target, i));
    }
    status(`Exported ${written.join(", ")}`);
  } catch (e) {
    failed("Table export", e);
  }
  return written;
}

/** Every chart of the selected card as SVG or PNG (PNG at 192 dpi: twice the size on screen). Returns the files written. */
export async function exportCardChart(format: "svg" | "png", path?: string, dpi = 192): Promise<string[]> {
  const p = pickedForShare();
  if (!p) return [];
  const n = p.card.result?.charts.length ?? 0;
  if (!n) {
    status(p.card.result ? "This card has no chart to export." : "This card has not been run yet. Run it, then export.");
    return [];
  }
  const target = await ask(`Export chart as ${format.toUpperCase()}`, `${cardName(p.card, p.sheet)}.${format}`, format, format.toUpperCase(), path);
  if (!target) return [];
  const written: string[] = [];
  try {
    for (let i = 0; i < n; i++) {
      await api.statExportChart(p.card, i, format, dpi, numberedPath(target, i));
      written.push(numberedPath(target, i));
    }
    status(`Exported ${written.join(", ")}`);
  } catch (e) {
    failed("Chart export", e);
  }
  return written;
}

/** The whole open sheet as one HTML report with methods and citations. Returns the engine's warnings (cards not run, out of date), or null if nothing was written. */
export async function exportSheetReport(path?: string): Promise<string[] | null> {
  const s = useStats.getState();
  const sheet = s.id ? s.docs[s.id] : undefined;
  if (!sheet) return null;
  if (!sheet.cards.length) {
    status("Add and run at least one card before making a report.");
    return null;
  }
  const target = await ask("Export sheet report", `${fileStem(sheet.name)}.html`, "html", "HTML", path);
  if (!target) return null;
  try {
    const warnings = await api.statExportReport(sheet, new Date().toISOString().slice(0, 10), target);
    status(`Exported ${target}${warnings.length ? ` · ${warnings.length} warning${warnings.length > 1 ? "s" : ""}: ${warnings[0]}` : ""}`);
    return warnings;
  } catch (e) {
    failed("Report export", e);
    return null;
  }
}

/** Put the selected card's first chart or table on a layout (an existing one by id, or "new"). Old layouts are not changed except by this one element. */
export function sendCardToLayout(part: "chart" | "table", layoutId: string): string | null {
  const p = pickedForShare();
  if (!p) return null;
  const have = part === "chart" ? p.card.result?.charts.length : p.card.result?.tables.length;
  if (!p.card.result) {
    status("This card has not been run yet. Run it, then send it to a layout.");
    return null;
  }
  if (!have) {
    status(`This card has no ${part} to send.`);
    return null;
  }
  let target = layoutId;
  if (layoutId === "new") {
    newLayout();
    target = useLayouts.getState().id ?? "";
  }
  const el = target ? addStatCardToLayout(target, { sheet: p.sheet.id, card: p.card.id, part }) : null;
  if (!el) {
    status("That layout could not be found.");
    return null;
  }
  const name = useLayouts.getState().list().find((l) => l.id === target)?.name ?? "the layout";
  status(`Added the ${part} to ${name}. It follows the card: run the card again to update the page.`);
  return el;
}
