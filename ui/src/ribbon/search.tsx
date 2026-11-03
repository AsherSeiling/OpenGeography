// Tab-row search: commands, tools and layers in one box (Alt+Q). Tools and commands use the Shift+A menu's matcher (nodes/toolSearch.ts).
// Picking a tool adds it to the active layer's node tree exactly as the menu does; picking a layer makes it the active one.
import { SearchRegular } from "@fluentui/react-icons";
import { useId, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { searchTools, type Searchable } from "../nodes/toolSearch";
import { useToolMenu } from "../nodes/toolMenuStore";
import { useRun } from "../store/model";
import { useApp } from "../store/store";
import { addToolFromUi } from "./addTool";
import { commands, CATALOG_COMMANDS, type Command } from "./commands";
import { TABS } from "./tabsDef";

interface Hit {
  key: string;
  label: string;
  hint: "Command" | "Tool" | "Layer";
  disabled?: boolean;
  run: () => void;
}

/** Group name of each command, so "Coordinate System" finds the commands in that group. */
function groupOf(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const groups of Object.values(TABS)) for (const g of groups) for (const it of g.items) if ("cmd" in it) out[it.cmd] ??= g.label;
  for (const id of CATALOG_COMMANDS) out[id] = "Public Data";
  return out;
}

export function TopSearch() {
  const [value, setValue] = useState("");
  const [open, setOpen] = useState(false);
  const [hi, setHi] = useState(0);
  const box = useRef<HTMLInputElement>(null);
  const uid = useId();
  const specs = useRun((s) => s.specs);
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && !l.temporary)));
  const fav = useToolMenu((s) => s.fav);

  const cmdItems = useMemo(() => {
    const g = groupOf();
    return Object.values(commands).map((c): Searchable & { cmd: Command } => ({ id: c.id, name: c.label, category: g[c.id] ?? "", description: c.tip ?? "", cmd: c }));
  }, []);
  const toolItems = useMemo(
    () => Object.values(specs).filter((s) => s.role !== "data" && s.id !== "layer_output" && s.id !== "add_to_map").map((s): Searchable => ({ id: s.id, name: s.name, category: s.category, description: s.description, keywords: s.keywords })),
    [specs],
  );
  const layerItems = useMemo(() => layers.map((l): Searchable => ({ id: l.id, name: l.name, category: "Layer", description: l.geomType })), [layers]);

  const hits: Hit[] = useMemo(() => {
    if (!value.trim()) return [];
    const c = searchTools(cmdItems, value, fav).slice(0, 5).map((h): Hit => ({ key: `c:${h.item.id}`, label: h.item.name, hint: "Command", disabled: h.item.cmd.enabled ? !h.item.cmd.enabled() : false, run: () => h.item.cmd.run() }));
    const t = searchTools(toolItems, value, fav).slice(0, 7).map((h): Hit => ({ key: `t:${h.item.id}`, label: h.item.name, hint: "Tool", run: () => void addToolFromUi(h.item.id) }));
    const l = searchTools(layerItems, value, fav).slice(0, 3).map((h): Hit => ({ key: `l:${h.item.id}`, label: h.item.name, hint: "Layer", run: () => useApp.getState().setActiveLayer(h.item.id) }));
    return [...c, ...t, ...l];
  }, [value, cmdItems, toolItems, layerItems, fav]);

  const close = () => {
    setValue("");
    setOpen(false);
    setHi(0);
  };
  const pick = (h: Hit | undefined) => {
    if (!h || h.disabled) return;
    close();
    box.current?.blur();
    h.run();
  };
  const cur = Math.min(hi, hits.length - 1);
  const listId = `${uid}-list`;

  return (
    <div className="tellme">
      <label className="search">
        <SearchRegular />
        <input
          ref={box}
          role="combobox"
          aria-expanded={open && !!value.trim()}
          aria-controls={listId}
          aria-activedescendant={cur >= 0 && open ? `${uid}-o${cur}` : undefined}
          aria-label="Search commands and tools"
          placeholder="Search commands and tools"
          autoComplete="off"
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            setOpen(true);
            setHi(0);
          }}
          onFocus={() => setOpen(true)}
          onBlur={() => window.setTimeout(() => setOpen(false), 120)}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") setHi(Math.min(hits.length - 1, cur + 1));
            else if (e.key === "ArrowUp") setHi(Math.max(0, cur - 1));
            else if (e.key === "Enter") pick(hits[cur]);
            else if (e.key === "Escape") {
              close();
              box.current?.blur();
            } else return;
            e.preventDefault();
            e.stopPropagation();
          }}
        />
        <kbd>Alt+Q</kbd>
      </label>
      {open && value.trim() && (
        <div className="search-pop" role="listbox" id={listId} aria-label="Search results">
          {hits.length === 0 && <div className="search-none">Nothing matches “{value.trim()}”. Try a plain word, like “cut” or “join”.</div>}
          {hits.map((h, i) => (
            <div
              key={h.key}
              id={`${uid}-o${i}`}
              role="option"
              aria-selected={i === cur}
              aria-disabled={h.disabled || undefined}
              className={`search-row${i === cur ? " on" : ""}`}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => pick(h)}
            >
              <span className="search-name">{h.label}</span>
              <span className="search-hint">{h.hint}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
