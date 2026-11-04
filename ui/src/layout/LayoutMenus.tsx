// Right-click menus and the Shift+A "add an element" menu of the layout page (add-on 2), and the context bar over a selected element.
// One popup component serves both menus; the entries call the same store actions and ribbon commands as the ribbon does.
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { create } from "zustand";
import { useDialog } from "../actions";
import { commands } from "../ribbon/commands";
import { beginStep, useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { LockClosedRegular, LockOpenRegular } from "@fluentui/react-icons";
import { copySelected, clipboardCount, cutSelected, duplicateSelected, pasteHeld, toggleLockSelected } from "./clipboard";
import { setFrameFromMapView } from "./actions";
import { DEFAULT_STYLES, textMatches, textPatch, useNamedStyles } from "./namedStyles";
import type { AlignOp, OrderOp } from "./arrange";
import { pointerMm } from "./items";
import { useLayoutView } from "./view";
import type { LayoutElement, TextElement } from "./types";

export interface Item {
  label: string;
  run?: () => void;
  disabled?: boolean;
  sub?: Item[];
  sep?: boolean;
  hint?: string;
  checked?: boolean;
}

interface MenuState {
  menu: null | { kind: "context" | "add"; x: number; y: number; target: "page" | "element" | "frame"; at: { x: number; y: number } | null };
  open: (m: NonNullable<MenuState["menu"]>) => void;
  close: () => void;
}
export const useLayoutMenus = create<MenuState>((set) => ({ menu: null, open: (menu) => set({ menu }), close: () => set({ menu: null }) }));

const ORDER: [OrderOp, string][] = [["front", "Bring to Front"], ["forward", "Bring Forward"], ["backward", "Send Backward"], ["back", "Send to Back"]];
const ALIGN: [AlignOp, string][] = [["left", "Align Left"], ["center", "Align Centre"], ["right", "Align Right"], ["top", "Align Top"], ["middle", "Align Middle"], ["bottom", "Align Bottom"]];

/** The elements the Insert menu offers, in the order of the rail. */
export const INSERTABLE = ["layoutMapFrame", "layoutOverview", "layoutLegend", "layoutScaleBar", "layoutNorthArrow", "layoutDeclination", "layoutText", "layoutSources", "layoutTable", "layoutChart", "layoutRect", "layoutEllipse", "layoutTriangle", "layoutLine", "layoutArrow", "layoutPicture"];

/** Run an insert command; when `at` (mm) is given, the new elements are moved so their top-left corner is there. */
export function insertAt(id: string, at: { x: number; y: number } | null) {
  const before = new Set((useLayouts.getState().doc?.elements ?? []).map((e) => e.id));
  commands[id].run();
  if (!at) return;
  const s = useLayouts.getState();
  const fresh = (s.doc?.elements ?? []).filter((e) => !before.has(e.id));
  if (!fresh.length) return;
  const p = s.doc!.page;
  const minX = Math.min(...fresh.map((e) => e.x));
  const minY = Math.min(...fresh.map((e) => e.y));
  const w = Math.max(...fresh.map((e) => e.x + e.w)) - minX;
  const h = Math.max(...fresh.map((e) => e.y + e.h)) - minY;
  const dx = Math.min(Math.max(0, at.x), Math.max(0, p.width - w)) - minX;
  const dy = Math.min(Math.max(0, at.y), Math.max(0, p.height - h)) - minY;
  const end = beginStep();
  for (const e of fresh) s.update(e.id, { x: Math.round((e.x + dx) * 10) / 10, y: Math.round((e.y + dy) * 10) / 10 });
  end();
}

export function contextItems(target: "page" | "element" | "frame", at: { x: number; y: number } | null, openAdd: () => void): Item[] {
  const s = useLayouts.getState;
  if (target === "page") {
    return [
      { label: "Paste", run: () => void pasteHeld(at ?? undefined), disabled: clipboardCount() === 0, hint: "Ctrl+V" },
      { label: "", sep: true },
      { label: "Insert…", run: openAdd, hint: "Shift+A" },
      { label: "", sep: true },
      { label: "Page Setup…", run: () => useDialog.getState().show({ kind: "layoutPage" }) },
    ];
  }
  const frame: Item[] = target === "frame"
    ? [
        { label: "Set from Map View", run: () => void setFrameFromMapView() },
        { label: "Which Map", sub: [{ label: "The active map", run: () => { const id = s().selected[0]; if (id) s().update(id, { layers: "map" }); } }, ...useMapsItems()] },
        { label: "", sep: true },
      ]
    : [];
  return [
    ...frame,
    { label: "Cut", run: () => void cutSelected(), hint: "Ctrl+X" },
    { label: "Copy", run: () => void copySelected(), hint: "Ctrl+C" },
    { label: "Duplicate", run: () => void duplicateSelected(), hint: "Ctrl+D" },
    { label: "Delete", run: () => s().removeElements(s().selected.filter((id) => !s().doc?.elements.find((x) => x.id === id)?.locked)), hint: "Del" },
    { label: "", sep: true },
    { label: "Order", sub: ORDER.map(([op, label]) => ({ label, run: () => s().arrange(op) })) },
    { label: "Align", sub: ALIGN.map(([op, label]) => ({ label, run: () => s().align(op) })) },
    { label: "Group", run: () => s().group(), disabled: s().selected.length < 2 },
    { label: "Lock", run: () => void toggleLockSelected(), checked: (s().doc?.elements ?? []).filter((e) => s().selected.includes(e.id)).every((e) => e.locked) },
  ];
}

/** "Which Map" entries: the project's maps (read when the menu is built). */
function useMapsItems(): Item[] {
  const sel = useLayouts.getState().selected[0];
  return useApp.getState().maps.map((m) => ({ label: m.name, run: () => sel && useLayouts.getState().update(sel, { layers: { map: m.id } }) }));
}

/** A popup list with arrow-key movement; Right opens a submenu, Left or Escape closes it. */
function Popup({ items, x, y, onDone, title, depth = 0 }: { items: Item[]; x: number; y: number; onDone: () => void; title?: string; depth?: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  const [open, setOpen] = useState<number | null>(null);
  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (r) setPos({ x: Math.max(4, Math.min(x, window.innerWidth - r.width - 4)), y: Math.max(4, Math.min(y, window.innerHeight - r.height - 4)) });
    if (depth === 0) ref.current?.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
  }, [x, y, depth]);
  const key = (e: React.KeyboardEvent) => {
    const btns = [...(ref.current?.querySelectorAll<HTMLElement>(":scope > button:not(:disabled)") ?? [])];
    const at = btns.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") btns[(at + 1) % btns.length]?.focus();
    else if (e.key === "ArrowUp") btns[(at - 1 + btns.length) % btns.length]?.focus();
    else if (e.key === "ArrowRight" && (document.activeElement as HTMLElement)?.dataset.sub) setOpen(Number((document.activeElement as HTMLElement).dataset.i));
    else if ((e.key === "ArrowLeft" && depth > 0) || e.key === "Escape") {
      if (depth > 0) setOpen(null);
      else onDone();
    } else return;
    e.preventDefault();
    e.stopPropagation();
  };
  return (
    <>
      <div ref={ref} className="lmenu" role="menu" aria-label={title ?? "Layout"} style={{ left: pos.x, top: pos.y }} onKeyDown={key} onPointerDown={(e) => e.stopPropagation()} onContextMenu={(e) => e.preventDefault()}>
        {items.map((it, i) =>
          it.sep ? (
            <hr key={i} />
          ) : (
            <button
              key={i}
              role="menuitem"
              data-i={i}
              data-sub={it.sub ? "1" : undefined}
              disabled={it.disabled}
              aria-haspopup={it.sub ? "menu" : undefined}
              aria-checked={it.checked === undefined ? undefined : it.checked}
              onMouseEnter={() => setOpen(it.sub ? i : null)}
              onClick={() => {
                if (it.sub) return setOpen(i);
                onDone();
                it.run?.();
              }}
            >
              <span className="lm-check">{it.checked ? "✓" : ""}</span>
              <span className="lm-label">{it.label}</span>
              {it.sub ? <span className="lm-hint">▸</span> : it.hint ? <span className="lm-hint">{it.hint}</span> : null}
            </button>
          ),
        )}
      </div>
      {open !== null && items[open]?.sub && ref.current && (
        <Popup items={items[open].sub!} x={ref.current.getBoundingClientRect().right - 2} y={ref.current.querySelector<HTMLElement>(`[data-i="${open}"]`)?.getBoundingClientRect().top ?? y} onDone={onDone} depth={depth + 1} title={items[open].label} />
      )}
    </>
  );
}

/** Shift+A: type to find an element to insert, Enter inserts it where the pointer was. */
function AddMenu({ x, y, at, onDone }: { x: number; y: number; at: { x: number; y: number } | null; onDone: () => void }) {
  const [q, setQ] = useState("");
  const [on, setOn] = useState(0);
  const list = useMemo(() => {
    const t = q.trim().toLowerCase();
    return INSERTABLE.map((id) => commands[id]).filter((c) => c && (!t || `${c.label} ${c.tip ?? ""}`.toLowerCase().includes(t)));
  }, [q]);
  useEffect(() => setOn(0), [q]);
  const pick = (id: string) => {
    onDone();
    insertAt(id, at);
  };
  return (
    <div className="lmenu add-menu" role="dialog" aria-label="Insert an element" style={{ left: Math.max(4, Math.min(x, window.innerWidth - 300)), top: Math.max(4, Math.min(y, window.innerHeight - 380)) }} onPointerDown={(e) => e.stopPropagation()}>
      <input
        className="add-search"
        autoFocus
        placeholder="Insert an element…"
        aria-label="Search elements to insert"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") { e.stopPropagation(); onDone(); }
          else if (e.key === "ArrowDown") { e.preventDefault(); setOn((v) => Math.min(list.length - 1, v + 1)); }
          else if (e.key === "ArrowUp") { e.preventDefault(); setOn((v) => Math.max(0, v - 1)); }
          else if (e.key === "Enter" && list[on]) { e.preventDefault(); const c = list[on]; if (!c.enabled || c.enabled()) pick(c.id); }
        }}
      />
      <div className="add-list" role="listbox">
        {list.map((c, i) => {
          const off = c.enabled ? !c.enabled() : false;
          return (
            <div key={c.id} role="option" aria-selected={i === on} aria-disabled={off} className={`add-row${i === on ? " on" : ""}`} onMouseEnter={() => setOn(i)} onClick={() => !off && pick(c.id)}>
              <span className="rbtn-icon">{c.icon}</span>
              <span className="lm-label">{c.label}</span>
              {off && <span className="lm-hint">needs a map frame</span>}
            </div>
          );
        })}
        {!list.length && <div className="lm-none">No element matches.</div>}
      </div>
    </div>
  );
}

/** Mounted once in the layout view; shows whichever menu was opened. */
export function LayoutPopups() {
  const m = useLayoutMenus((s) => s.menu);
  const close = useLayoutMenus((s) => s.close);
  useEffect(() => {
    if (!m) return;
    const off = () => close();
    window.addEventListener("pointerdown", off);
    window.addEventListener("blur", off);
    return () => {
      window.removeEventListener("pointerdown", off);
      window.removeEventListener("blur", off);
    };
  }, [m, close]);
  if (!m) return null;
  // On the body, not in the page: the dock draws other panes over a pane's own overflow.
  const menu = m.kind === "add" ? <AddMenu x={m.x} y={m.y} at={m.at} onDone={close} /> : <Popup x={m.x} y={m.y} title="Page" items={contextItems(m.target, m.at, () => useLayoutMenus.getState().open({ ...m, kind: "add" }))} onDone={close} />;
  return createPortal(menu, document.body);
}

// ---- the context bar

/** Over the selected element: text style, size, colour, alignment and lock. Only for a Text element (the others have their lists in Properties); lock is always there. */
export function ContextBar({ el, left, top }: { el: LayoutElement; left: number; top: number }) {
  const set = (patch: Record<string, unknown>, burst?: string) => useLayouts.getState().update(el.id, patch, burst);
  const layoutId = useLayouts((s) => s.id) ?? "";
  const styles = useNamedStyles((s) => s.byLayout[layoutId]) ?? DEFAULT_STYLES;
  const t = el.kind === "text" ? (el as TextElement) : null;
  const match = t ? styles.find((s) => textMatches(t, s)) : undefined;
  return (
    <div className="ctxbar" role="toolbar" aria-label="Selected element" style={{ left, top }} onPointerDown={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
      {t && (
        <>
          <select aria-label="Text style" value={match?.name ?? ""} onChange={(e) => { const s = styles.find((x) => x.name === e.target.value); if (s) set(textPatch(s)); }}>
            <option value="" disabled>{match ? "" : "Custom"}</option>
            {styles.map((s) => <option key={s.name} value={s.name}>{s.name}</option>)}
          </select>
          <input className="ctx-size" type="number" aria-label="Text size in points" min={1} max={400} step={1} value={t.size} onChange={(e) => { const v = Number(e.target.value); if (Number.isFinite(v) && v >= 1 && v <= 400) set({ size: v }, "size"); }} />
          <button aria-label="Bold" aria-pressed={t.font === "sans-bold"} className={t.font === "sans-bold" ? "on" : ""} onClick={() => set({ font: t.font === "sans-bold" ? "sans" : "sans-bold" })}><b>B</b></button>
          <input type="color" aria-label="Text colour" value={t.color} onChange={(e) => set({ color: e.target.value }, "color")} />
          {(["left", "center", "right"] as const).map((a) => (
            <button key={a} aria-label={`Text ${a}`} aria-pressed={t.align === a} className={t.align === a ? "on" : ""} onClick={() => set({ align: a })}>{a === "left" ? "⇤" : a === "center" ? "↔" : "⇥"}</button>
          ))}
          <span className="ctx-sep" />
        </>
      )}
      <button aria-label={el.locked ? "Unlock" : "Lock"} aria-pressed={el.locked} className={el.locked ? "on" : ""} onClick={() => void toggleLockSelected()}>{el.locked ? <LockOpenRegular /> : <LockClosedRegular />}{el.locked ? "Unlock" : "Lock"}</button>
    </div>
  );
}

// ---- Shift+A

/**
 * Shift+A with the keys in the layout page opens the Insert menu. The Model's Add Tool shortcut listens on the window while the event is
 * still on its way down and swallows the key, so this listener is added first (when this module loads, before any pane mounts) and keeps
 * the key for itself only while the focus is inside the layout view and not in a text box.
 */
let pointer: { x: number; y: number } | null = null;
if (typeof window !== "undefined") {
  window.addEventListener("pointermove", (e) => { pointer = { x: e.clientX, y: e.clientY }; }, true);
  window.addEventListener(
    "keydown",
    (e) => {
      if (e.key !== "A" || !e.shiftKey || e.ctrlKey || e.metaKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      if (!target?.closest?.(".layout-view") || target.closest("input, textarea, select, [contenteditable]")) return;
      const s = useLayouts.getState();
      if (!s.doc || !useLayoutView.getState().workspace) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      const page = document.querySelector(".layout-page")?.getBoundingClientRect();
      const wrap = document.querySelector(".layout-canvas")?.getBoundingClientRect();
      const over = pointer && page && pointer.x >= page.left && pointer.x <= page.right && pointer.y >= page.top && pointer.y <= page.bottom;
      useLayoutMenus.getState().open({
        kind: "add",
        x: pointer?.x ?? (wrap?.left ?? 0) + 80,
        y: pointer?.y ?? (wrap?.top ?? 0) + 60,
        target: "page",
        at: over && pointer && page ? pointerMm(pointer.x, pointer.y, page, useLayoutView.getState().shown, s.doc.page) : null,
      });
    },
    true,
  );
}
