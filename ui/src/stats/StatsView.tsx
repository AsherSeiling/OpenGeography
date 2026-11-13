// The statistics sheet (centre of the workspace): a scrolling column of analysis cards. A card shows its inputs, the method in words, how many
// rows were used and dropped and why, its result tables and its notes, and says plainly when its numbers are from before a change to the data.
import { Button } from "@fluentui/react-components";
import { useEffect, useMemo } from "react";
import { useRun } from "../store/model";
import { useApp } from "../store/store";
import { runStatCard } from "./actions";
import { cardState, staleSentence } from "./staleness";
import { cardKey, useStats } from "./store";
import type { StatCard, StatResult, StatTable } from "./types";

const MAX_SHOWN_ROWS = 200;

/** A number for reading: whole numbers as they are, others to 6 significant digits. The stored value keeps full precision. */
export function fmt(v: number | string | boolean | null): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "number") return Number.isInteger(v) ? String(v) : String(Number(v.toPrecision(6)));
  return String(v);
}

export function StatsView({ sheetId }: { sheetId: string }) {
  const sheet = useStats((s) => s.docs[sheetId]);
  const selected = useStats((s) => s.selected);
  // Properties follows the selected card.
  useEffect(() => {
    const app = useApp.getState();
    if (selected && sheet?.cards.some((c) => c.id === selected)) app.setProperties({ kind: "statCard", sheet: sheetId, id: selected });
    else if (app.properties?.kind === "statCard" && app.properties.sheet === sheetId) app.setProperties(null);
  }, [selected, sheetId, sheet]);
  if (!sheet) return <div className="empty-hint pad">This statistics sheet no longer exists.</div>;
  return (
    <div className="stats-view" data-testid="stats-view" onClick={() => useStats.getState().select(null)}>
      <div className="stats-col">
        <div className="stats-head">
          <h2>{sheet.name}</h2>
          <span className="muted">{sheet.cards.length} card{sheet.cards.length === 1 ? "" : "s"}</span>
        </div>
        {sheet.cards.length === 0 && (
          <div className="stats-empty">
            No cards yet. On the Explore tab, Add Card lists the statistics tools. Each card is one analysis of a layer or table, and keeps its result in the project.
          </div>
        )}
        {sheet.cards.map((c) => (
          <CardView key={c.id} sheetId={sheetId} card={c} />
        ))}
      </div>
    </div>
  );
}

function CardView({ sheetId, card }: { sheetId: string; card: StatCard }) {
  const key = cardKey(sheetId, card.id);
  const spec = useRun((s) => s.specs[card.tool]);
  const on = useStats((s) => s.selected === card.id);
  const running = useStats((s) => !!s.running[key]);
  const error = useStats((s) => s.errors[key]);
  const layers = useApp((s) => s.layers); // the store's own object: a selector that built a new one each time would re-render forever
  const state = useMemo(() => cardState(card, layers), [card, layers]);
  const name = (id: string) => layers[id]?.name ?? id;
  const inputs = Object.entries(card.inputs);
  const stale = state.state === "out-of-date";
  return (
    <div className={`stat-card${on ? " on" : ""}${stale ? " stale" : ""}`} data-testid="stat-card" data-state={state.state} onClick={(e) => (e.stopPropagation(), useStats.getState().select(card.id))}>
      <div className="stat-card-head">
        <span className="stat-card-title">{card.title || spec?.name || card.tool}</span>
        <span className={`stat-badge ${stale ? "stale" : state.state === "current" ? "current" : ""}`}>{running ? "Running" : state.state === "out-of-date" ? "Out of date" : state.state === "current" ? "Up to date" : "Not run"}</span>
        <div className="stat-card-actions">
          <Button size="small" appearance="primary" disabled={running} onClick={(e) => (e.stopPropagation(), void runStatCard(sheetId, card.id))}>
            {card.result ? "Run again" : "Run"}
          </Button>
          <Button size="small" onClick={(e) => (e.stopPropagation(), useStats.getState().removeCard(sheetId, card.id))}>Remove</Button>
        </div>
      </div>
      <div className="stat-card-body">
        <div className="stat-meta">
          {inputs.length === 0
            ? "No layer chosen yet. Choose one in Properties."
            : inputs.map(([port, i]) => `${spec?.inputs.find((p) => p.id === port)?.label ?? port}: ${name(i.layer)}${i.fids ? ` (${i.fids.length.toLocaleString()} selected features only)` : ""}`).join(" · ")}
        </div>
        {stale && <div className="stat-banner" role="status">{staleSentence(state, name)}</div>}
        {error && <div className="stat-banner error" role="alert">{error}</div>}
        {card.result ? <ResultView result={card.result} /> : !error && <div className="stat-meta">{running ? "Running…" : "Not run yet. Run it to see the numbers."}</div>}
      </div>
    </div>
  );
}

function countsLine(r: StatResult): string[] {
  const out: string[] = [r.n_used === null ? "n used: not reported by this tool" : `n used: ${r.n_used.toLocaleString()}`];
  const dropped = r.dropped.reduce((a, d) => a + d.count, 0);
  if (r.n_used !== null || dropped > 0) out.push(dropped === 0 ? "dropped: 0" : `dropped: ${dropped.toLocaleString()} (${r.dropped.map((d) => `${d.count.toLocaleString()} ${d.reason}`).join("; ")})`);
  if (r.seed !== null) out.push(`random seed: ${r.seed}`);
  return out;
}

function ResultView({ result }: { result: StatResult }) {
  return (
    <>
      <div className="stat-method">{result.method}</div>
      <div className="stat-counts">
        {countsLine(result).map((t) => (
          <span key={t}>{t}</span>
        ))}
      </div>
      {result.tables.map((t, i) => (
        <TableView key={i} table={t} />
      ))}
      {result.charts.length > 0 && <div className="stat-meta">{result.charts.length} chart{result.charts.length === 1 ? "" : "s"} in this result: {result.charts.map((c) => c.title || c.kind).join(", ")}. Charts are drawn by a later step of the statistics workspace.</div>}
      {result.notes.length > 0 && (
        <ul className="stat-notes">
          {result.notes.map((n, i) => (
            <li key={i}>{n}</li>
          ))}
        </ul>
      )}
      {result.citations.length > 0 && (
        <details className="stat-cite">
          <summary>Citations ({result.citations.length})</summary>
          <ul className="stat-notes">
            {result.citations.map((c, i) => (
              <li key={i}>{c}</li>
            ))}
          </ul>
        </details>
      )}
    </>
  );
}

function TableView({ table }: { table: StatTable }) {
  const shown = table.rows.slice(0, MAX_SHOWN_ROWS);
  return (
    <div className="stat-table-wrap">
      <table className="stat-table">
        <caption>{table.title}</caption>
        <thead>
          <tr>
            {table.columns.map((c) => (
              <th key={c.name}>{c.unit ? `${c.name} (${c.unit})` : c.name}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {shown.map((r, i) => (
            <tr key={i}>
              {r.map((v, j) => (
                <td key={j} className={table.columns[j]?.kind === "number" || table.columns[j]?.kind === "integer" ? "num" : ""}>
                  {fmt(v)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {table.rows.length > shown.length && <div className="stat-meta" style={{ padding: 6 }}>Showing the first {shown.length} of {table.rows.length.toLocaleString()} rows.</div>}
    </div>
  );
}
