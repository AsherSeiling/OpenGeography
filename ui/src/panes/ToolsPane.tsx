import { useShallow } from "zustand/react/shallow";
import { SearchBox } from "@fluentui/react-components";
import { DatabaseRegular } from "@fluentui/react-icons";
import { ToolIcon } from "../nodes/toolIcon";
import { useMemo, useState } from "react";
import { justDragged, startDrag } from "../dnd";
import { useModel, useRun } from "../store/model";
import { useApp } from "../store/store";
import { addToolToModel } from "./ModelPane";
import { showPane } from "./panes";
import { buildTree } from "./../nodes/categories";
import { ChevronDownRegular, ChevronRightRegular } from "@fluentui/react-icons";

/** Geoprocessing pane: find a tool or input layer, click or drag it into the Model. */
export function ToolsPane() {
  const specs = useRun((s) => s.specs);
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && !l.temporary)));
  const [q, setQ] = useState("");
  const query = q.trim().toLowerCase();
  const tree = useMemo(() => {
    // Every tree already has its own Layer Input and Output, so those (and the older Add to Map) aren't offered here.
    const list = Object.values(specs).filter((s) => s.role !== "data" && s.id !== "layer_output" && s.id !== "add_to_map");
    return buildTree(list.filter((s) => !query || `${s.name} ${s.description} ${s.category}`.toLowerCase().includes(query)));
  }, [specs, query]);
  const [closed, setClosed] = useState<Record<string, boolean>>(loadClosed);
  // A search opens everything that matches; otherwise a dropdown is open unless the user closed it.
  const isOpen = (key: string) => !!query || !closed[key];
  const toggle = (key: string) => setClosed((c) => { const n = { ...c, [key]: !c[key] }; try { localStorage.setItem(CLOSED_KEY, JSON.stringify(n)); } catch { /* private window */ } return n; });
  const treeLayer = useModel((s) => s.layer);
  const inputs = layers.filter((l) => l.id !== treeLayer && (!query || l.name.toLowerCase().includes(query) || "input layer data".includes(query)));

  const add = (tool: string, params?: Record<string, unknown>) => {
    if (justDragged()) return;
    showPane("model");
    addToolToModel(tool, params);
  };

  return (
    <div className="tools">
      <SearchBox placeholder="Find Tools" value={q} onChange={(_, d) => setQ(d.value)} size="small" />
      <div className="tools-hint">Click to add to the Model, or drag onto it.</div>
      <div className="tools-list">
        <div className="tools-cat">Other layers (extra inputs)</div>
        {inputs.length === 0 && <div className="muted tool-empty">Other layers appear here, e.g. to clip by.</div>}
        {inputs.map((l) => (
          <div
            key={l.id}
            className="tool-item input"
            title={`Use ${l.name} as an input`}
            onPointerDown={(e) => startDrag(e, { kind: "layer", id: l.id, label: l.name })}
            onClick={() => add("input_layer", { layer: l.id })}
          >
            <DatabaseRegular />
            <div>
              <div className="tool-name">{l.name}</div>
              <div className="tool-desc">{l.geomType === "Table" ? "Table" : `${l.geomType} layer`} · {l.featureCount.toLocaleString()}</div>
            </div>
          </div>
        ))}
        {tree.map((g) => (
          <div key={g.group} data-testid={`tool-group-${g.group}`}>
            <button className="tools-cat tools-fold" aria-expanded={isOpen(g.group)} onClick={() => toggle(g.group)}>
              {isOpen(g.group) ? <ChevronDownRegular /> : <ChevronRightRegular />} {g.group === "Output" ? "Save to file" : g.group} <span className="tools-count">{g.count}</span>
            </button>
            {isOpen(g.group) && (
              <>
                {g.tools.map((t) => <ToolItem key={t.id} t={t} add={add} />)}
                {g.subs.map((sub) => (
                  <div key={sub.name} className="tools-sub">
                    <button className="tools-subcat tools-fold" aria-expanded={isOpen(`${g.group}/${sub.name}`)} onClick={() => toggle(`${g.group}/${sub.name}`)}>
                      {isOpen(`${g.group}/${sub.name}`) ? <ChevronDownRegular /> : <ChevronRightRegular />} {sub.name} <span className="tools-count">{sub.tools.length}</span>
                    </button>
                    {isOpen(`${g.group}/${sub.name}`) && sub.tools.map((t) => <ToolItem key={t.id} t={t} add={add} />)}
                  </div>
                ))}
              </>
            )}
          </div>
        ))}
        {tree.length === 0 && inputs.length === 0 && <div className="muted pad">No tools match.</div>}
      </div>
    </div>
  );
}

const CLOSED_KEY = "gis.toolsClosed";
function loadClosed(): Record<string, boolean> {
  try {
    return JSON.parse(localStorage.getItem(CLOSED_KEY) ?? "{}") ?? {};
  } catch {
    return {};
  }
}

function ToolItem({ t, add }: { t: { id: string; name: string; description: string; role: string }; add: (tool: string) => void }) {
  return (
    <div className={`tool-item ${t.role === "output" ? "output" : ""}`} title={t.description} onPointerDown={(e) => startDrag(e, { kind: "tool", id: t.id, label: t.name })} onClick={() => add(t.id)}>
      <ToolIcon tool={t.id} />
      <div>
        <div className="tool-name">{t.name}</div>
        <div className="tool-desc">{t.description}</div>
      </div>
    </div>
  );
}
