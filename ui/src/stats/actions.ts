// Statistics sheet actions shared by the ribbon, the Catalog and the sheet view: open, create, rename, delete, add and run cards.
import { api, errorText, type ToolSpec } from "../ipc/api";
import { reportError } from "../errors/store";
import { isCancelled } from "../progress/progress";
import { leaveWorkspace } from "../layout/actions";
import { anchorMapPanel, getDock } from "../panes/panes";
import { useRun } from "../store/model";
import { useApp } from "../store/store";
import { projectLayerInfos } from "../store/maps";
import { cardKey, newSheetDoc, newStatId, useStats } from "./store";
import { CATEGORY_PREFIX, type StatCard } from "./types";

const status = (s: string) => useApp.getState().setStatus(s);
export const statsPanelId = (id: string) => `stats:${id}`;

/** Show a sheet's tab, opening it beside the Map if needed. */
export function openStatsSheet(id: string) {
  const dock = getDock();
  const sheet = useStats.getState().docs[id];
  if (!dock || !sheet) return;
  const existing = dock.getPanel(statsPanelId(id));
  if (existing) return existing.api.setActive();
  dock.addPanel({
    id: statsPanelId(id),
    component: "stats",
    title: sheet.name,
    params: { statsId: id },
    position: anchorMapPanel() ? { referencePanel: anchorMapPanel()!.id, direction: "within" } : undefined,
  });
}

export function newStatsSheet() {
  const names = new Set(useStats.getState().list().map((s) => s.name));
  let n = 1;
  while (names.has(`Statistics ${n}`)) n++;
  const sheet = newSheetDoc(`Statistics ${n}`);
  useStats.getState().add(sheet);
  openStatsSheet(sheet.id);
  status(`Created ${sheet.name}. Add a card from the Explore tab, then run it.`);
}

export function renameStatsSheet(id: string) {
  const cur = useStats.getState().docs[id];
  const name = cur && window.prompt("Statistics sheet name", cur.name)?.trim();
  if (!name) return;
  useStats.getState().rename(id, name);
  getDock()?.getPanel(statsPanelId(id))?.api.setTitle(name);
}

export function deleteStatsSheet(id: string) {
  const cur = useStats.getState().docs[id];
  if (!cur || !window.confirm(`Delete statistics sheet '${cur.name}'? This cannot be undone.`)) return;
  getDock()?.getPanel(statsPanelId(id))?.api.close();
  useStats.getState().remove(id);
}

/** The tools a card can use: every ToolSpec whose category starts with "Statistics ›". Same specs as the node tree; no second list. */
export function statTools(specs: Record<string, ToolSpec>): ToolSpec[] {
  return Object.values(specs)
    .filter((s) => s.category.startsWith(CATEGORY_PREFIX))
    .sort((a, b) => a.category.localeCompare(b.category) || a.name.localeCompare(b.name));
}

/** Layers and tables a card can read: not rasters, not temporary previews. */
export function statLayers() {
  return projectLayerInfos(useApp.getState()).filter((l) => l && !l.temporary && l.geomType !== "Raster");
}

/** Add a card of `toolId` to the open sheet, on the active layer (else the first layer), with the tool's default settings. */
export function addStatCard(toolId: string) {
  const { id: sheetId, selectedOnly } = useStats.getState();
  const spec = useRun.getState().specs[toolId];
  if (!sheetId || !spec) return;
  const app = useApp.getState();
  const layers = statLayers();
  const pick = layers.find((l) => l.id === app.activeLayer) ?? layers[0];
  const inputs: StatCard["inputs"] = {};
  if (pick) {
    const sel = app.selection[pick.id] ?? [];
    for (const port of spec.inputs) inputs[port.id] = { layer: pick.id, fids: selectedOnly && sel.length ? [...sel] : null };
  }
  const options: Record<string, unknown> = {};
  for (const p of spec.params) if (!p.hidden && p.default !== null && p.default !== undefined) options[p.id] = p.default;
  useStats.getState().addCard(sheetId, { id: newStatId("c"), tool: toolId, title: null, inputs, options, result: null });
  status(pick ? `Added ${spec.name}. Check its settings in Properties, then run it.` : `Added ${spec.name}. There is no layer or table in the project yet to run it on.`);
}

/** What stops a card from running, in a sentence, or null. Checked here so a mistake in the settings never reaches the engine. */
export function cardProblem(card: StatCard): string | null {
  const spec = useRun.getState().specs[card.tool];
  if (!spec) return `The tool '${card.tool}' is not available in this version of GIS Studio.`;
  const layers = useApp.getState().layers;
  for (const port of spec.inputs) {
    const input = card.inputs[port.id];
    if (!input?.layer) return `Choose a layer or table for ${port.label}.`;
    if (!layers[input.layer]) return `The layer chosen for ${port.label} is no longer in the project. Choose another.`;
  }
  for (const p of spec.params) {
    const v = card.options[p.id];
    if (!p.optional && !p.hidden && p.kind === "field" && (v === undefined || v === null || v === "")) return `Choose a field for ${p.label}.`;
  }
  return null;
}

export async function runStatCard(sheetId: string, cardId: string): Promise<boolean> {
  const st = useStats.getState();
  const card = st.docs[sheetId]?.cards.find((c) => c.id === cardId);
  if (!card) return false;
  const key = cardKey(sheetId, cardId);
  if (st.running[key]) return false;
  const problem = cardProblem(card);
  if (problem) {
    st.setError(key, problem);
    status(problem);
    return false;
  }
  st.setError(key, null);
  st.setRunning(key, true);
  try {
    const result = await api.runStatCard(card);
    // Settings edited while it ran: these numbers belong to the old settings.
    const now = useStats.getState().docs[sheetId]?.cards.find((c) => c.id === cardId);
    if (!now) return false;
    if (JSON.stringify([now.inputs, now.options]) !== JSON.stringify([card.inputs, card.options])) {
      useStats.getState().setError(key, "The settings changed while this ran, so the result was not kept. Run it again.");
      return false;
    }
    useStats.getState().setResult(sheetId, cardId, result);
    status(`${useRun.getState().specs[card.tool]?.name ?? "Card"} finished.`);
    return true;
  } catch (e) {
    if (isCancelled(e)) {
      useStats.getState().setError(key, "Cancelled.");
      status("Cancelled.");
    } else {
      useStats.getState().setError(key, errorText(e));
      status(`The card could not run: ${errorText(e)}`);
      reportError(e, `stat-card:${key}`, { title: "The statistics card could not run", context: "running a statistics card" });
    }
    return false;
  } finally {
    useStats.getState().setRunning(key, false);
  }
}

/** Run every card of the open sheet, top to bottom (the user asked: nothing here runs by itself). */
export async function runAllStatCards() {
  const sheet = useStats.getState().id ? useStats.getState().docs[useStats.getState().id!] : null;
  if (!sheet) return;
  for (const c of sheet.cards) await runStatCard(sheet.id, c.id);
}

/** A dock panel was closed: if it was the open sheet's tab, no sheet is open any more, and with no other layout or sheet tab the window leaves the workspace. */
export function statsPanelClosed(sheetId: string | undefined) {
  if (!sheetId || useStats.getState().id !== sheetId) return;
  useStats.getState().open(null);
  if (!getDock()?.panels.some((p) => (p.id.startsWith("layout:") || p.id.startsWith("stats:")) && p.id !== statsPanelId(sheetId))) leaveWorkspace();
}

/** Read the project's statistics sheets from the engine; their tabs come back with the dock layout. */
export async function loadStatSheets() {
  try {
    useStats.getState().loadAll(await api.listStatSheets());
  } catch (e) {
    useStats.getState().loadAll([]);
    reportError(e, "action:Opening project", { title: "This project's statistics sheets could not be read", context: "reading statistics sheets" });
  }
}
