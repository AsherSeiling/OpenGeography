import { Button, Dropdown, Input, Option, ToggleButton, Tooltip } from "@fluentui/react-components";
import {
  ArrowSortDown16Regular,
  ArrowSortUp16Regular,
  DismissCircleRegular,
  FilterRegular,
  SelectAllOnRegular,
  ZoomFitRegular,
} from "@fluentui/react-icons";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDialog, zoomToSelection } from "../actions";
import { api, errorText, type RowsPage } from "../ipc/api";
import { displayInfo, useApp } from "../store/store";

const PAGE = 200;
const ROW_H = 24;
const COL_W = 140;
const FID_W = 64;

export function AttributeTable() {
  const order = useApp((s) => s.order);
  const layers = useApp((s) => s.layers);
  const active = useApp((s) => s.activeLayer);
  const owner = active && layers[active] ? active : order[0];
  const results = useApp((s) => s.results);
  const showOriginal = useApp((s) => s.showOriginal);
  // Rows come from what the layer currently shows (node-tree result or original).
  const info = owner ? displayInfo({ layers, results, showOriginal }, owner) : undefined;
  const id = info?.id;
  const selected = useApp((s) => (owner ? s.selection[owner] : undefined));
  const [sort, setSort] = useState<{ field: string; desc: boolean } | null>(null);
  const [filterText, setFilterText] = useState("");
  const [filter, setFilter] = useState("");
  const [onlySelected, setOnlySelected] = useState(false);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const pages = useRef(new Map<number, RowsPage>());
  const loading = useRef(new Set<number>());
  const [, force] = useState(0);
  const scrollRef = useRef<HTMLDivElement>(null);
  const anchor = useRef<number | null>(null);
  const currentKey = useRef("");

  const selSet = useMemo(() => new Set(selected ?? []), [selected]);
  const fidsFilter = onlySelected ? (selected ?? []) : null;
  const queryKey = JSON.stringify([id, info?.version, sort, filter, onlySelected ? fidsFilter : null]);

  // Reset paging whenever the query changes.
  useEffect(() => {
    currentKey.current = queryKey;
    pages.current.clear();
    loading.current.clear();
    setError(null);
    if (!id) {
      setTotal(0);
      return;
    }
    loadPage(0);
    scrollRef.current?.scrollTo({ top: 0 });
  }, [queryKey]);

  const loadPage = useCallback(
    async (p: number) => {
      if (!id || pages.current.has(p) || loading.current.has(p)) return;
      loading.current.add(p);
      const key = queryKey;
      try {
        const page = await api.getRows(id, { offset: p * PAGE, limit: PAGE, sort: sort?.field, desc: sort?.desc, filter: filter || null, fids: fidsFilter });
        if (key !== currentKey.current) return; // query changed while loading
        pages.current.set(p, page);
        setTotal(page.total);
        force((n) => n + 1);
      } catch (e) {
        setError(errorText(e).split("\n")[0]);
      } finally {
        loading.current.delete(p);
      }
    },
    [queryKey],
  );

  const virt = useVirtualizer({ count: total, getScrollElement: () => scrollRef.current, estimateSize: () => ROW_H, overscan: 20 });
  const items = virt.getVirtualItems();
  useEffect(() => {
    if (!items.length) return;
    const first = Math.floor(items[0].index / PAGE);
    const last = Math.floor(items[items.length - 1].index / PAGE);
    for (let p = first; p <= last; p++) loadPage(p);
  }, [items, loadPage]);

  const rowAt = (i: number) => {
    const page = pages.current.get(Math.floor(i / PAGE));
    if (!page) return null;
    const j = i % PAGE;
    return { fid: page.fids[j], values: page.rows[j] };
  };

  const clickRow = (i: number, fid: number, e: React.MouseEvent) => {
    const s = useApp.getState();
    const id = owner;
    if (!id) return;
    if (e.shiftKey && anchor.current !== null) {
      const [a, b] = [Math.min(anchor.current, i), Math.max(anchor.current, i)];
      const fids: number[] = [];
      for (let k = a; k <= b; k++) {
        const r = rowAt(k);
        if (r) fids.push(r.fid);
      }
      s.setSelection(id, fids, "add");
    } else if (e.ctrlKey || e.metaKey) {
      s.setSelection(id, [fid], "toggle");
      anchor.current = i;
    } else {
      s.setSelection(id, [fid], "new");
      anchor.current = i;
    }
  };

  if (!info) return <div className="empty-hint pad">No layers. Add data to see its attributes here.</div>;

  const width = FID_W + info.fields.length * COL_W;
  const nsel = selected?.length ?? 0;

  return (
    <div className="attr">
      <div className="attr-bar">
        <Dropdown size="small" value={info.name + (info.resultOf ? " (node result)" : "")} onOptionSelect={(_, d) => useApp.getState().setActiveLayer(d.optionValue!)} style={{ minWidth: 140 }}>
          {order.map((lid) => (
            <Option key={lid} value={lid}>{layers[lid]?.name ?? lid}</Option>
          ))}
        </Dropdown>
        <Tooltip content="Select By Attributes" relationship="label">
          <Button size="small" icon={<SelectAllOnRegular />} onClick={() => useDialog.getState().show({ kind: "selectByAttributes", layer: owner! })} />
        </Tooltip>
        <Tooltip content="Zoom To Selection" relationship="label">
          <Button size="small" icon={<ZoomFitRegular />} disabled={!nsel} onClick={zoomToSelection} />
        </Tooltip>
        <Tooltip content="Clear Selection" relationship="label">
          <Button size="small" icon={<DismissCircleRegular />} disabled={!nsel} onClick={() => useApp.getState().setSelection(owner!, [])} />
        </Tooltip>
        <ToggleButton size="small" checked={onlySelected} onClick={() => setOnlySelected(!onlySelected)}>
          {onlySelected ? "Showing selected" : "Show selected"}
        </ToggleButton>
        <Input
          size="small"
          contentBefore={<FilterRegular />}
          placeholder="Filter, e.g. POP > 1000"
          value={filterText}
          onChange={(_, d) => setFilterText(d.value)}
          onKeyDown={(e) => e.key === "Enter" && setFilter(filterText)}
          onBlur={() => setFilter(filterText)}
          style={{ flex: 1, minWidth: 120 }}
        />
      </div>
      {error && <div className="attr-error">{error}</div>}
      <div className="grid" ref={scrollRef}>
        <div style={{ width, minWidth: "100%" }}>
          <div className="grid-head" style={{ width }}>
            <div className="cell fid" style={{ width: FID_W }}>FID</div>
            {info.fields.map((f) => (
              <div
                key={f.name}
                className="cell head"
                style={{ width: COL_W }}
                title={`${f.name} (${f.type}) — click to sort`}
                onClick={() => setSort(sort?.field === f.name ? (sort.desc ? null : { field: f.name, desc: true }) : { field: f.name, desc: false })}
              >
                <span>{f.name}</span>
                {sort?.field === f.name && (sort.desc ? <ArrowSortDown16Regular /> : <ArrowSortUp16Regular />)}
              </div>
            ))}
          </div>
          <div style={{ height: virt.getTotalSize(), position: "relative" }}>
            {items.map((it) => {
              const r = rowAt(it.index);
              const isSel = r ? selSet.has(r.fid) : false;
              return (
                <div
                  key={it.key}
                  className={`grid-row ${isSel ? "sel" : ""} ${it.index % 2 ? "odd" : ""}`}
                  style={{ transform: `translateY(${it.start}px)`, height: ROW_H, width }}
                  onClick={(e) => r && clickRow(it.index, r.fid, e)}
                >
                  <div className="cell fid" style={{ width: FID_W }}>{r?.fid ?? ""}</div>
                  {info.fields.map((f, k) => {
                    const v = r?.values[k];
                    return (
                      <div key={f.name} className={`cell ${isNumeric(f.type) ? "num" : ""}`} style={{ width: COL_W }}>
                        {r ? (v === null ? <span className="null">&lt;Null&gt;</span> : v) : ""}
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        </div>
      </div>
      <div className="attr-foot">
        {nsel.toLocaleString()} of {info.featureCount.toLocaleString()} selected
        {filter || onlySelected ? ` · ${total.toLocaleString()} shown` : ""}
      </div>
    </div>
  );
}

function isNumeric(t: string) {
  return /INT|DOUBLE|FLOAT|DECIMAL|REAL|NUMERIC|BIGINT|HUGEINT/i.test(t);
}
