// The Insert rail beside the layout page (option B): what goes on the page, and the project's maps to drag onto it (add-on 3).
// The commands are the ribbon's own (ribbon/commands.tsx); only their arrangement is here.
import { Tooltip } from "@fluentui/react-components";
import { commands } from "../ribbon/commands";
import { RAIL } from "../ribbon/tabsDef";
import { useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { insertMapFrame } from "./actions";
import { frames, followedMap } from "./types";

export const MAP_DRAG = "application/x-gis-map";

export function Rail() {
  useLayouts((s) => s.doc); // enabled state of the buttons follows the page
  useLayouts((s) => s.selected);
  const maps = useApp((s) => s.maps);
  const doc = useLayouts((s) => s.doc);
  const onPage = new Set(doc ? frames(doc).map((f) => followedMap(f.layers)).filter((m): m is string => !!m) : []);
  return (
    <div className="rail" role="toolbar" aria-label="Insert" aria-orientation="vertical" data-testid="insert-rail">
      <div className="rail-head">Insert</div>
      {RAIL.map((g, gi) => (
        <div key={g.label} className="rail-group" role="group" aria-label={g.label}>
          {gi > 0 && <hr />}
          <div className="rail-label">{g.label}</div>
          {g.items.map((it, i) => {
            if ("custom" in it) return <div key={i} className="rail-item">{it.custom}</div>;
            const c = commands[it.cmd];
            const btn = (
              <button className="rbtn rail-btn" disabled={c.enabled ? !c.enabled() : false} onClick={() => c.run()}>
                <span className="rbtn-icon">{c.icon}</span>
                <span className="rbtn-label">{c.label}</span>
              </button>
            );
            return (
              <div key={it.cmd} className="rail-item">
                {c.tip ? <Tooltip content={c.tip} relationship="description" withArrow positioning="after">{btn}</Tooltip> : btn}
              </div>
            );
          })}
        </div>
      ))}
      {maps.length > 0 && (
        <div className="rail-group rail-maps" role="group" aria-label="Maps">
          <hr />
          <div className="rail-label">Maps</div>
          <div className="rail-hint">Drag onto the page</div>
          {maps.map((m) => (
            <button
              key={m.id}
              className="rail-map"
              draggable
              disabled={!doc}
              data-map={m.id}
              title={`${m.name}${onPage.has(m.id) ? " (already on this page)" : ""}: drag onto the page, or click, to add a frame that draws this map`}
              onDragStart={(e) => { e.dataTransfer.setData(MAP_DRAG, m.id); e.dataTransfer.effectAllowed = "copy"; }}
              onClick={() => insertMapFrame({ layers: { map: m.id } })}
            >
              <i className="rail-map-thumb" aria-hidden />
              <span className="rail-map-name">{m.name}</span>
              {onPage.has(m.id) && <span className="rail-map-on">on page</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
