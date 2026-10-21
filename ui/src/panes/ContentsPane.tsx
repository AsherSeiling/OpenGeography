import { Menu, MenuDivider, MenuItem, MenuList, MenuPopover, Tooltip } from "@fluentui/react-components";
import {
  ArrowExportRegular,
  DeleteRegular,
  EditRegular,
  GlobeRegular,
  MapRegular,
  PaintBrushRegular,
  TableRegular,
  Warning16Filled,
  ZoomFitRegular,
} from "@fluentui/react-icons";
import { useState } from "react";
import { exportLayer, pickMapCrs, removeLayer, useDialog, zoomToLayer } from "../actions";
import type { LayerInfo } from "../ipc/api";
import { useApp, type LayerStyle } from "../store/store";
import { showPane } from "./panes";

export function Swatch({ info, style }: { info: LayerInfo; style?: LayerStyle }) {
  if (info.geomType === "Table") return <TableRegular className="swatch-icon" />;
  const fill = style?.fill ?? "#999";
  const stroke = style?.stroke ?? "#444";
  if (info.geomType === "Point")
    return (
      <svg className="swatch" viewBox="0 0 16 16">
        <circle cx="8" cy="8" r="4.5" fill={fill} stroke="#333" strokeWidth="0.8" />
      </svg>
    );
  if (info.geomType === "Line")
    return (
      <svg className="swatch" viewBox="0 0 16 16">
        <path d="M1 12 L6 5 L10 10 L15 3" fill="none" stroke={fill} strokeWidth="2" />
      </svg>
    );
  return (
    <svg className="swatch" viewBox="0 0 16 16">
      <rect x="1.5" y="3" width="13" height="10" fill={fill} stroke={stroke} strokeWidth="1" />
    </svg>
  );
}

export function ContentsPane() {
  const order = useApp((s) => s.order);
  const layers = useApp((s) => s.layers);
  const visible = useApp((s) => s.visible);
  const styles = useApp((s) => s.styles);
  const active = useApp((s) => s.activeLayer);
  const mapCrs = useApp((s) => s.mapCrs);
  const selection = useApp((s) => s.selection);
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [drag, setDrag] = useState<string | null>(null);
  const [dropAt, setDropAt] = useState<number | null>(null);
  const app = useApp.getState;

  const select = (id: string) => {
    app().setActiveLayer(id);
    app().setProperties({ kind: "layer", id });
  };

  return (
    <div className="contents" onClick={() => setMenu(null)}>
      <div className="contents-head">Drawing Order</div>
      <div className="tree">
        <div className="tree-map" onClick={pickMapCrs} title="Click to change the map's coordinate system">
          <MapRegular />
          <span>Map</span>
          <span className="tree-crs">
            <GlobeRegular /> {mapCrs ? mapCrs.name : "No coordinate system yet"}
          </span>
        </div>
        {order.length === 0 && (
          <div className="empty-hint">
            Add data from the <b>Map</b> tab, or drag files onto the window.
          </div>
        )}
        {order.map((id, i) => {
          const info = layers[id];
          if (!info) return null;
          const nsel = selection[id]?.length ?? 0;
          return (
            <div
              key={id}
              className={`tree-layer ${active === id ? "active" : ""} ${dropAt === i ? "drop-before" : ""} ${info.temporary ? "temp" : ""}`}
              draggable
              onDragStart={(e) => {
                setDrag(id);
                e.dataTransfer.setData("application/gis-layer", id);
                e.dataTransfer.effectAllowed = "copyMove";
              }}
              onDragOver={(e) => {
                if (!drag) return;
                e.preventDefault();
                setDropAt(i);
              }}
              onDragEnd={() => {
                setDrag(null);
                setDropAt(null);
              }}
              onDrop={(e) => {
                e.preventDefault();
                if (drag && drag !== id) app().moveLayer(drag, order.indexOf(drag) < i ? i - 1 : i);
                setDrag(null);
                setDropAt(null);
              }}
              onClick={(e) => {
                e.stopPropagation();
                select(id);
              }}
              onDoubleClick={() => showPane("properties")}
              onContextMenu={(e) => {
                e.preventDefault();
                select(id);
                setMenu({ x: e.clientX, y: e.clientY, id });
              }}
            >
              <input
                type="checkbox"
                checked={!!visible[id]}
                disabled={info.geomType === "Table"}
                onClick={(e) => e.stopPropagation()}
                onChange={(e) => app().setVisible(id, e.target.checked)}
                aria-label={`Show ${info.name}`}
              />
              <Swatch info={info} style={styles[id]} />
              <span className="tree-name">{info.name}</span>
              {nsel > 0 && <span className="sel-count" title="Selected features">{nsel.toLocaleString()}</span>}
              {(!info.crs && info.geomType !== "Table") || info.warnings.length ? (
                <Tooltip content={info.warnings.join(" ") || "No coordinate system"} relationship="description">
                  <Warning16Filled
                    className="warn"
                    onClick={(e) => {
                      e.stopPropagation();
                      useDialog.getState().show({ kind: "defineProjection", layer: id });
                    }}
                  />
                </Tooltip>
              ) : null}
            </div>
          );
        })}
      </div>
      {menu && <LayerMenu {...menu} onClose={() => setMenu(null)} />}
    </div>
  );
}

function LayerMenu({ x, y, id, onClose }: { x: number; y: number; id: string; onClose: () => void }) {
  const info = useApp((s) => s.layers[id]);
  const target = { getBoundingClientRect: () => ({ x, y, left: x, top: y, right: x, bottom: y, width: 0, height: 0, toJSON: () => null }) };
  const dlg = useDialog.getState().show;
  if (!info) return null;
  return (
    <Menu open positioning={{ target: target as never, position: "below", align: "start" }} onOpenChange={(_, d) => !d.open && onClose()}>
      <MenuPopover>
        <MenuList>
          <MenuItem icon={<ZoomFitRegular />} disabled={!info.extent} onClick={() => zoomToLayer(id)}>Zoom To Layer</MenuItem>
          <MenuItem icon={<TableRegular />} onClick={() => { useApp.getState().setActiveLayer(id); showPane("table"); }}>Attribute Table</MenuItem>
          <MenuItem icon={<PaintBrushRegular />} disabled={info.geomType === "Table"} onClick={() => showPane("properties")}>Symbology…</MenuItem>
          <MenuDivider />
          <MenuItem icon={<GlobeRegular />} onClick={() => dlg({ kind: "defineProjection", layer: id })}>Define Projection…</MenuItem>
          <MenuItem icon={<ArrowExportRegular />} onClick={() => exportLayer(id)}>Export Features…</MenuItem>
          <MenuItem icon={<EditRegular />} onClick={() => dlg({ kind: "rename", layer: id })}>Rename…</MenuItem>
          <MenuDivider />
          <MenuItem icon={<DeleteRegular />} onClick={() => removeLayer(id)}>Remove</MenuItem>
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}
