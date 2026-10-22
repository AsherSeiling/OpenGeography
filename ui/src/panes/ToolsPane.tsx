import { useShallow } from "zustand/react/shallow";
import { SearchBox } from "@fluentui/react-components";
import { DatabaseRegular, WrenchRegular } from "@fluentui/react-icons";
import { useMemo, useState } from "react";
import { justDragged, startDrag } from "../dnd";
import { useModel, useRun } from "../store/model";
import { useApp } from "../store/store";
import { addToolToModel } from "./ModelPane";
import { showPane } from "./panes";

/** Geoprocessing pane: find a tool or input layer, click or drag it into the Model. */
export function ToolsPane() {
  const specs = useRun((s) => s.specs);
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && !l.temporary)));
  const [q, setQ] = useState("");
  const query = q.trim().toLowerCase();
  const groups = useMemo(() => {
    const out: Record<string, typeof list> = {};
    // Every tree already has its own Layer Input and Output, so those (and
    // the older Add to Map) aren't offered here.
    const list = Object.values(specs).filter((s) => s.role !== "data" && s.id !== "layer_output" && s.id !== "add_to_map");
    for (const s of list) {
      if (query && !`${s.name} ${s.description} ${s.category}`.toLowerCase().includes(query)) continue;
      (out[s.category] ??= []).push(s);
    }
    // Outputs last, after the processing tools.
    return Object.entries(out).sort(([a], [b]) => (a === "Output" ? 1 : b === "Output" ? -1 : a.localeCompare(b)));
  }, [specs, query]);
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
        {groups.map(([cat, tools]) => (
          <div key={cat}>
            <div className="tools-cat">{cat === "Output" ? "Save to file" : cat}</div>
            {tools.map((t) => (
              <div
                key={t.id}
                className={`tool-item ${t.role === "output" ? "output" : ""}`}
                title={t.description}
                onPointerDown={(e) => startDrag(e, { kind: "tool", id: t.id, label: t.name })}
                onClick={() => add(t.id)}
              >
                <WrenchRegular />
                <div>
                  <div className="tool-name">{t.name}</div>
                  <div className="tool-desc">{t.description}</div>
                </div>
              </div>
            ))}
          </div>
        ))}
        {groups.length === 0 && inputs.length === 0 && <div className="muted pad">No tools match.</div>}
      </div>
    </div>
  );
}
