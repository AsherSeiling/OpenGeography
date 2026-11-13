// A chart on a card: the engine's SVG (one renderer for the sheet, layouts and exports) inserted after parsing and stripping anything that could run.
// Click a mark to select its features through the shared selection store, as a click on the map does (Shift adds, Ctrl or Cmd toggles); a selection
// made on the map puts a `sel` class on the matching marks. Neither direction draws the chart again.
import { useEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "../ipc/api";
import { useApp } from "../store/store";
import { applySelection, modeFor, sanitizeSvg, selectableMarks } from "./chartLink";
import type { ChartSpec, StatCard } from "./types";

export const CHART_W = 720;
export const CHART_H = 400;

export function ChartView({ card, spec }: { card: StatCard; spec: ChartSpec }) {
  const host = useRef<HTMLDivElement>(null);
  const [error, setError] = useState("");
  const marks = useMemo(() => selectableMarks(spec), [spec]);
  const layer = card.inputs.in?.layer ?? Object.values(card.inputs)[0]?.layer;
  const specKey = useMemo(() => JSON.stringify(spec), [spec]);

  // Draw: ask the engine for the SVG, make it safe, insert it, and mark what the map has selected right now.
  useEffect(() => {
    let live = true;
    api.statChartSvg(spec, CHART_W, CHART_H).then(
      (text) => {
        if (!live || !host.current) return;
        const svg = sanitizeSvg(text);
        if (!svg) return setError("The chart could not be drawn.");
        host.current.replaceChildren(document.importNode(svg, true));
        setError("");
        if (layer) applySelection(host.current, spec, new Set(useApp.getState().selection[layer] ?? []));
      },
      (e) => live && setError(errorText(e)),
    );
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [specKey]);

  // The map's selection changes: toggle classes on the marks already there.
  useEffect(() => {
    if (!layer) return;
    let last: number[] | undefined;
    return useApp.subscribe((s) => {
      const sel = s.selection[layer];
      if (sel === last || !host.current) return;
      last = sel;
      applySelection(host.current, spec, new Set(sel ?? []));
    });
  }, [layer, spec]);

  const click = async (e: React.MouseEvent) => {
    const el = (e.target as Element).closest?.("[data-mark]");
    if (!el || !layer) return;
    e.stopPropagation();
    const mark = marks[Number(el.getAttribute("data-mark"))];
    if (!mark) return;
    try {
      const fids = mark.fids?.length ? mark.fids : mark.bin && !mark.bin.startsWith("pt:") ? await api.statChartBinFids(card, mark.bin) : [];
      if (fids.length === 0) {
        useApp.getState().setStatus("That part of the chart does not stand for features, so there is nothing to select.");
        return;
      }
      useApp.getState().setSelection(layer, fids, modeFor(e));
      useApp.getState().setStatus(`Selected ${fids.length.toLocaleString()} feature${fids.length === 1 ? "" : "s"} from the chart.`);
    } catch (err) {
      useApp.getState().setStatus(`The chart could not select those features: ${errorText(err)}`);
    }
  };

  return (
    <div className="stat-chart-wrap">
      {error && <div className="stat-banner error">{error}</div>}
      <div ref={host} className="stat-chart-host" data-testid="stat-chart" onClick={click} />
    </div>
  );
}
