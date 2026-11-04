// Add-on 4: the Properties of a layout page or element as a few tabs, what it is first and Position last (docs/LAYOUT_UI_OPTIONS.md).
import { useRef, useState, type ReactNode } from "react";

export interface InspectorTab {
  id: string;
  label: string;
  body: ReactNode;
}

/** The tab last open for each kind of thing, so moving from one legend to another keeps the tab. */
const lastTab: Record<string, string> = {};

export function InspectorTabs({ kind, tabs }: { kind: string; tabs: InspectorTab[] }) {
  const [, bump] = useState(0);
  const list = useRef<HTMLDivElement>(null);
  const current = tabs.find((t) => t.id === lastTab[kind]) ?? tabs[0];
  const pick = (id: string) => {
    lastTab[kind] = id;
    bump((n) => n + 1);
  };
  const onKey = (e: React.KeyboardEvent) => {
    const i = tabs.indexOf(current);
    const next = e.key === "ArrowRight" ? (i + 1) % tabs.length : e.key === "ArrowLeft" ? (i - 1 + tabs.length) % tabs.length : e.key === "Home" ? 0 : e.key === "End" ? tabs.length - 1 : -1;
    if (next < 0) return;
    e.preventDefault();
    pick(tabs[next].id);
    requestAnimationFrame(() => list.current?.querySelectorAll<HTMLElement>('[role="tab"]')[next]?.focus());
  };
  return (
    <>
      <div className="itabs" role="tablist" aria-label="Properties" ref={list} onKeyDown={onKey}>
        {tabs.map((t) => (
          <button key={t.id} role="tab" id={`itab-${kind}-${t.id}`} aria-selected={t === current} aria-controls={`ipanel-${kind}`} tabIndex={t === current ? 0 : -1} className={t === current ? "on" : ""} onClick={() => pick(t.id)}>
            {t.label}
          </button>
        ))}
      </div>
      <div className="itab-body stack" role="tabpanel" id={`ipanel-${kind}`} aria-labelledby={`itab-${kind}-${current.id}`} data-tab={current.id}>
        {current.body}
      </div>
    </>
  );
}
