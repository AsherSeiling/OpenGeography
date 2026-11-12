// Catalog pane: one tree. Project (Layouts, Project files) and Public Data (the
// download dialogs). Files are added through Add Data or by dropping them on the
// window; files already in the project are listed under Project files.
import { Menu, MenuItem, MenuList, MenuPopover } from "@fluentui/react-components";
import {
  AddRegular,
  ArrowDownloadRegular,
  BriefcaseRegular,
  BuildingMultipleRegular,
  ChevronDownRegular,
  ChevronRightRegular,
  CopyRegular,
  DatabaseRegular,
  DeleteRegular,
  DocumentPdfRegular,
  DocumentRegular,
  EditRegular,
  FolderRegular,
  FolderZipRegular,
  GlobeRegular,
  ImageMultipleRegular,
  ImageRegular,
  MapPinRegular,
  MapRegular,
  MountainTrailRegular,
  PlugConnectedRegular,
  ServerRegular,
  ShapesRegular,
  TableRegular,
  WarningRegular,
} from "@fluentui/react-icons";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { addData, openProject, useDialog, type DialogState } from "../actions";
import { startDrag } from "../dnd";
import { api, errorText, type FsEntry } from "../ipc/api";
import { deleteLayout, duplicateLayout, newLayout, openLayout, renameLayout } from "../layout/actions";
import { useShallow } from "zustand/react/shallow";
import { useLayouts } from "../store/layout";
import { useModel } from "../store/model";
import { useApp } from "../store/store";
import { deleteMap, duplicateMap, newMap, openMap, renameMap } from "../maps/mapActions";
import { addToolToModel } from "./ModelPane";
import { zoomToLayer } from "../actions";
import { openLayerProperties } from "../dialogs/LayerPropertiesWindow";
import { showPane } from "./panes";
import { projectLayerInfos } from "../store/maps";
const NO_ORDER: string[] = []; // a stable empty list: a fresh [] from a store selector re-renders forever when no map is active

const ADDABLE = new Set(["vector", "raster", "table", "archive", "gdb"]);

/** One row per public-data dialog. Each opens the dialog the ribbon command used to open. */
export const PUBLIC_DATA: { id: string; label: string; tip: string; icon: ReactNode; dialog: DialogState }[] = [
  { id: "tiger", label: "Census TIGER/Line", tip: "Download Census boundaries, roads and water for any state or county", icon: <ArrowDownloadRegular />, dialog: { kind: "tiger" } },
  { id: "nationalMap", label: "USGS National Map", tip: "Download USGS streams, lakes, watersheds, roads, trails, structures, place names and federal lands", icon: <MapPinRegular />, dialog: { kind: "nationalMap" } },
  { id: "elevation", label: "Elevation (3DEP)", tip: "Download USGS 3DEP elevation for the map view or a layer's area", icon: <MountainTrailRegular />, dialog: { kind: "elevation" } },
  { id: "stac", label: "Imagery & DEM (STAC)", tip: "Search Sentinel-2 imagery and the Copernicus DEM and clip a scene to your area", icon: <ImageMultipleRegular />, dialog: { kind: "stac" } },
  { id: "overture", label: "Overture Maps", tip: "Download buildings, places, roads, water, land use or boundaries for your area", icon: <BuildingMultipleRegular />, dialog: { kind: "overture" } },
  { id: "hazardsSoils", label: "Earthquakes & Soils", tip: "Download USGS earthquakes or NRCS soil map units for your area", icon: <WarningRegular />, dialog: { kind: "hazardsSoils" } },
  { id: "arcgis", label: "ArcGIS Services", tip: "Download PAD-US, FEMA flood zones, wetlands, wildfire perimeters or any ArcGIS REST layer", icon: <ServerRegular />, dialog: { kind: "arcgis" } },
  { id: "ogc", label: "OGC API Features", tip: "Download features from USGS Water Data or any OGC API – Features service", icon: <PlugConnectedRegular />, dialog: { kind: "ogc" } },
];

function KindIcon({ kind }: { kind: string }) {
  switch (kind) {
    case "folder":
      return <FolderRegular className="cat-ico folder" />;
    case "project":
      return <BriefcaseRegular className="cat-ico project" />;
    case "gdb":
      return <DatabaseRegular className="cat-ico gdb" />;
    case "vector":
      return <ShapesRegular className="cat-ico vector" />;
    case "raster":
      return <ImageRegular className="cat-ico raster" />;
    case "table":
      return <TableRegular className="cat-ico table" />;
    case "archive":
      return <FolderZipRegular className="cat-ico archive" />;
    default:
      return <DocumentRegular className="cat-ico file" />;
  }
}

const sizeText = (n: number | null) =>
  n === null ? "" : n < 1024 ? `${n} B` : n < 1024 ** 2 ? `${(n / 1024).toFixed(0)} KB` : n < 1024 ** 3 ? `${(n / 1024 ** 2).toFixed(1)} MB` : `${(n / 1024 ** 3).toFixed(1)} GB`;
const dateText = (s: number | null) => (s ? new Date(s * 1000).toLocaleDateString() : "");
const baseName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const parentOf = (p: string) => p.replace(/[\\/][^\\/]+$/, "") || p;
const isAbsolute = (p: string) => p.startsWith("/") || /^[A-Za-z]:[\\/]/.test(p) || p.startsWith("\\\\");

interface UsedFile {
  path: string;
  entry: FsEntry | null;
  layers: string[];
}

/**
 * The local files the project's layers came from (from the source log, plus layer
 * sources), and the project folder itself. Files moved or deleted since they were
 * added come back with `entry: null`.
 */
function useProjectFiles() {
  const layers = useApp((s) => s.layers);
  const projectDir = useApp((s) => s.projectDir);
  const [files, setFiles] = useState<UsedFile[] | null>(null);
  const [extra, setExtra] = useState<FsEntry[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const layerKey = Object.values(layers)
    .map((l) => `${l.id}:${l.name}:${l.source ?? ""}`)
    .join("|");

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const present = new Set(Object.keys(layers));
        const used = new Map<string, Set<string>>();
        const add = (path: string, layer: string) => {
          if (!used.has(path)) used.set(path, new Set());
          used.get(path)!.add(layer);
        };
        // Files recorded in the source log, for layers still in the project.
        for (const s of await api.sources()) {
          if (!s.citation.key.startsWith("file:")) continue;
          const path = s.citation.key.slice(5);
          for (const u of s.uses) if (present.has(u.layerId)) add(path, u.layerName);
        }
        // Layers whose source is a local path (e.g. added before the log existed).
        for (const l of Object.values(layers)) if (l.source && isAbsolute(l.source) && !l.resultOf) add(l.source, l.name);
        const paths = [...used.keys()];
        const extraPaths = projectDir ? [projectDir, `${projectDir}/Bibliography`] : [];
        const states = await api.fsStat([...paths, ...extraPaths]);
        if (!alive) return;
        setFiles(paths.map((path, i) => ({ path, entry: states[i].entry, layers: [...used.get(path)!].sort() })));
        setExtra(states.slice(paths.length).flatMap((s) => (s.entry ? [s.entry] : [])));
        setErr(null);
      } catch (e) {
        if (alive) setErr(errorText(e));
      }
    })();
    return () => {
      alive = false;
    };
  }, [layerKey, projectDir]);

  const groups = useMemo(() => {
    const m = new Map<string, UsedFile[]>();
    for (const f of files ?? []) {
      const dir = parentOf(f.path);
      m.set(dir, [...(m.get(dir) ?? []), f]);
    }
    return [...m.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [files]);

  return { files, extra, groups, err };
}

/** Branches that start expanded. */
const DEFAULT_OPEN: Record<string, boolean> = { project: true, maps: true, layouts: true, layers: true, files: false, public: true };

interface RowProps {
  id: string;
  level: number;
  icon: ReactNode;
  label: ReactNode;
  /** Present on branches: whether the branch is open. */
  open?: boolean;
  selected?: boolean;
  className?: string;
  title?: string;
  /** Cells after the label (counts, buttons, sizes). */
  trailing?: ReactNode;
  onSelect?: () => void;
  onActivate?: () => void;
  onToggle?: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  onPointerDown?: (e: React.PointerEvent) => void;
}

/** One tree row. Branches toggle on click; leaves select on click and activate on double-click or Enter. */
function Row(p: RowProps) {
  const branch = p.open !== undefined;
  return (
    <div
      role="treeitem"
      tabIndex={-1}
      data-node={p.id}
      aria-level={p.level}
      aria-expanded={branch ? p.open : undefined}
      aria-selected={!!p.selected}
      title={p.title}
      className={`cat-row ${branch ? "branch" : "leaf"} ${p.selected ? "sel" : ""} ${p.className ?? ""}`}
      style={{ paddingLeft: 6 + (p.level - 1) * 14 }}
      onClick={() => (branch ? p.onToggle?.() : p.onSelect?.())}
      onDoubleClick={() => !branch && p.onActivate?.()}
      onContextMenu={p.onContextMenu}
      onPointerDown={p.onPointerDown}
    >
      <span className="cat-chev">{branch ? p.open ? <ChevronDownRegular /> : <ChevronRightRegular /> : null}</span>
      <span className="cat-icon">{p.icon}</span>
      <span className="cat-name">{p.label}</span>
      {p.trailing}
    </div>
  );
}

export function CatalogPane() {
  const [open, setOpen] = useState<Record<string, boolean>>(DEFAULT_OPEN);
  const [selected, setSelected] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; entry: FsEntry } | null>(null);
  const treeRef = useRef<HTMLDivElement>(null);
  const show = useDialog((s) => s.show);
  const { files, extra, groups, err } = useProjectFiles();
  const missing = (files ?? []).filter((f) => !f.entry).length;

  useLayouts((s) => s.docs);
  useLayouts((s) => s.doc);
  const activeLayout = useLayouts((s) => s.id);
  const layouts = useLayouts.getState().list();
  const maps = useApp((s) => s.maps);
  const activeMapId = useApp((s) => s.activeMapId);
  // Every layer of the project, whichever map shows it (imported ones included): the active map's first, then the rest in the project's order.
  const projectLayers = useApp(useShallow((s) => projectLayerInfos(s).filter(Boolean)));
  const inActiveMap = useApp((s) => s.maps.find((m) => m.id === s.activeMapId)?.order ?? NO_ORDER);
  const shownIn = (id: string) => maps.filter((m) => m.order.includes(id)).map((m) => m.name);
  /** Put the layer on the active map if it isn't there, then show it (a table opens the Attribute Table instead). */
  const openLayer = (id: string) => {
    const s = useApp.getState();
    const info = s.layers[id];
    if (!info) return;
    if (!inActiveMap.includes(id)) s.addToMap([id]);
    s.set({ activeLayer: id });
    if (info.geomType === "Table") showPane("table");
    else void zoomToLayer(id);
  };

  const isOpen = (k: string) => open[k] ?? DEFAULT_OPEN[k] ?? true;
  const toggle = (k: string) => setOpen((o) => ({ ...o, [k]: !isOpen(k) }));
  const items = () => [...(treeRef.current?.querySelectorAll<HTMLElement>('[role="treeitem"]') ?? [])];
  const focusItem = (el: HTMLElement | undefined) => {
    if (!el) return;
    el.focus();
    setSelected(el.dataset.node ?? null);
  };

  // Arrow keys move, Right/Left open and close (or step into / out of a branch), Enter or Space acts on the row.
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>('[role="treeitem"]');
    if (!row || (e.target as HTMLElement) !== row) return;
    const all = items();
    const i = all.indexOf(row);
    const level = Number(row.getAttribute("aria-level"));
    const expanded = row.getAttribute("aria-expanded");
    const key = row.dataset.node!;
    switch (e.key) {
      case "ArrowDown":
        focusItem(all[i + 1]);
        break;
      case "ArrowUp":
        focusItem(all[i - 1]);
        break;
      case "Home":
        focusItem(all[0]);
        break;
      case "End":
        focusItem(all[all.length - 1]);
        break;
      case "ArrowRight":
        if (expanded === "false") setOpen((o) => ({ ...o, [key]: true }));
        else if (expanded === "true") focusItem(all[i + 1]);
        break;
      case "ArrowLeft":
        if (expanded === "true") setOpen((o) => ({ ...o, [key]: false }));
        else focusItem([...all.slice(0, i)].reverse().find((r) => Number(r.getAttribute("aria-level")) < level));
        break;
      case "Enter":
      case " ":
        if (expanded !== null) toggle(key);
        else row.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
        break;
      case "F2":
        if (key.startsWith("layout:")) renameLayout(key.slice(7));
        if (key.startsWith("map:")) renameMap(key.slice(4));
        break;
      case "Delete":
        if (key.startsWith("layout:")) deleteLayout(key.slice(7));
        if (key.startsWith("map:")) deleteMap(key.slice(4));
        break;
      default:
        return;
    }
    e.preventDefault();
  };

  // Roving focus: one row is in the tab order, the selected one (else the first).
  useEffect(() => {
    const all = items();
    all.forEach((r) => (r.tabIndex = -1));
    (all.find((r) => r.dataset.node === selected) ?? all[0])?.setAttribute("tabindex", "0");
  });

  const addToTree = async (e: FsEntry) => {
    const owner = useModel.getState().layer;
    const infos = await addData([e.path]);
    if (infos[0] && owner) addToolToModel("input_layer", { layer: infos[0].id });
  };

  const fileRow = (f: UsedFile, level: number) => {
    const e: FsEntry = f.entry ?? { name: baseName(f.path), path: f.path, kind: "file", size: null, modified: null };
    const id = `file:${f.path}`;
    return (
      <Row
        key={id}
        id={id}
        level={level}
        icon={<KindIcon kind={e.kind} />}
        className={`file ${e.kind} ${f.entry ? "" : "missing"}`}
        selected={selected === id}
        title={f.entry ? f.path : `${f.path} — missing: moved, renamed or deleted since it was added`}
        label={
          <>
            {e.name}
            <span className="cat-used">{f.entry ? `used by ${f.layers.join(", ")}` : "missing — moved or deleted"}</span>
          </>
        }
        trailing={
          <>
            <span className="cat-meta">{sizeText(e.size)}</span>
            <span className="cat-meta date">{dateText(e.modified)}</span>
          </>
        }
        onSelect={() => setSelected(id)}
        onActivate={() => f.entry && ADDABLE.has(e.kind) && void addData([f.path])}
        onContextMenu={(ev) => {
          ev.preventDefault();
          setSelected(id);
          if (f.entry) setMenu({ x: ev.clientX, y: ev.clientY, entry: e });
        }}
        onPointerDown={(ev) => f.entry && ADDABLE.has(e.kind) && startDrag(ev, { kind: "file", id: f.path, label: e.name })}
      />
    );
  };

  return (
    <div className="pane catalog-pane" onClick={() => setMenu(null)}>
      {err && <div className="src-warn">{err}</div>}
      <div className="cat-tree" role="tree" aria-label="Catalog" ref={treeRef} onKeyDown={onKeyDown}>
        <Row id="project" level={1} icon={<BriefcaseRegular />} label="Project" className="head" open={isOpen("project")} onToggle={() => toggle("project")} />
        {isOpen("project") && (
          <>
            <Row
              id="maps"
              level={2}
              icon={<MapRegular />}
              label="Maps"
              open={isOpen("maps")}
              onToggle={() => toggle("maps")}
              trailing={
                <>
                  <span className="cat-meta">{maps.length}</span>
                  <button className="linkbtn cat-new" onClick={(e) => (e.stopPropagation(), newMap())}>
                    New
                  </button>
                </>
              }
            />
            {isOpen("maps") &&
              maps.map((m) => (
                <Row
                  key={m.id}
                  id={`map:${m.id}`}
                  level={3}
                  icon={<MapRegular />}
                  label={m.name}
                  className={`map-row ${activeMapId === m.id ? "active" : ""}`}
                  title={`${m.name}: ${m.order.length} layer${m.order.length === 1 ? "" : "s"} · ${m.crs?.name ?? "no coordinate system yet"}. Double-click to open.`}
                  selected={selected === `map:${m.id}`}
                  onSelect={() => setSelected(`map:${m.id}`)}
                  onActivate={() => openMap(m.id)}
                  trailing={
                    <span className="row-actions">
                      <button title="Open" aria-label={`Open ${m.name}`} onClick={(e) => (e.stopPropagation(), openMap(m.id))}>
                        <MapRegular />
                      </button>
                      <button title="Rename" aria-label={`Rename ${m.name}`} onClick={(e) => (e.stopPropagation(), renameMap(m.id))}>
                        <EditRegular />
                      </button>
                      <button title="Duplicate" aria-label={`Duplicate ${m.name}`} onClick={(e) => (e.stopPropagation(), duplicateMap(m.id))}>
                        <CopyRegular />
                      </button>
                      <button title="Delete" aria-label={`Delete ${m.name}`} onClick={(e) => (e.stopPropagation(), deleteMap(m.id))}>
                        <DeleteRegular />
                      </button>
                    </span>
                  }
                />
              ))}

            <Row
              id="layouts"
              level={2}
              icon={<DocumentPdfRegular />}
              label="Layouts"
              open={isOpen("layouts")}
              onToggle={() => toggle("layouts")}
              trailing={
                <>
                  <span className="cat-meta">{layouts.length || ""}</span>
                  <button
                    className="linkbtn cat-new"
                    onClick={(e) => {
                      e.stopPropagation();
                      newLayout();
                    }}
                  >
                    New
                  </button>
                </>
              }
            />
            {isOpen("layouts") && layouts.length === 0 && <div className="empty-hint cat-empty">No layouts yet. Insert → New Layout creates a print map page.</div>}
            {isOpen("layouts") &&
              layouts.map((l) => (
                <Row
                  key={l.id}
                  id={`layout:${l.id}`}
                  level={3}
                  icon={<DocumentPdfRegular />}
                  label={l.name}
                  className={`layout-row ${activeLayout === l.id ? "active" : ""}`}
                  selected={selected === `layout:${l.id}`}
                  onSelect={() => setSelected(`layout:${l.id}`)}
                  onActivate={() => openLayout(l.id)}
                  trailing={
                    <span className="row-actions">
                      <button title="Rename" aria-label={`Rename ${l.name}`} onClick={(e) => (e.stopPropagation(), renameLayout(l.id))}>
                        <EditRegular />
                      </button>
                      <button title="Duplicate" aria-label={`Duplicate ${l.name}`} onClick={(e) => (e.stopPropagation(), duplicateLayout(l.id))}>
                        <CopyRegular />
                      </button>
                      <button title="Delete" aria-label={`Delete ${l.name}`} onClick={(e) => (e.stopPropagation(), deleteLayout(l.id))}>
                        <DeleteRegular />
                      </button>
                    </span>
                  }
                />
              ))}

            <Row
              id="layers"
              level={2}
              icon={<ShapesRegular />}
              label="Project layers"
              open={isOpen("layers")}
              onToggle={() => toggle("layers")}
              trailing={<span className="cat-meta">{projectLayers.length || ""}</span>}
            />
            {isOpen("layers") && projectLayers.length === 0 && <div className="empty-hint cat-empty">No layers yet. Add Data, or download some from Public Data below.</div>}
            {isOpen("layers") &&
              projectLayers.map((l) => {
                const id = `player:${l.id}`;
                const maps_ = shownIn(l.id);
                return (
                  <Row
                    key={id}
                    id={id}
                    level={3}
                    icon={l.geomType === "Table" ? <TableRegular className="cat-ico table" /> : l.geomType === "Raster" ? <ImageRegular className="cat-ico raster" /> : <ShapesRegular className="cat-ico vector" />}
                    className={`player ${l.geomType === "Table" ? "table" : ""}`}
                    label={
                      <>
                        {l.name}
                        <span className="cat-used">{maps_.length ? `on ${maps_.join(", ")}` : "on no map"}</span>
                      </>
                    }
                    title={`${l.name}: ${l.geomType === "Table" ? `table, ${l.featureCount.toLocaleString()} rows` : l.geomType === "Raster" ? "raster" : `${l.geomType.toLowerCase()}, ${l.featureCount.toLocaleString()} features`}. Double-click to open; drag onto a map to add it.`}
                    selected={selected === id}
                    onSelect={() => setSelected(id)}
                    onActivate={() => openLayer(l.id)}
                    onPointerDown={(ev) => startDrag(ev, { kind: "layer", id: l.id, label: l.name })}
                    trailing={
                      <span className="row-actions">
                        {!inActiveMap.includes(l.id) && (
                          <button title="Add to the active map" aria-label={`Add ${l.name} to the active map`} onClick={(e) => (e.stopPropagation(), useApp.getState().addToMap([l.id]))}>
                            <AddRegular />
                          </button>
                        )}
                        <button title={l.geomType === "Table" ? "Open the table" : "Open: show and zoom to it"} aria-label={`Open ${l.name}`} onClick={(e) => (e.stopPropagation(), openLayer(l.id))}>
                          <MapRegular />
                        </button>
                        <button title="Properties" aria-label={`Properties of ${l.name}`} onClick={(e) => (e.stopPropagation(), openLayerProperties(l.id))}>
                          <EditRegular />
                        </button>
                      </span>
                    }
                  />
                );
              })}

            <Row
              id="files"
              level={2}
              icon={<DocumentRegular />}
              label="Project files"
              open={isOpen("files")}
              onToggle={() => toggle("files")}
              trailing={<span className="cat-meta">{files === null ? "" : `${files.length}${missing ? ` · ${missing} missing` : ""}`}</span>}
            />
            {isOpen("files") && (
              <>
                {files === null && <div className="empty-hint cat-empty">Looking up the project's files…</div>}
                {extra.map((e) => (
                  <Row
                    key={e.path}
                    id={`extra:${e.path}`}
                    level={3}
                    icon={<KindIcon kind={e.kind} />}
                    className={e.kind}
                    title={e.path}
                    label={e.kind === "project" ? `${e.name} (project folder)` : e.name}
                    selected={selected === `extra:${e.path}`}
                    onSelect={() => setSelected(`extra:${e.path}`)}
                    trailing={<span className="cat-meta date">{dateText(e.modified)}</span>}
                  />
                ))}
                {files !== null && files.length === 0 && (
                  <div className="empty-hint cat-empty">No local files yet. Layers added from files (Add Data, drag-and-drop) are listed here; downloaded data (TIGER, National Map, elevation) is cited in Sources instead.</div>
                )}
                {groups.map(([dir, fs]) => (
                  <div key={dir} role="group">
                    <Row
                      id={`dir:${dir}`}
                      level={3}
                      icon={<FolderRegular className="cat-ico folder" />}
                      title={dir}
                      label={
                        <>
                          {baseName(dir)}
                          <span className="cat-used">{dir}</span>
                        </>
                      }
                      open={isOpen(`dir:${dir}`)}
                      onToggle={() => toggle(`dir:${dir}`)}
                    />
                    {isOpen(`dir:${dir}`) && fs.map((f) => fileRow(f, 4))}
                  </div>
                ))}
              </>
            )}
          </>
        )}

        <Row id="public" level={1} icon={<GlobeRegular />} label="Public Data" className="head" open={isOpen("public")} onToggle={() => toggle("public")} />
        {isOpen("public") &&
          PUBLIC_DATA.map((p) => (
            <Row
              key={p.id}
              id={`public:${p.id}`}
              level={2}
              icon={p.icon}
              label={p.label}
              className="public"
              title={`${p.tip} (double-click or Enter to open)`}
              selected={selected === `public:${p.id}`}
              onSelect={() => setSelected(`public:${p.id}`)}
              onActivate={() => show(p.dialog)}
            />
          ))}
      </div>

      {menu && (
        <Menu
          open
          onOpenChange={(_, d) => !d.open && setMenu(null)}
          positioning={{
            target: { getBoundingClientRect: () => ({ x: menu.x, y: menu.y, left: menu.x, top: menu.y, right: menu.x, bottom: menu.y, width: 0, height: 0, toJSON: () => null }) } as never,
            position: "below",
            align: "start",
          }}
        >
          <MenuPopover>
            <MenuList>
              {ADDABLE.has(menu.entry.kind) && (
                <MenuItem icon={<AddRegular />} onClick={() => addData([menu.entry.path])}>
                  Add to map
                </MenuItem>
              )}
              {ADDABLE.has(menu.entry.kind) && (
                <MenuItem icon={<ShapesRegular />} disabled={!useModel.getState().layer} onClick={() => addToTree(menu.entry)}>
                  Add to the open node tree
                </MenuItem>
              )}
              {menu.entry.kind === "project" && <MenuItem onClick={() => openProject(menu.entry.path)}>Open project</MenuItem>}
              <MenuItem icon={<CopyRegular />} onClick={() => navigator.clipboard.writeText(menu.entry.path)}>
                Copy path
              </MenuItem>
            </MenuList>
          </MenuPopover>
        </Menu>
      )}
    </div>
  );
}
