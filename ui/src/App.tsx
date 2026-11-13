import { FluentProvider } from "@fluentui/react-components";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { DockviewReact, type DockviewReadyEvent, type IDockviewHeaderActionsProps, type IDockviewPanelProps } from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import { useEffect } from "react";
import { startProgressPolling } from "./progress/progress";
import { useSettings } from "./store/settings";
import { addData, autosaveProject, registerDock, registerMapPanelSync, saveProject, openProject } from "./actions";
import { RecoveryDialog } from "./recovery/RecoveryDialog";
import { autosaveEveryMinutes } from "./recovery/format";
import { useDialog } from "./actions";
import { EditHost } from "./edit/EditHost";
import { ScriptApprovalDialog } from "./dialogs/ScriptApprovalDialog";
import { Dialogs } from "./dialogs/Dialogs";
import { actionFor, eventCombo, useKeymap } from "./nodes/keymap";
import { TEXT_FIELD } from "./nodes/toolMenuKeys";
import { redoActive, undoActive } from "./ribbon/quickAccess";
import { LayerPropertiesWindow } from "./dialogs/LayerPropertiesWindow";
import { reportError } from "./errors/store";
import { api, IN_TAURI, type NodeResult } from "./ipc/api";
import { AttributeTable } from "./panes/AttributeTable";
import { ContentsPane } from "./panes/ContentsPane";
import { MapView } from "./panes/MapView";
import { ModelPane } from "./panes/ModelPane";
import { defaultLayout, dockIsRebuilding, restoreDock, serializeDock, setDock, syncMapPanels, type PaneId } from "./panes/panes";
import { newMap, rememberView } from "./maps/mapActions";
import { isMapPanelId } from "./store/maps";
import { CatalogPane } from "./panes/CatalogPane";
import { ItemsPane } from "./panes/ItemsPane";
import { PropertiesPane } from "./panes/PropertiesPane";
import { SourcesPane } from "./panes/SourcesPane";
import { ErrorDialog } from "./errors/ErrorDialog";
import { PaneBoundary } from "./errors/PaneBoundary";
import { LayoutView } from "./layout/LayoutView";
import { followActivePanel, layoutPanelClosed } from "./layout/actions";
import { DataPane } from "./stats/DataPane";
import { StatsView } from "./stats/StatsView";
import { statsPanelClosed } from "./stats/actions";
import { Ribbon } from "./ribbon/Ribbon";
import { StatusBar } from "./statusbar/StatusBar";
import { GeoreferenceWindow } from "./georef/GeoreferenceWindow";
import { AdjustWindow } from "./georef/AdjustWindow";
import { BookmarkNameDialog } from "./map/BookmarkMenu";
import { useModel, useRun } from "./store/model";
import { useApp } from "./store/store";
import { useGoTo } from "./goto/gotoStore";
import { basemapsInUse } from "./store/maps";
import { applyThemeMode, dockTheme, fluentDark, fluentLight } from "./theme/fluentTheme";

const guard = (pane: string, title: string, el: React.ReactNode) => (
  <PaneBoundary pane={pane} title={title}>
    {el}
  </PaneBoundary>
);

const components: Record<PaneId | "layout" | "stats", React.FunctionComponent<IDockviewPanelProps>> = {
  layout: (p) => guard(`layout:${(p.params as { layoutId: string }).layoutId}`, "Layout", <LayoutView layoutId={(p.params as { layoutId: string }).layoutId} />),
  contents: () => guard("contents", "Map Layers", <ContentsPane />),
  catalog: () => guard("catalog", "Catalog", <CatalogPane />),
  map: (p) => guard(`map:${p.api.id}`, "Map", <MapView mapId={p.api.id} />),
  table: () => guard("table", "Attribute Table", <AttributeTable />),
  model: () => guard("model", "Processing nodes", <ModelPane />),
  properties: () => guard("properties", "Properties", <PropertiesPane />),
  sources: () => guard("sources", "Sources", <SourcesPane />),
  items: () => guard("items", "Items", <ItemsPane />),
  statdata: () => guard("statdata", "Statistics data", <DataPane />),
  stats: (p) => guard(`stats:${(p.params as { statsId: string }).statsId}`, "Statistics sheet", <StatsView sheetId={(p.params as { statsId: string }).statsId} />),
};

/** The + at the end of a tab strip that holds a map: a new map, opened in that strip. */
function NewMapButton({ group }: IDockviewHeaderActionsProps) {
  if (!group.panels.some((p) => isMapPanelId(p.id))) return null;
  return (
    <button type="button" className="map-tab-add" title="New map" aria-label="New map" onClick={() => newMap({ group })}>
      +
    </button>
  );
}

export function App() {
  const dark = useApp((s) => s.dark);
  useEffect(() => applyThemeMode(dark ? "dark" : "light"), [dark]);

  useEffect(startProgressPolling, []);
  // The Start page opens when the desktop app starts (not in a browser dev session), unless turned off or there is work to recover first.
  useEffect(() => {
    if (!IN_TAURI) return;
    void (async () => {
      try {
        const s = await api.getSettings();
        if (s?.showStartPage === false) return;
        if ((await api.listRecovery()).length) return;
        if (!useDialog.getState().dialog) useDialog.getState().show({ kind: "start" });
      } catch {
        /* no start page; the app works without it */
      }
    })();
  }, []);
  // Autosave (UX1): a separate recovery copy of unsaved work on a timer; the project itself is never written and stays marked unsaved.
  const autosaveSetting = useSettings((s) => s.settings.autosaveMinutes);
  const autosaveMinutes = autosaveEveryMinutes(autosaveSetting);
  useEffect(() => {
    if (!autosaveMinutes) return;
    const t = setInterval(() => void autosaveProject(), autosaveMinutes * 60_000);
    return () => clearInterval(t);
  }, [autosaveMinutes]);
  // Program settings (UR-16): read once at start and applied. (The ribbon's Dark Theme keeps using the app's own remembered choice; the theme in Settings is saved to the file.)
  useEffect(() => {
    void useSettings.getState().load();
  }, []);
  // PROJ without its datum files moves NAD27 / NAD83 / WGS 84 coordinates by metres, silently: say so at start-up (R3).
  useEffect(() => {
    api.datumDataStatus().then((r) => {
      if (!r.ok) reportError(r.problems.join("\n"), "datum-data", { title: "Datum shifts can't be trusted on this computer", context: "checking the coordinate system data" });
    }, () => {});
  }, []);
  // Every map's basemap is cited, not only the active map's (the engine keeps earlier citations).
  const basemaps = useApp((s) => basemapsInUse(s.maps).join("|"));
  useEffect(() => {
    for (const b of basemaps.split("|")) if (b) api.recordBasemap(b).catch(() => {});
  }, [basemaps]);
  useEffect(() => {
    api.toolSpecs().then(useRun.getState().setSpecs).catch((e) => useApp.getState().setStatus(`Engine not available: ${e}`));
    // Live step status while a model runs; drag and drop files onto the window to add them.
    const un1 = IN_TAURI ? listen<NodeResult>("model-status", (e) => useRun.getState().setResult(e.payload)) : Promise.resolve(() => {});
    const un2 = IN_TAURI
      ? getCurrentWebview().onDragDropEvent((e) => {
          if (e.payload.type === "drop" && e.payload.paths.length) addData(e.payload.paths);
        })
      : Promise.resolve(() => {});
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key === "s") {
        e.preventDefault();
        saveProject(e.shiftKey);
      } else if (mod && e.key === "o") {
        e.preventDefault();
        openProject();
        // The other node-editor keys (run, …) live in nodes/keymap.ts and apply only there; undo and redo work everywhere, below.
      } else if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "g") {
        // Go To Coordinates (a browser would use Ctrl+G for "find next"; nothing else in the app binds it).
        e.preventDefault();
        useGoTo.getState().setOpen(true);
      } else if (e.altKey && e.key.toLowerCase() === "q") {
        e.preventDefault();
        (document.querySelector(".tellme input") as HTMLInputElement | null)?.focus();
      } else if (!e.defaultPrevented) {
        // Undo and redo from anywhere, like the buttons. The node editor, the layout page and the table handle their own first (defaultPrevented);
        // a text box keeps its own undo, and a dialog or the tool menu takes no shortcut.
        const combo = eventCombo(e);
        const action = combo && actionFor(useKeymap.getState().bindings, combo);
        if (action !== "undo" && action !== "redo") return;
        if ((document.activeElement as HTMLElement | null)?.closest?.(TEXT_FIELD) || document.querySelector(".toolmenu, [role=dialog]")) return;
        e.preventDefault();
        if (action === "undo") undoActive();
        else redoActive();
      }
    };
    window.addEventListener("keydown", onKey);
    // The Node Editor shows the node tree of the layer selected in Map Layers.
    useModel.getState().openTree(useApp.getState().activeLayer);
    const unTree = useApp.subscribe((s, prev) => {
      if (s.activeLayer !== prev.activeLayer) useModel.getState().openTree(s.activeLayer);
    });
    const beforeUnload = (e: BeforeUnloadEvent) => {
      if (useApp.getState().dirty) e.preventDefault();
    };
    window.addEventListener("beforeunload", beforeUnload);
    // Leaving with unsaved work keeps the recovery copy; note that the window was closed so the next start can say so (best effort).
    const pageHide = () => {
      if (useApp.getState().dirty) api.markRecoveryClosed().catch(() => {});
    };
    window.addEventListener("pagehide", pageHide);
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      window.removeEventListener("keydown", onKey);
      unTree();
      window.removeEventListener("beforeunload", beforeUnload);
      window.removeEventListener("pagehide", pageHide);
    };
  }, []);

  const onReady = (e: DockviewReadyEvent) => {
    setDock(e.api);
    registerDock(serializeDock, restoreDock);
    registerMapPanelSync(syncMapPanels);
    defaultLayout();
    // The Layout ribbon tab and Properties follow the active layout tab.
    e.api.onDidActivePanelChange((ev) => {
      followActivePanel(ev.panel?.params as { layoutId?: string; statsId?: string } | undefined);
      // Focusing a map's tab makes it the active map (Map Layers, the status bar and the tools follow it).
      if (ev.panel && isMapPanelId(ev.panel.id)) useApp.getState().setActiveMap(ev.panel.id);
    });
    e.api.onDidRemovePanel((panel) => {
      layoutPanelClosed((panel.params as { layoutId?: string } | undefined)?.layoutId);
      statsPanelClosed((panel.params as { statsId?: string } | undefined)?.statsId);
      if (isMapPanelId(panel.id) && !dockIsRebuilding()) rememberView(panel.id);
    });
    // Renaming, duplicating or deleting a map updates its tab; a deleted map's tab closes.
    useApp.subscribe((s, prev) => {
      if (s.maps !== prev.maps) syncMapPanels();
    });
  };

  return (
    // The layout class goes on our own div, not the FluentProvider: Fluent
    // copies the provider's className onto its popup containers, and a
    // full-height white "app" container would cover the whole window.
    <FluentProvider theme={dark ? fluentDark : fluentLight} style={{ height: "100%" }}>
      <div className={`app ${dark ? "dark" : ""}`}>
        <Ribbon />
        <div className="dock">
          <DockviewReact components={components} onReady={onReady} theme={dockTheme} defaultRenderer="always" rightHeaderActionsComponent={NewMapButton} />
        </div>
        <StatusBar />
        <LayerPropertiesWindow />
        <GeoreferenceWindow />
        <AdjustWindow />
        <BookmarkNameDialog />
        <Dialogs />
        <ScriptApprovalDialog />
        <RecoveryDialog />
        <EditHost />
        <ErrorDialog />
      </div>
    </FluentProvider>
  );
}
