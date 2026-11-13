// The Data pane of the statistics workspace (left): the project's layers and tables, each with its fields, their types and how many values
// are missing. A filter narrows layers and fields by name; "Selected features only" counts just the features selected on the map (and is what
// new cards start with).
import { Checkbox, Input } from "@fluentui/react-components";
import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, type LayerInfo } from "../ipc/api";
import { useApp } from "../store/store";
import { projectLayerInfos } from "../store/maps";
import { useStats } from "./store";
import type { DataProfile } from "./types";

const eligible = (l: LayerInfo) => !l.temporary && l.geomType !== "Raster";

export function DataPane() {
  const layers = useApp(useShallow((s) => projectLayerInfos(s).filter((l) => l && eligible(l))));
  const rasters = useApp(useShallow((s) => projectLayerInfos(s).filter((l) => l && !l.temporary && l.geomType === "Raster").map((l) => l.id)));
  const selection = useApp((s) => s.selection);
  const selectedOnly = useStats((s) => s.selectedOnly);
  const [q, setQ] = useState("");
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const needle = q.trim().toLowerCase();
  const shown = layers.filter((l) => !needle || l.name.toLowerCase().includes(needle) || l.fields.some((f) => f.name.toLowerCase().includes(needle)));
  return (
    <div className="stat-data" aria-label="Statistics data">
      <Input size="small" placeholder="Filter layers and fields" aria-label="Filter layers and fields" value={q} onChange={(_, d) => setQ(d.value)} />
      <Checkbox
        label="Selected features only"
        checked={selectedOnly}
        onChange={(_, d) => useStats.getState().setSelectedOnly(!!d.checked)}
      />
      {selectedOnly && <div className="stat-meta">Counts and new cards use the features selected on the map. A layer with nothing selected shows 0.</div>}
      {layers.length === 0 && <div className="empty-hint">No layers or tables yet. Add Data on the Map tab, then come back.</div>}
      {shown.map((l) => (
        <LayerBlock
          key={l.id}
          layer={l}
          open={!!open[l.id] || (!!needle && l.fields.some((f) => f.name.toLowerCase().includes(needle)))}
          onToggle={() => setOpen((o) => ({ ...o, [l.id]: !o[l.id] }))}
          fids={selectedOnly ? (selection[l.id] ?? []) : null}
          needle={needle}
        />
      ))}
      {layers.length > 0 && shown.length === 0 && <div className="empty-hint">Nothing matches "{q}".</div>}
      {rasters.length > 0 && <div className="stat-meta">{rasters.length} raster{rasters.length === 1 ? " is" : "s are"} not listed: statistics sheets work on layers and tables.</div>}
    </div>
  );
}

function LayerBlock({ layer, open, onToggle, fids, needle }: { layer: LayerInfo; open: boolean; onToggle: () => void; fids: number[] | null; needle: string }) {
  const [profile, setProfile] = useState<DataProfile | null>(null);
  const [error, setError] = useState("");
  // Counts again when the layer's data changes (its version) or the selection it is counted on does.
  const fidKey = fids ? fids.join(",") : "";
  useEffect(() => {
    if (!open) return;
    let live = true;
    api.statDataProfile(layer.id, fids)
      .then((p) => live && (setProfile(p), setError("")))
      .catch((e) => live && setError(errorText(e)));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, layer.id, layer.version, fidKey, fids === null]);
  const missing = useMemo(() => new Map((profile?.fields ?? []).map((f) => [f.name, f.missing])), [profile]);
  const fields = layer.fields.filter((f) => !needle || layer.name.toLowerCase().includes(needle) || f.name.toLowerCase().includes(needle));
  return (
    <div className="stat-data-layer">
      <button className="stat-data-name" aria-expanded={open} onClick={onToggle}>
        <span aria-hidden>{open ? "▾" : "▸"}</span>
        <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis" }}>{layer.name}</span>
        <span className="muted">{layer.geomType === "Table" ? "table" : layer.geomType.toLowerCase()}</span>
      </button>
      {open && (
        <div style={{ padding: "0 4px 4px" }}>
          {error && <div className="stat-banner error">{error}</div>}
          <div className="stat-meta" style={{ padding: "0 8px" }}>
            {profile ? `${profile.rows.toLocaleString()} ${fids ? "selected " : ""}row${profile.rows === 1 ? "" : "s"}` : "Counting…"}
          </div>
          <table className="stat-fields">
            <thead>
              <tr>
                <th>Field</th>
                <th>Type</th>
                <th className="num">Missing</th>
              </tr>
            </thead>
            <tbody>
              {fields.map((f) => (
                <tr key={f.name}>
                  <td>{f.name}</td>
                  <td className="muted">{f.type.toLowerCase()}</td>
                  <td className={`num${(missing.get(f.name) ?? 0) > 0 ? " miss" : ""}`}>{missing.has(f.name) ? missing.get(f.name)!.toLocaleString() : ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
