import { SearchBox } from "@fluentui/react-components";
import { WrenchRegular } from "@fluentui/react-icons";
import { useMemo, useState } from "react";
import { useRun } from "../store/model";
import { addToolToModel } from "./ModelPane";
import { showPane } from "./panes";

/** Geoprocessing pane: find a tool, click or drag it into the Model. */
export function ToolsPane() {
  const specs = useRun((s) => s.specs);
  const [q, setQ] = useState("");
  const groups = useMemo(() => {
    const out: Record<string, typeof list> = {};
    const list = Object.values(specs).filter((s) => s.role !== "data");
    const query = q.trim().toLowerCase();
    for (const s of list) {
      if (query && !`${s.name} ${s.description} ${s.category}`.toLowerCase().includes(query)) continue;
      (out[s.category] ??= []).push(s);
    }
    return Object.entries(out).sort(([a], [b]) => a.localeCompare(b));
  }, [specs, q]);

  return (
    <div className="tools">
      <SearchBox placeholder="Find Tools" value={q} onChange={(_, d) => setQ(d.value)} size="small" />
      <div className="tools-list">
        {groups.map(([cat, tools]) => (
          <div key={cat}>
            <div className="tools-cat">{cat}</div>
            {tools.map((t) => (
              <div
                key={t.id}
                className="tool-item"
                draggable
                title={t.description}
                onDragStart={(e) => e.dataTransfer.setData("application/gis-tool", t.id)}
                onClick={() => {
                  showPane("model");
                  addToolToModel(t.id);
                }}
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
        {groups.length === 0 && <div className="muted pad">No tools match.</div>}
      </div>
    </div>
  );
}
