// Spreadsheet-style attribute table: virtual scrolling over any number of
// rows, in-place cell editing with keyboard navigation, field tools
// (add / calculate / calculate geometry / rename / delete) and feature
// selection from the row headers.
import { Button, Dropdown, Input, Menu, MenuDivider, MenuItem, MenuList, MenuPopover, Option, ToggleButton, Tooltip } from "@fluentui/react-components";
import {
  AddRegular,
  ArrowExportRegular,
  ArrowSortDown16Regular,
  ArrowSortDownRegular,
  ArrowSortUp16Regular,
  ArrowSortUpRegular,
  ArrowUndoRegular,
  CalculatorRegular,
  DeleteRegular,
  DismissCircleRegular,
  EditRegular,
  FilterRegular,
  RulerRegular,
  SelectAllOnRegular,
  ZoomFitRegular,
} from "@fluentui/react-icons";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { exportLayer, useDialog, zoomToSelection } from "../actions";
import { api, errorText, type LayerInfo, type RowsPage } from "../ipc/api";
import { displayInfo, useApp } from "../store/store";

const PAGE = 200;
const ROW_H = 24;
const DEFAULT_W = 130;
const FID_W = 64;

interface Cell {
  row: number;
  col: number;
}

interface CellEdit {
  layer: string;
  fid: number;
  field: string;
  before: string | null;
  after: string | null;
}

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
  const readOnly = !!info?.resultOf;
  const selected = useApp((s) => (owner ? s.selection[owner] : undefined));
  const [sort, setSort] = useState<{ field: string; desc: boolean } | null>(null);
  const [filterText, setFilterText] = useState("");
  const [filter, setFilter] = useState("");
  const [onlySelected, setOnlySelected] = useState(false);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [cell, setCell] = useState<Cell | null>(null);
  const [editing, setEditingState] = useState<{ cell: Cell; text: string } | null>(null);
  // Mirror of `editing` that updates immediately, so fast typing reaches the
  // edit in progress even before the cell editor has rendered.
  const editingRef = useRef<{ cell: Cell; text: string } | null>(null);
  const setEditing = (v: { cell: Cell; text: string } | null) => {
    editingRef.current = v;
    setEditingState(v);
  };
  const [widths, setWidths] = useState<Record<string, number>>({});
  const [headerMenu, setHeaderMenu] = useState<{ x: number; y: number; field: string } | null>(null);
  const undo = useRef<CellEdit[]>([]);
  const pages = useRef(new Map<number, RowsPage>());
  const loading = useRef(new Set<number>());
  const [, force] = useState(0);
  const scrollRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const anchor = useRef<number | null>(null);
  const currentKey = useRef("");

  const selSet = useMemo(() => new Set(selected ?? []), [selected]);
  const fidsFilter = onlySelected ? (selected ?? []) : null;
  // Changing these starts over at the top; a version bump (an edit) keeps the scroll position.
  const viewKey = JSON.stringify([id, sort, filter, onlySelected ? fidsFilter : null]);
  const queryKey = JSON.stringify([viewKey, info?.version]);
  const fields = info?.fields ?? [];
  const colW = (f: string) => widths[`${id}:${f}`] ?? DEFAULT_W;

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: 0 });
    setCell(null);
    setEditing(null);
  }, [viewKey]);

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
    force((n) => n + 1);
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

  const setLocal = (row: number, col: number, value: string | null) => {
    const page = pages.current.get(Math.floor(row / PAGE));
    if (page) page.rows[row % PAGE][col] = value;
  };

  const applied = (layer: LayerInfo) => useApp.getState().updateLayer(layer);
  const status = (s: string) => useApp.getState().setStatus(s);

  // ---------- selection (row headers) ----------

  const clickRowHeader = (i: number, fid: number, e: React.MouseEvent) => {
    const s = useApp.getState();
    if (!owner) return;
    if (e.shiftKey && anchor.current !== null) {
      const [a, b] = [Math.min(anchor.current, i), Math.max(anchor.current, i)];
      const fids: number[] = [];
      for (let k = a; k <= b; k++) {
        const r = rowAt(k);
        if (r) fids.push(r.fid);
      }
      s.setSelection(owner, fids, "add");
    } else if (e.ctrlKey || e.metaKey) {
      s.setSelection(owner, [fid], "toggle");
      anchor.current = i;
    } else {
      s.setSelection(owner, [fid], "new");
      anchor.current = i;
    }
  };

  // ---------- editing ----------

  const commit = async (target: Cell, text: string) => {
    if (!id || readOnly) return;
    const r = rowAt(target.row);
    const f = fields[target.col];
    if (!r || !f) return;
    const before = r.values[target.col];
    const after = text === "" && !/VARCHAR/i.test(f.type) ? null : text;
    if (before === after) return;
    setLocal(target.row, target.col, after);
    force((n) => n + 1);
    try {
      applied(await api.updateCell(id, r.fid, f.name, after));
      undo.current.push({ layer: id, fid: r.fid, field: f.name, before, after });
      if (undo.current.length > 200) undo.current.shift();
    } catch (e) {
      setLocal(target.row, target.col, before);
      force((n) => n + 1);
      status(errorText(e));
    }
  };

  const undoEdit = async () => {
    const last = undo.current.pop();
    if (!last) {
      status("Nothing to undo");
      return;
    }
    try {
      applied(await api.updateCell(last.layer, last.fid, last.field, last.before));
      status(`Undid edit to ${last.field}`);
    } catch (e) {
      status(errorText(e));
    }
  };

  const startEdit = (c: Cell, initial?: string) => {
    if (readOnly) {
      status("This is a node-tree result (read-only). Show the original data to edit it.");
      return;
    }
    const r = rowAt(c.row);
    if (!r) return;
    setEditing({ cell: c, text: initial ?? r.values[c.col] ?? "" });
  };

  const move = (c: Cell, dr: number, dc: number): Cell => ({
    row: Math.max(0, Math.min(total - 1, c.row + dr)),
    col: Math.max(0, Math.min(fields.length - 1, c.col + dc)),
  });

  const focusCell = (c: Cell) => {
    setCell(c);
    virt.scrollToIndex(c.row, { align: "auto" });
    requestAnimationFrame(() => gridRef.current?.focus({ preventScroll: true }));
  };

  const onGridKey = async (e: React.KeyboardEvent) => {
    const mod = e.ctrlKey || e.metaKey;
    const pending = editingRef.current;
    if (pending) {
      // Keys that arrive before the editor is focused belong to the edit.
      if (e.target instanceof HTMLInputElement) return;
      e.preventDefault();
      if (e.key.length === 1 && !mod) setEditing({ ...pending, text: pending.text + e.key });
      else if (e.key === "Backspace") setEditing({ ...pending, text: pending.text.slice(0, -1) });
      else if (e.key === "Escape") setEditing(null);
      else if (e.key === "Enter" || e.key === "Tab") {
        setEditing(null);
        await commit(pending.cell, pending.text);
        focusCell(e.key === "Enter" ? move(pending.cell, 1, 0) : move(pending.cell, 0, 1));
      }
      return;
    }
    if (mod && e.key.toLowerCase() === "z") {
      e.preventDefault();
      undoEdit();
      return;
    }
    if (!cell) return;
    if (mod && e.key.toLowerCase() === "c") {
      const v = rowAt(cell.row)?.values[cell.col];
      navigator.clipboard?.writeText(v ?? "");
      status("Copied");
      return;
    }
    if (mod && e.key.toLowerCase() === "v") {
      e.preventDefault();
      const text = await navigator.clipboard?.readText().catch(() => "");
      if (text !== undefined) await commit(cell, text.replace(/\r?\n$/, ""));
      return;
    }
    const nav: Record<string, [number, number]> = { ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1], Enter: [1, 0], Tab: [0, e.shiftKey ? -1 : 1] };
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      startEdit(cell);
      return;
    }
    if (nav[e.key]) {
      e.preventDefault();
      focusCell(move(cell, ...nav[e.key]));
    } else if (e.key === "F2") {
      e.preventDefault();
      startEdit(cell);
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      await commit(cell, "");
    } else if (e.key.length === 1 && !mod) {
      e.preventDefault();
      startEdit(cell, e.key); // typing replaces the value, as in Excel
    }
  };

  const onEditorKey = async (e: React.KeyboardEvent<HTMLInputElement>) => {
    const cur = editingRef.current;
    if (!cur) return;
    if (e.key === "Escape") {
      e.preventDefault();
      setEditing(null);
      focusCell(cur.cell);
    } else if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      const c = cur.cell;
      setEditing(null);
      await commit(c, cur.text);
      focusCell(e.key === "Enter" ? move(c, e.shiftKey ? -1 : 1, 0) : move(c, 0, e.shiftKey ? -1 : 1));
    }
  };

  // ---------- column resize ----------

  const startResize = (field: string, e: React.PointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const x0 = e.clientX;
    const w0 = colW(field);
    const mv = (ev: PointerEvent) => setWidths((w) => ({ ...w, [`${id}:${field}`]: Math.max(50, w0 + ev.clientX - x0) }));
    const up = () => {
      window.removeEventListener("pointermove", mv, true);
      window.removeEventListener("pointerup", up, true);
    };
    window.addEventListener("pointermove", mv, true);
    window.addEventListener("pointerup", up, true);
  };

  if (!info || !owner) return <div className="empty-hint pad">No layers. Add data to see its attributes here.</div>;

  const width = FID_W + fields.reduce((n, f) => n + colW(f.name), 0);
  const nsel = selected?.length ?? 0;
  const dlg = useDialog.getState().show;

  return (
    <div className="attr">
      <div className="attr-bar">
        <Dropdown size="small" value={info.name + (readOnly ? " (node result)" : "")} onOptionSelect={(_, d) => useApp.getState().setActiveLayer(d.optionValue!)} style={{ minWidth: 140 }}>
          {order.map((lid) => (
            <Option key={lid} value={lid}>{layers[lid]?.name ?? lid}</Option>
          ))}
        </Dropdown>
        <div className="attr-sep" />
        <Tooltip content="Add Field" relationship="label">
          <Button size="small" icon={<AddRegular />} disabled={readOnly} onClick={() => dlg({ kind: "addField", layer: id! })}>Field</Button>
        </Tooltip>
        <Tooltip content="Calculate Field" relationship="label">
          <Button size="small" icon={<CalculatorRegular />} disabled={readOnly} onClick={() => dlg({ kind: "calcField", layer: id!, owner })} />
        </Tooltip>
        <Tooltip content="Calculate Geometry (area, length, coordinates)" relationship="label">
          <Button size="small" icon={<RulerRegular />} disabled={readOnly || info.geomType === "Table"} onClick={() => dlg({ kind: "calcGeometry", layer: id!, owner })} />
        </Tooltip>
        <Tooltip content={`Delete ${nsel} selected row(s)`} relationship="label">
          <Button
            size="small"
            icon={<DeleteRegular />}
            disabled={readOnly || !nsel}
            onClick={async () => {
              if (!confirm(`Delete ${nsel.toLocaleString()} selected row(s) from ${info.name}? This can't be undone.`)) return;
              try {
                const r = await api.deleteRows(id!, selected!);
                applied(r.layer);
                useApp.getState().setSelection(owner, []);
                status(`Deleted ${r.count.toLocaleString()} row(s)`);
              } catch (e) {
                status(errorText(e));
              }
            }}
          />
        </Tooltip>
        <Tooltip content="Undo last cell edit (Ctrl+Z)" relationship="label">
          <Button size="small" icon={<ArrowUndoRegular />} disabled={readOnly} onClick={undoEdit} />
        </Tooltip>
        <div className="attr-sep" />
        <Tooltip content="Select By Attributes" relationship="label">
          <Button size="small" icon={<SelectAllOnRegular />} onClick={() => dlg({ kind: "selectByAttributes", layer: owner })} />
        </Tooltip>
        <Tooltip content="Zoom To Selection" relationship="label">
          <Button size="small" icon={<ZoomFitRegular />} disabled={!nsel} onClick={zoomToSelection} />
        </Tooltip>
        <Tooltip content="Clear Selection" relationship="label">
          <Button size="small" icon={<DismissCircleRegular />} disabled={!nsel} onClick={() => useApp.getState().setSelection(owner, [])} />
        </Tooltip>
        <Tooltip content={nsel ? `Export (${nsel} selected or all)` : "Export features"} relationship="label">
          <Button size="small" icon={<ArrowExportRegular />} onClick={() => exportLayer(owner)} />
        </Tooltip>
        <ToggleButton size="small" checked={onlySelected} onClick={() => setOnlySelected(!onlySelected)}>
          {onlySelected ? "Selected" : "All rows"}
        </ToggleButton>
        <Input
          size="small"
          contentBefore={<FilterRegular />}
          placeholder="Filter, e.g. POP > 1000"
          value={filterText}
          onChange={(_, d) => setFilterText(d.value)}
          onKeyDown={(e) => e.key === "Enter" && setFilter(filterText)}
          onBlur={() => setFilter(filterText)}
          style={{ flex: 1, minWidth: 110 }}
        />
      </div>
      {readOnly && (
        <div className="attr-banner">
          Showing this layer's node-tree result (read-only).{" "}
          <button className="linkbtn" onClick={() => useApp.getState().setShowOriginal(owner, true)}>Show original data to edit</button>
        </div>
      )}
      {error && <div className="attr-error">{error}</div>}
      <div className="grid" ref={scrollRef}>
        <div style={{ width, minWidth: "100%" }} ref={gridRef} tabIndex={0} onKeyDown={onGridKey} className="grid-inner">
          <div className="grid-head" style={{ width }}>
            <div className="cell fid corner" style={{ width: FID_W }} title="Click a row number to select that feature">FID</div>
            {fields.map((f) => (
              <div
                key={f.name}
                className={`cell head ${cell && fields[cell.col]?.name === f.name ? "col-active" : ""}`}
                style={{ width: colW(f.name) }}
                title={`${f.name} (${f.type}) — click to sort, right-click for field tools`}
                onClick={() => setSort(sort?.field === f.name ? (sort.desc ? null : { field: f.name, desc: true }) : { field: f.name, desc: false })}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setHeaderMenu({ x: e.clientX, y: e.clientY, field: f.name });
                }}
              >
                <span className="head-name">{f.name}</span>
                <span className="head-type">{shortType(f.type)}</span>
                {sort?.field === f.name && (sort.desc ? <ArrowSortDown16Regular /> : <ArrowSortUp16Regular />)}
                <span className="col-resize" onPointerDown={(e) => startResize(f.name, e)} onClick={(e) => e.stopPropagation()} />
              </div>
            ))}
          </div>
          <div style={{ height: virt.getTotalSize(), position: "relative" }}>
            {items.map((it) => {
              const r = rowAt(it.index);
              const isSel = r ? selSet.has(r.fid) : false;
              return (
                <div key={it.key} className={`grid-row ${isSel ? "sel" : ""} ${it.index % 2 ? "odd" : ""}`} style={{ transform: `translateY(${it.start}px)`, height: ROW_H, width }}>
                  <div className={`cell fid ${isSel ? "sel" : ""}`} style={{ width: FID_W }} onClick={(e) => r && clickRowHeader(it.index, r.fid, e)}>
                    {r?.fid ?? ""}
                  </div>
                  {fields.map((f, k) => {
                    const isActive = cell?.row === it.index && cell?.col === k;
                    const isEditing = editing?.cell.row === it.index && editing?.cell.col === k;
                    const v = r?.values[k];
                    return (
                      <div
                        key={f.name}
                        className={`cell ${isNumeric(f.type) ? "num" : ""} ${isActive ? "active" : ""} ${readOnly ? "ro" : ""}`}
                        style={{ width: colW(f.name) }}
                        onMouseDown={(e) => {
                          if (isEditing) return;
                          e.preventDefault();
                          focusCell({ row: it.index, col: k });
                        }}
                        onDoubleClick={() => startEdit({ row: it.index, col: k })}
                      >
                        {isEditing ? (
                          <input
                            className="cell-editor"
                            autoFocus
                            onFocus={(e) => e.target.setSelectionRange(e.target.value.length, e.target.value.length)}
                            value={editing!.text}
                            onChange={(e) => setEditing({ ...editingRef.current!, text: e.target.value })}
                            onKeyDown={onEditorKey}
                            onBlur={() => {
                              const cur = editingRef.current;
                              if (!cur) return;
                              setEditing(null);
                              commit(cur.cell, cur.text);
                            }}
                          />
                        ) : r ? (
                          v === null ? <span className="null">&lt;Null&gt;</span> : v
                        ) : (
                          ""
                        )}
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
        <span>
          {nsel.toLocaleString()} of {(layers[owner]?.definitionQuery && !readOnly ? (layers[owner]?.queryCount ?? 0) : info.featureCount).toLocaleString()} selected
          {layers[owner]?.definitionQuery && !readOnly ? " (definition query)" : ""}
          {filter || onlySelected ? ` · ${total.toLocaleString()} shown` : ""}
        </span>
        <span className="muted">{readOnly ? "Read-only" : "Double-click or type to edit · Enter/Tab to move · Ctrl+Z to undo"}</span>
      </div>
      {headerMenu && (
        <HeaderMenu
          {...headerMenu}
          readOnly={readOnly}
          isGeo={info.geomType !== "Table"}
          onSort={(desc) => setSort({ field: headerMenu.field, desc })}
          onCalc={() => dlg({ kind: "calcField", layer: id!, owner, field: headerMenu.field })}
          onGeo={() => dlg({ kind: "calcGeometry", layer: id!, owner, field: headerMenu.field })}
          onRename={() => dlg({ kind: "renameField", layer: id!, field: headerMenu.field })}
          onDelete={async () => {
            if (!confirm(`Delete field "${headerMenu.field}" from ${info.name}? This can't be undone.`)) return;
            try {
              applied(await api.deleteField(id!, headerMenu.field));
              status(`Deleted field ${headerMenu.field}`);
            } catch (e) {
              status(errorText(e));
            }
          }}
          onClose={() => setHeaderMenu(null)}
        />
      )}
    </div>
  );
}

function HeaderMenu(p: {
  x: number;
  y: number;
  field: string;
  readOnly: boolean;
  isGeo: boolean;
  onSort: (desc: boolean) => void;
  onCalc: () => void;
  onGeo: () => void;
  onRename: () => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  const target = { getBoundingClientRect: () => ({ x: p.x, y: p.y, left: p.x, top: p.y, right: p.x, bottom: p.y, width: 0, height: 0, toJSON: () => null }) };
  return (
    <Menu open positioning={{ target: target as never, position: "below", align: "start" }} onOpenChange={(_, d) => !d.open && p.onClose()}>
      <MenuPopover>
        <MenuList>
          <MenuItem icon={<ArrowSortUpRegular />} onClick={() => p.onSort(false)}>Sort Ascending</MenuItem>
          <MenuItem icon={<ArrowSortDownRegular />} onClick={() => p.onSort(true)}>Sort Descending</MenuItem>
          <MenuDivider />
          <MenuItem icon={<CalculatorRegular />} disabled={p.readOnly} onClick={p.onCalc}>Calculate Field…</MenuItem>
          <MenuItem icon={<RulerRegular />} disabled={p.readOnly || !p.isGeo} onClick={p.onGeo}>Calculate Geometry…</MenuItem>
          <MenuDivider />
          <MenuItem icon={<EditRegular />} disabled={p.readOnly} onClick={p.onRename}>Rename Field…</MenuItem>
          <MenuItem icon={<DeleteRegular />} disabled={p.readOnly} onClick={p.onDelete}>Delete Field</MenuItem>
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

function isNumeric(t: string) {
  return /INT|DOUBLE|FLOAT|DECIMAL|REAL|NUMERIC|BIGINT|HUGEINT/i.test(t);
}

function shortType(t: string) {
  const u = t.toUpperCase();
  if (/INT/.test(u)) return "123";
  if (/DOUBLE|FLOAT|DECIMAL|REAL|NUMERIC/.test(u)) return "1.5";
  if (/DATE|TIME/.test(u)) return "date";
  if (/BOOL/.test(u)) return "T/F";
  return "abc";
}
