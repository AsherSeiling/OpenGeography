// The Items catalog (LW2): everything on the open layout in stacking order, top first — page, elements, groups. Click to select
// (Cmd/Ctrl toggles, Shift selects a range), eye and lock per row, double-click to rename, drag to reorder, search, group/ungroup.
import { Input } from "@fluentui/react-components";
import { useMemo, useState } from "react";
import { clickSelection, displayOrder, itemName, KIND_NAME, matchItems } from "../layout/items";
import { useLayouts } from "../store/layout";

export function ItemsPane() {
  const doc = useLayouts((s) => s.doc);
  const selected = useLayouts((s) => s.selected);
  const [q, setQ] = useState("");
  const [anchor, setAnchor] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [drop, setDrop] = useState<{ id: string; below: boolean } | null>(null);
  const rows = useMemo(() => (doc ? matchItems(displayOrder(doc.elements), q) : []), [doc, q]);
  if (!doc) return <div className="pane-empty muted">Open a layout to see its items.</div>;
  const s = useLayouts.getState();
  const grouped = selected.some((id) => doc.elements.find((e) => e.id === id)?.group);
  return (
    <div className="layout-outline items-pane" aria-label="Layout items">
      <div className="layouts-head"><span>{doc.name}</span></div>
      <Input size="small" placeholder="Search items" aria-label="Search items" value={q} onChange={(_, d) => setQ(d.value)} />
      <div className="row page-row" data-testid="items-page" onClick={() => s.select([])}>
        <span className="grow">Page · {doc.page.width} × {doc.page.height} mm</span>
      </div>
      <div className="row items-actions">
        <button className="linkbtn" disabled={selected.length < 2} onClick={() => s.group()}>Group</button>
        <button className="linkbtn" disabled={!grouped} onClick={() => s.ungroup()}>Ungroup</button>
        <span className="grow" />
        <span className="muted">{doc.elements.length} item{doc.elements.length === 1 ? "" : "s"}</span>
      </div>
      {rows.map((el) => {
        const on = selected.includes(el.id);
        const label = itemName(el);
        const kind = KIND_NAME[el.kind] ?? el.kind;
        const mark = drop?.id === el.id ? (drop.below ? "drop-below" : "drop-above") : "";
        return (
          <div
            key={el.id}
            className={`row ${on ? "on" : ""} ${mark}`}
            data-element={el.id}
            draggable={!q}
            onDragStart={(e) => e.dataTransfer.setData("text/plain", el.id)}
            onDragOver={(e) => {
              e.preventDefault();
              const r = e.currentTarget.getBoundingClientRect();
              setDrop({ id: el.id, below: e.clientY > r.top + r.height / 2 });
            }}
            onDragLeave={() => setDrop(null)}
            onDrop={(e) => {
              e.preventDefault();
              const id = e.dataTransfer.getData("text/plain");
              // The list shows the top first, so "below" in the list is lower in the stacking order.
              if (id) s.moveElement(id, el.id, !!drop?.below);
              setDrop(null);
            }}
            onClick={(e) => {
              s.select(clickSelection(doc.elements, selected, anchor, el.id, { toggle: e.metaKey || e.ctrlKey, shift: e.shiftKey }));
              if (!e.shiftKey) setAnchor(el.id);
            }}
            onDoubleClick={() => setRenaming(el.id)}
          >
            <input type="checkbox" checked={el.visible} title="Show or hide" aria-label={`Show ${kind} ${el.id}`} onClick={(e) => e.stopPropagation()} onChange={(e) => s.update(el.id, { visible: e.target.checked })} />
            {renaming === el.id ? (
              <input
                className="grow items-rename"
                autoFocus
                aria-label={`Name of ${kind} ${el.id}`}
                defaultValue={el.name ?? ""}
                placeholder={label}
                onClick={(e) => e.stopPropagation()}
                onBlur={(e) => { s.update(el.id, { name: e.target.value.trim() }); setRenaming(null); }}
                onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); else if (e.key === "Escape") setRenaming(null); e.stopPropagation(); }}
              />
            ) : (
              <span className="grow" title={`${label} · ${kind}${el.group ? " (grouped)" : ""}`}>{label}{el.group ? " ⛓" : ""}{el.locked ? " 🔒" : ""}</span>
            )}
            <button className="linkbtn" title={el.locked ? "Unlock" : "Lock"} aria-label={`${el.locked ? "Unlock" : "Lock"} ${el.id}`} onClick={(e) => { e.stopPropagation(); s.update(el.id, { locked: !el.locked }); }}>{el.locked ? "Unlock" : "Lock"}</button>
            <button className="linkbtn" title="Bring forward" aria-label={`Bring ${el.id} forward`} onClick={(e) => { e.stopPropagation(); s.select([el.id]); s.arrange("forward"); }}>▲</button>
            <button className="linkbtn" title="Send backward" aria-label={`Send ${el.id} backward`} onClick={(e) => { e.stopPropagation(); s.select([el.id]); s.arrange("backward"); }}>▼</button>
          </div>
        );
      })}
      {!rows.length && <div className="muted">{q ? "No item matches." : "This layout has no items yet."}</div>}
    </div>
  );
}
