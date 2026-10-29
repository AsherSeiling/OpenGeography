// Blender-style "Add" menu for the Model (Shift+A, right-click, or the Add
// button): categories on the left, items on the right; typing searches
// everything. Enter adds the highlighted item, Esc closes.
import { DatabaseRegular, SearchRegular, WrenchRegular } from "@fluentui/react-icons";
import { compareCategories } from "./categories";
import { useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { useModel, useRun } from "../store/model";
import { useApp } from "../store/store";

export interface AddItem {
  key: string;
  label: string;
  category: string;
  description: string;
  tool: string;
  params?: Record<string, unknown>;
  kind: "input" | "tool" | "output";
}

/** Everything that can be added to the open tree. */
export function useAddItems(): AddItem[] {
  const specs = useRun((s) => s.specs);
  const owner = useModel((s) => s.layer);
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && !l.temporary)));
  return useMemo(() => {
    const items: AddItem[] = [
      { key: "layer:empty", label: "Layer", category: "Input", description: "A layer input. Choose a layer in it, or drop one from Contents onto it.", tool: "input_layer", kind: "input" },
      ...layers
        .filter((l) => l.id !== owner)
        .map((l): AddItem => ({ key: `layer:${l.id}`, label: l.name, category: "Input", description: `${l.geomType} layer · ${l.featureCount.toLocaleString()} features`, tool: "input_layer", params: { layer: l.id }, kind: "input" })),
    ];
    for (const s of Object.values(specs)) {
      if (s.role === "data" || s.id === "layer_output" || s.id === "add_to_map") continue;
      items.push({ key: `tool:${s.id}`, label: s.name, category: s.role === "output" ? "Save to File" : s.category, description: s.description, tool: s.id, kind: s.role === "output" ? "output" : "tool" });
    }
    return items;
  }, [specs, owner, layers]);
}


export function AddMenu({ x, y, onPick, onClose }: { x: number; y: number; onPick: (item: AddItem) => void; onClose: () => void }) {
  const items = useAddItems();
  const [q, setQ] = useState("");
  const [cat, setCat] = useState("Input");
  const [hi, setHi] = useState(0);
  const ref = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);

  const categories = useMemo(() => {
    const cats = Array.from(new Set(items.map((i) => i.category)));
    return cats.sort(compareCategories);
  }, [items]);

  const shown = useMemo(() => {
    const query = q.trim().toLowerCase();
    if (!query) return items.filter((i) => i.category === cat);
    const words = query.split(/\s+/);
    return items
      .filter((i) => words.every((w) => `${i.label} ${i.category} ${i.description}`.toLowerCase().includes(w)))
      .sort((a, b) => Number(!a.label.toLowerCase().startsWith(query)) - Number(!b.label.toLowerCase().startsWith(query)))
      .slice(0, 80);
  }, [items, q, cat]);

  useEffect(() => {
    input.current?.focus();
    const away = (e: PointerEvent) => !ref.current?.contains(e.target as Node) && onClose();
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, []);
  useEffect(() => setHi(0), [q, cat]);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setHi((h) => Math.min(shown.length - 1, h + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setHi((h) => Math.max(0, h - 1));
    } else if (e.key === "Enter" && shown[hi]) {
      e.preventDefault();
      onPick(shown[hi]);
    }
    e.stopPropagation();
  };

  // Keep the menu on screen.
  const W = 520;
  const H = 380;
  const left = Math.max(8, Math.min(x, window.innerWidth - W - 8));
  const top = Math.max(8, Math.min(y, window.innerHeight - H - 8));

  return (
    <div className="addmenu" ref={ref} style={{ left, top, width: W, height: H }} onKeyDown={onKey} onContextMenu={(e) => e.preventDefault()}>
      <div className="addmenu-search">
        <SearchRegular />
        <input ref={input} value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search tools and layers…" />
        <span className="muted small">Shift+A</span>
      </div>
      <div className="addmenu-body">
        {!q && (
          <div className="addmenu-cats">
            {categories.map((c) => (
              <button key={c} className={`addmenu-cat ${c === cat ? "on" : ""}`} onMouseEnter={() => setCat(c)} onClick={() => setCat(c)}>
                {c}
              </button>
            ))}
          </div>
        )}
        <div className="addmenu-items">
          {shown.length === 0 && <div className="muted pad">Nothing matches "{q}".</div>}
          {shown.map((i, k) => (
            <button
              key={i.key}
              className={`addmenu-item ${k === hi ? "on" : ""} ${i.kind}`}
              onMouseEnter={() => setHi(k)}
              onClick={() => onPick(i)}
              title={i.description}
            >
              {i.kind === "input" ? <DatabaseRegular /> : <WrenchRegular />}
              <span className="addmenu-label">{i.label}</span>
              {q && <span className="addmenu-catlabel">{i.category}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
