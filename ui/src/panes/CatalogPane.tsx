// Catalog pane (file manager): browse folders, see GIS data at a glance, and
// add it — double-click, right-click, or drag onto the map, Contents or the
// node editor. Folder connections are remembered per user.
import { Menu, MenuItem, MenuList, MenuPopover, Tooltip } from "@fluentui/react-components";
import {
  AddRegular,
  ArrowClockwiseRegular,
  ArrowLeftRegular,
  ArrowUpRegular,
  BriefcaseRegular,
  CopyRegular,
  DatabaseRegular,
  DesktopRegular,
  DismissRegular,
  DocumentFolderRegular,
  DocumentRegular,
  ArrowDownloadRegular,
  FolderLinkRegular,
  FolderRegular,
  FolderZipRegular,
  HardDriveRegular,
  HomeRegular,
  ImageRegular,
  InfoRegular,
  ShapesRegular,
  StarFilled,
  StarRegular,
  TableRegular,
} from "@fluentui/react-icons";
import { useCallback, useEffect, useMemo, useState } from "react";
import { create } from "zustand";
import { addData, openProject } from "../actions";
import { startDrag } from "../dnd";
import { api, errorText, type FsDetails, type FsEntry, type FsListing, type FsRoot } from "../ipc/api";
import { addToolToModel } from "./ModelPane";
import { useModel } from "../store/model";
import { useApp } from "../store/store";

const KEY = "gis-studio.catalog.v1";

interface CatalogPrefs {
  connections: string[];
  last: string | null;
  gisOnly: boolean;
  set: (p: Partial<Pick<CatalogPrefs, "connections" | "last" | "gisOnly">>) => void;
}

const loadPrefs = () => {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { connections: Array.isArray(v.connections) ? v.connections : [], last: typeof v.last === "string" ? v.last : null, gisOnly: v.gisOnly !== false };
  } catch {
    return { connections: [], last: null, gisOnly: true };
  }
};

/** Per-user catalog state: folder connections, last folder, filter. */
export const useCatalog = create<CatalogPrefs>((set, get) => ({
  ...loadPrefs(),
  set: (p) => {
    set(p);
    try {
      const { connections, last, gisOnly } = get();
      localStorage.setItem(KEY, JSON.stringify({ connections, last, gisOnly }));
    } catch {
      // Not saved; fine for this session.
    }
  },
}));

const ADDABLE = new Set(["vector", "raster", "table", "archive", "gdb"]);

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

function RootIcon({ kind }: { kind: FsRoot["kind"] }) {
  switch (kind) {
    case "home":
      return <HomeRegular />;
    case "desktop":
      return <DesktopRegular />;
    case "documents":
      return <DocumentFolderRegular />;
    case "downloads":
      return <ArrowDownloadRegular />;
    default:
      return <HardDriveRegular />;
  }
}

const sizeText = (n: number | null) =>
  n === null ? "" : n < 1024 ? `${n} B` : n < 1024 ** 2 ? `${(n / 1024).toFixed(0)} KB` : n < 1024 ** 3 ? `${(n / 1024 ** 2).toFixed(1)} MB` : `${(n / 1024 ** 3).toFixed(1)} GB`;
const dateText = (s: number | null) => (s ? new Date(s * 1000).toLocaleDateString() : "");
const baseName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;

export function CatalogPane() {
  const { connections, last, gisOnly } = useCatalog();
  const [roots, setRoots] = useState<FsRoot[]>([]);
  const [listing, setListing] = useState<FsListing | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [pathText, setPathText] = useState("");
  const [filter, setFilter] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [selected, setSelected] = useState<FsEntry | null>(null);
  const [details, setDetails] = useState<FsDetails | string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; entry: FsEntry } | null>(null);

  const go = useCallback(async (path: string, remember = true) => {
    setErr(null);
    try {
      const l = await api.fsList(path);
      setListing((prev) => {
        if (remember && prev && prev.path !== l.path) setHistory((h) => [...h.slice(-30), prev.path]);
        return l;
      });
      setPathText(l.path);
      setSelected(null);
      setDetails(null);
      useCatalog.getState().set({ last: l.path });
    } catch (e) {
      setErr(errorText(e));
    }
  }, []);

  useEffect(() => {
    api.fsRoots()
      .then((r) => {
        setRoots(r);
        const start = last ?? useApp.getState().projectDir?.replace(/[\\/][^\\/]+$/, "") ?? r[0]?.path;
        if (start) void go(start, false);
      })
      .catch((e) => setErr(errorText(e)));
  }, []);

  const refresh = () => listing && go(listing.path, false);
  const back = () => {
    const prev = history[history.length - 1];
    if (!prev) return;
    setHistory((h) => h.slice(0, -1));
    void go(prev, false);
  };
  const connected = !!listing && connections.includes(listing.path);
  const toggleConnection = (path: string) =>
    useCatalog.getState().set({ connections: connections.includes(path) ? connections.filter((c) => c !== path) : [...connections, path] });

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return (listing?.entries ?? []).filter((e) => (!gisOnly || e.kind !== "file") && (!q || e.name.toLowerCase().includes(q)));
  }, [listing, filter, gisOnly]);

  const open = (e: FsEntry) => {
    if (e.kind === "folder") return void go(e.path);
    if (e.kind === "project") return void openProject(e.path);
    if (ADDABLE.has(e.kind)) return void addData([e.path]);
  };

  const inspect = async (e: FsEntry) => {
    setSelected(e);
    setDetails(null);
    if (!ADDABLE.has(e.kind)) return;
    try {
      setDetails(await api.fsInspect(e.path));
    } catch (x) {
      setDetails(errorText(x));
    }
  };

  const addToTree = async (e: FsEntry) => {
    const owner = useModel.getState().layer;
    const infos = await addData([e.path]);
    if (infos[0] && owner) addToolToModel("input_layer", { layer: infos[0].id });
  };

  return (
    <div className="pane catalog-pane" onClick={() => setMenu(null)}>
      <div className="pane-toolbar">
        <Tooltip content="Back" relationship="label">
          <button className="cat-btn" disabled={!history.length} onClick={back}>
            <ArrowLeftRegular />
          </button>
        </Tooltip>
        <Tooltip content="Up one folder" relationship="label">
          <button className="cat-btn" disabled={!listing?.parent} onClick={() => listing?.parent && go(listing.parent)}>
            <ArrowUpRegular />
          </button>
        </Tooltip>
        <input
          className="cat-path"
          value={pathText}
          onChange={(e) => setPathText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && go(pathText.trim())}
          title="Folder — type a path and press Enter"
          spellCheck={false}
        />
        <Tooltip content="Refresh" relationship="label">
          <button className="cat-btn" onClick={refresh}>
            <ArrowClockwiseRegular />
          </button>
        </Tooltip>
        <Tooltip content={connected ? "Remove this folder connection" : "Connect this folder (keep it under Places)"} relationship="label">
          <button className="cat-btn" disabled={!listing} onClick={() => listing && toggleConnection(listing.path)}>
            {connected ? <StarFilled className="cat-star" /> : <StarRegular />}
          </button>
        </Tooltip>
      </div>

      <div className="cat-places">
        {roots.map((r) => (
          <button key={r.path} className={`cat-place ${listing?.path === r.path ? "on" : ""}`} onClick={() => go(r.path)} title={r.path}>
            <RootIcon kind={r.kind} /> {r.name}
          </button>
        ))}
        {connections.map((c) => (
          <span key={c} className={`cat-place conn ${listing?.path === c ? "on" : ""}`} title={c}>
            <button onClick={() => go(c)}>
              <FolderLinkRegular /> {baseName(c)}
            </button>
            <button className="cat-x" title="Remove connection" onClick={() => toggleConnection(c)}>
              <DismissRegular />
            </button>
          </span>
        ))}
      </div>

      <div className="cat-filter">
        <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filter this folder…" />
        <label title="Hide files that aren't GIS data">
          <input type="checkbox" checked={gisOnly} onChange={(e) => useCatalog.getState().set({ gisOnly: e.target.checked })} /> GIS only
        </label>
      </div>

      {err && <div className="src-warn">{err}</div>}
      <div className="cat-list" role="listbox" aria-label="Folder contents">
        {listing && shown.length === 0 && <div className="muted pane-empty">{filter ? `Nothing matches "${filter}".` : gisOnly ? "No GIS data or folders here." : "This folder is empty."}</div>}
        {shown.map((e) => (
          <div
            key={e.path}
            role="option"
            aria-selected={selected?.path === e.path}
            className={`cat-row ${selected?.path === e.path ? "sel" : ""} ${e.kind}`}
            title={e.path}
            onClick={() => inspect(e)}
            onDoubleClick={() => open(e)}
            onContextMenu={(ev) => {
              ev.preventDefault();
              void inspect(e);
              setMenu({ x: ev.clientX, y: ev.clientY, entry: e });
            }}
            onPointerDown={(ev) => ADDABLE.has(e.kind) && startDrag(ev, { kind: "file", id: e.path, label: e.name })}
          >
            <KindIcon kind={e.kind} />
            <span className="cat-name">{e.name}</span>
            <span className="cat-meta">{sizeText(e.size)}</span>
            <span className="cat-meta date">{dateText(e.modified)}</span>
          </div>
        ))}
      </div>

      {selected && (
        <div className="cat-details">
          <div className="cat-details-head">
            <KindIcon kind={selected.kind} /> <b>{selected.name}</b>
          </div>
          {typeof details === "string" && <div className="muted">{details}</div>}
          {details && typeof details !== "string" && (
            <>
              <div className="muted">{details.driver}</div>
              {details.raster && (
                <div>
                  Raster {details.raster[0].toLocaleString()} × {details.raster[1].toLocaleString()} px · {details.raster[2]} band(s){details.crs ? ` · ${details.crs}` : ""}
                </div>
              )}
              {details.layers.map((l) => (
                <div key={l.name} className="cat-layer">
                  <b>{l.name}</b> · {l.geometry} · {l.features.toLocaleString()} features
                  <div className="muted">{l.crs ?? "No coordinate system"}</div>
                </div>
              ))}
            </>
          )}
          {ADDABLE.has(selected.kind) && <div className="muted small">Double-click or drag onto the map, Contents or the node editor to add it.</div>}
        </div>
      )}

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
              {menu.entry.kind === "folder" && <MenuItem onClick={() => go(menu.entry.path)}>Open folder</MenuItem>}
              {(menu.entry.kind === "folder" || menu.entry.kind === "gdb") && (
                <MenuItem icon={<StarRegular />} onClick={() => toggleConnection(menu.entry.path)}>
                  {connections.includes(menu.entry.path) ? "Remove folder connection" : "Connect folder"}
                </MenuItem>
              )}
              {ADDABLE.has(menu.entry.kind) && (
                <MenuItem icon={<InfoRegular />} onClick={() => inspect(menu.entry)}>
                  Details
                </MenuItem>
              )}
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
