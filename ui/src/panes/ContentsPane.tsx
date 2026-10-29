import { addData } from "../actions";
import { Menu, MenuDivider, MenuItem, MenuList, MenuPopover, Tooltip } from "@fluentui/react-components";
import {
  ArrowExportRegular,
  SettingsRegular,
  FilterRegular,
  LayerDiagonalAddRegular,
  FlowchartRegular,
  CopyRegular,
  DeleteRegular,
  DocumentPdfRegular,
  EditRegular,
  GlobeRegular,
  MapRegular,
  PaintBrushRegular,
  TableRegular,
  Warning16Filled,
  ZoomFitRegular,
} from "@fluentui/react-icons";
import { useEffect, useRef, useState } from "react";
import { applyStyleToLayers, exportLayer, exportTable, moveLayers, pickedLayers, removeLayers, setLayersVisible, zoomToLayers, layerFromSelection, splitByAttribute, openDefinitionQuery, pickMapCrs, removeLayer, useDialog, zoomToLayer } from "../actions";
import { openLayerProperties } from "../dialogs/LayerPropertiesWindow";
import { justDragged, startDrag, type DropDetail } from "../dnd";
import type { LayerInfo } from "../ipc/api";
import { useApp, type LayerStyle } from "../store/store";
import { useLayouts } from "../store/layout";
import { deleteLayout, duplicateLayout, newLayout, openLayout, renameLayout } from "../layout/actions";
import { nextSelection } from "../layers/multiselect";
import { showPane } from "./panes";

export function Swatch({ info, style }: { info: LayerInfo; style?: LayerStyle }) {
  if (info.geomType === "Table") return <TableRegular className="swatch-icon" />;
  if (info.geomType === "Raster")
    return (
      <svg className="swatch" viewBox="0 0 16 16">
        <defs>
          <linearGradient id="rg" x1="0" y1="1" x2="1" y2="0">
            <stop offset="0" stopColor="#467850" />
            <stop offset="0.5" stopColor="#cccc8a" />
            <stop offset="1" stopColor="#f5f5f5" />
          </linearGradient>
        </defs>
        <rect x="1.5" y="2.5" width="13" height="11" fill="url(#rg)" stroke="#555" strokeWidth="0.8" />
      </svg>
    );
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
  const picked = useApp((s) => s.selectedLayers);
  const results = useApp((s) => s.results);
  const showOriginal = useApp((s) => s.showOriginal);
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const app = useApp.getState;
  const treeRef = useRef<HTMLDivElement>(null);

  // Drop a dragged layer on another row to move it above that row.
  useEffect(() => {
    const el = treeRef.current;
    if (!el) return;
    const onDrop = (e: Event) => {
      const { payload } = (e as CustomEvent<DropDetail>).detail;
      if (payload.kind === "file") return void addData([payload.id]);
      const row = (e.target as HTMLElement).closest("[data-index]") as HTMLElement | null;
      if (payload.kind !== "layer" || !row) return;
      const to = Number(row.dataset.index);
      const order = app().order;
      const from = order.indexOf(payload.id);
      if (from >= 0 && from !== to) app().moveLayer(payload.id, from < to ? to - 1 : to);
    };
    el.addEventListener("gis-drop", onDrop);
    return () => el.removeEventListener("gis-drop", onDrop);
  }, [app]);

  const select = (id: string, mods: { shift?: boolean; toggle?: boolean } = {}) => {
    const next = nextSelection(app().order, { selected: app().selectedLayers, anchor: app().layerAnchor }, id, mods);
    app().setLayerSelection(next.selected, next.anchor);
    // The active layer follows the click, except when Ctrl/Cmd unticks it: then another picked layer takes over.
    const active = next.selected.includes(id) ? id : (next.selected.at(-1) ?? id);
    app().setActiveLayer(active);
    app().setProperties({ kind: "layer", id: active });
  };

  return (
    <div className="contents" onClick={() => setMenu(null)}>
      <div className="contents-head">Drawing Order</div>
      <div className="tree" ref={treeRef} data-drop="contents">
        <div className="tree-map" onClick={pickMapCrs} title="Click to change the map's coordinate system">
          <MapRegular />
          <span>Map</span>
          <span className="tree-crs">
            <GlobeRegular /> {mapCrs ? mapCrs.name : "No coordinate system yet"}
          </span>
        </div>
        {order.length === 0 && (
          <div className="empty-hint">
            Add data from the <b>Map</b> tab, drag files onto the window, or get{" "}
            <button className="linkbtn" onClick={() => useDialog.getState().show({ kind: "tiger" })}>Census sample data</button>.
          </div>
        )}
        {order.length > 0 && <div className="tree-hint">Drag a layer onto the Model to use it as an input, or onto another layer to reorder.</div>}
        {order.map((id, i) => {
          const info = layers[id];
          if (!info) return null;
          const nsel = selection[id]?.length ?? 0;
          return (
            <div
              key={id}
              className={`tree-layer ${active === id || picked.includes(id) ? "active" : ""} ${picked.includes(id) && picked.length > 1 ? "multi" : ""} ${info.temporary ? "temp" : ""}`}
              aria-selected={picked.includes(id)}
              data-drop="layer"
              data-index={i}
              onPointerDown={(e) => {
                if ((e.target as HTMLElement).closest("input, svg.warn")) return;
                startDrag(e, { kind: "layer", id, label: info.name });
              }}
              onClick={(e) => {
                e.stopPropagation();
                if (justDragged()) return;
                select(id, { shift: e.shiftKey, toggle: e.ctrlKey || e.metaKey });
              }}
              onDoubleClick={() => openLayerProperties(id)}
              onContextMenu={(e) => {
                e.preventDefault();
                if (!app().selectedLayers.includes(id)) select(id); // right-click inside the selection keeps it
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
              <Swatch info={(results[id] && !showOriginal[id] ? results[id] : info)} style={styles[id]} />
              <span className="tree-name">{info.name}</span>
              {nsel > 0 && <span className="sel-count" title="Selected features">{nsel.toLocaleString()}</span>}
              {info.definitionQuery && (
                <FilterRegular
                  className="tree-query"
                  title={`Definition query: ${info.definitionQuery} (${(info.queryCount ?? 0).toLocaleString()} of ${info.featureCount.toLocaleString()})`}
                  onClick={(e) => {
                    e.stopPropagation();
                    openDefinitionQuery(id);
                  }}
                />
              )}
              {results[id] && (
                <FlowchartRegular
                  className={`tree-result ${showOriginal[id] ? "off" : ""}`}
                  title={showOriginal[id] ? "Showing original data (node result hidden) — click to show the result" : "Showing this layer's node-tree result — click to show the original data"}
                  onClick={(e) => {
                    e.stopPropagation();
                    app().setShowOriginal(id, !showOriginal[id]);
                  }}
                />
              )}
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
      <LayoutsSection />
      {menu && (picked.length > 1 && picked.includes(menu.id) ? <MultiMenu {...menu} onClose={() => setMenu(null)} /> : <LayerMenu {...menu} onClose={() => setMenu(null)} />)}
    </div>
  );
}

/** The project's print layouts: click opens the layout tab. */
function LayoutsSection() {
  useLayouts((s) => s.docs);
  useLayouts((s) => s.doc);
  const active = useLayouts((s) => s.id);
  const layouts = useLayouts.getState().list();
  return (
    <div className="layouts" onClick={(e) => e.stopPropagation()}>
      <div className="layouts-head">
        <span>Layouts</span>
        <button className="linkbtn" onClick={() => newLayout()}>New</button>
      </div>
      {layouts.length === 0 && <div className="empty-hint">No layouts yet. Insert → New Layout creates a print map page.</div>}
      {layouts.map((l) => (
        <div key={l.id} className={`layout-row ${active === l.id ? "active" : ""}`} onClick={() => openLayout(l.id)} onDoubleClick={() => openLayout(l.id)}>
          <DocumentPdfRegular />
          <span className="tree-name">{l.name}</span>
          <span className="row-actions">
            <button title="Rename" aria-label={`Rename ${l.name}`} onClick={(e) => { e.stopPropagation(); renameLayout(l.id); }}><EditRegular /></button>
            <button title="Duplicate" aria-label={`Duplicate ${l.name}`} onClick={(e) => { e.stopPropagation(); duplicateLayout(l.id); }}><CopyRegular /></button>
            <button title="Delete" aria-label={`Delete ${l.name}`} onClick={(e) => { e.stopPropagation(); deleteLayout(l.id); }}><DeleteRegular /></button>
          </span>
        </div>
      ))}
    </div>
  );
}

function LayerMenu({ x, y, id, onClose }: { x: number; y: number; id: string; onClose: () => void }) {
  const info = useApp((s) => s.layers[id]);
  const hasResult = useApp((s) => !!s.results[id]);
  const nsel = useApp((s) => s.selection[id]?.length ?? 0);
  const showingOriginal = useApp((s) => !!s.showOriginal[id]);
  const target = { getBoundingClientRect: () => ({ x, y, left: x, top: y, right: x, bottom: y, width: 0, height: 0, toJSON: () => null }) };
  const dlg = useDialog.getState().show;
  if (!info) return null;
  return (
    <Menu open positioning={{ target: target as never, position: "below", align: "start" }} onOpenChange={(_, d) => !d.open && onClose()}>
      <MenuPopover>
        <MenuList>
          <MenuItem icon={<ZoomFitRegular />} disabled={!info.extent} onClick={() => zoomToLayer(id)}>Zoom To Layer</MenuItem>
          <MenuItem icon={<TableRegular />} onClick={() => { useApp.getState().setActiveLayer(id); showPane("table"); }}>Attribute Table</MenuItem>
          <MenuItem icon={<PaintBrushRegular />} disabled={info.geomType === "Table"} onClick={() => openLayerProperties(id, "symbology")}>Symbology…</MenuItem>
          <MenuItem icon={<FlowchartRegular />} onClick={() => showPane("model")}>Edit Node Tree</MenuItem>
          {hasResult && (
            <MenuItem icon={<FlowchartRegular />} onClick={() => useApp.getState().setShowOriginal(id, !showingOriginal)}>
              {showingOriginal ? "Show Node Result" : "Show Original Data"}
            </MenuItem>
          )}
          <MenuDivider />
          <MenuItem icon={<FilterRegular />} onClick={() => openDefinitionQuery(id)}>Definition Query…</MenuItem>
          <MenuItem icon={<GlobeRegular />} onClick={() => dlg({ kind: "defineProjection", layer: id })}>Define Projection…</MenuItem>
          <MenuItem icon={<ArrowExportRegular />} onClick={() => exportLayer(id)}>Export Features…</MenuItem>
          <MenuItem icon={<ArrowExportRegular />} disabled={info.geomType !== "Point"} onClick={() => exportTable(id)}>Export Table…</MenuItem>
          <MenuItem icon={<LayerDiagonalAddRegular />} disabled={info.geomType === "Raster"} onClick={() => splitByAttribute(id)}>Split by Attribute…</MenuItem>
          <MenuItem icon={<LayerDiagonalAddRegular />} disabled={!nsel} onClick={() => layerFromSelection(id)}>Create Layer From Selection{nsel ? ` (${nsel.toLocaleString()})` : ""}</MenuItem>
          <MenuItem icon={<EditRegular />} onClick={() => dlg({ kind: "rename", layer: id })}>Rename…</MenuItem>
          <MenuDivider />
          <MenuItem icon={<DeleteRegular />} onClick={() => removeLayer(id)}>Remove</MenuItem>
          <MenuDivider />
          <MenuItem icon={<SettingsRegular />} onClick={() => openLayerProperties(id)}>Properties…</MenuItem>
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** Menu for several picked layers (Shift/Ctrl/Cmd-click): actions that make sense on all of them. */
function MultiMenu({ x, y, id, onClose }: { x: number; y: number; id: string; onClose: () => void }) {
  const ids = pickedLayers();
  const names = useApp((s) => s.layers);
  const vector = ids.filter((i) => names[i] && names[i].geomType !== "Raster" && names[i].geomType !== "Table");
  const target = { getBoundingClientRect: () => ({ x, y, left: x, top: y, right: x, bottom: y, width: 0, height: 0, toJSON: () => null }) };
  const from = names[id];
  return (
    <Menu open positioning={{ target: target as never, position: "below", align: "start" }} onOpenChange={(_, d) => !d.open && onClose()}>
      <MenuPopover>
        <MenuList>
          <MenuItem disabled>{ids.length} layers selected</MenuItem>
          <MenuItem icon={<ZoomFitRegular />} onClick={() => zoomToLayers(ids)}>Zoom To Layers</MenuItem>
          <MenuItem onClick={() => setLayersVisible(ids, true)}>Show All</MenuItem>
          <MenuItem onClick={() => setLayersVisible(ids, false)}>Hide All</MenuItem>
          <MenuDivider />
          <MenuItem onClick={() => moveLayers(ids, "top")}>Move To Top</MenuItem>
          <MenuItem onClick={() => moveLayers(ids, "up")}>Move Up</MenuItem>
          <MenuItem onClick={() => moveLayers(ids, "down")}>Move Down</MenuItem>
          <MenuItem onClick={() => moveLayers(ids, "bottom")}>Move To Bottom</MenuItem>
          <MenuItem onClick={() => moveLayers(ids, "gather")}>Gather Together</MenuItem>
          <MenuDivider />
          <MenuItem icon={<PaintBrushRegular />} disabled={!vector.includes(id) || vector.length < 2} onClick={() => applyStyleToLayers(id, vector, "symbology")}>Apply Symbology of “{from?.name}” To Others</MenuItem>
          <MenuItem icon={<PaintBrushRegular />} disabled={!vector.includes(id) || vector.length < 2} onClick={() => applyStyleToLayers(id, vector, "labels")}>Apply Labels of “{from?.name}” To Others</MenuItem>
          <MenuDivider />
          <MenuItem icon={<ArrowExportRegular />} onClick={() => useDialog.getState().show({ kind: "exportLayers", ids })}>Export Layers…</MenuItem>
          <MenuItem icon={<DeleteRegular />} onClick={() => void removeLayers(ids)}>Remove {ids.length} Layers</MenuItem>
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}
