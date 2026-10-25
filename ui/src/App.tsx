import { FluentProvider, webDarkTheme, webLightTheme, type Theme } from "@fluentui/react-components";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { DockviewReact, themeLight, themeDark, type DockviewReadyEvent, type IDockviewPanelProps } from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import { useEffect } from "react";
import { startProgressPolling } from "./progress/progress";
import { addData, registerDock, saveProject, openProject } from "./actions";
import { Dialogs } from "./dialogs/Dialogs";
import { LayerPropertiesWindow } from "./dialogs/LayerPropertiesWindow";
import { api, IN_TAURI, type NodeResult } from "./ipc/api";
import { AttributeTable } from "./panes/AttributeTable";
import { ContentsPane } from "./panes/ContentsPane";
import { MapView } from "./panes/MapView";
import { ModelPane } from "./panes/ModelPane";
import { defaultLayout, setDock, type PaneId } from "./panes/panes";
import { CatalogPane } from "./panes/CatalogPane";
import { PropertiesPane } from "./panes/PropertiesPane";
import { SourcesPane } from "./panes/SourcesPane";
import { ErrorDialog } from "./errors/ErrorDialog";
import { PaneBoundary } from "./errors/PaneBoundary";
import { LayoutView } from "./layout/LayoutView";
import { useLayouts } from "./store/layout";
import { ToolsPane } from "./panes/ToolsPane";
import { Ribbon } from "./ribbon/Ribbon";
import { StatusBar } from "./statusbar/StatusBar";
import { useModel, useRun } from "./store/model";
import { useApp } from "./store/store";

// Office-like: Segoe UI, square-ish corners, compact controls.
const officeLight: Theme = {
  ...webLightTheme,
  fontFamilyBase: '"Segoe UI", system-ui, -apple-system, "Helvetica Neue", sans-serif',
  borderRadiusMedium: "2px",
  borderRadiusSmall: "2px",
  borderRadiusLarge: "3px",
  colorBrandBackground: "#2b579a",
  colorBrandBackgroundHover: "#1e4380",
  colorBrandBackgroundPressed: "#173563",
  colorCompoundBrandStroke: "#2b579a",
  colorCompoundBrandBackground: "#2b579a",
  colorBrandForeground1: "#2b579a",
};
const officeDark: Theme = { ...webDarkTheme, fontFamilyBase: officeLight.fontFamilyBase, borderRadiusMedium: "2px", borderRadiusSmall: "2px" };

const guard = (pane: string, title: string, el: React.ReactNode) => (
  <PaneBoundary pane={pane} title={title}>
    {el}
  </PaneBoundary>
);

const components: Record<PaneId | "layout", React.FunctionComponent<IDockviewPanelProps>> = {
  layout: (p) => guard(`layout:${(p.params as { layoutId: string }).layoutId}`, "Layout", <LayoutView layoutId={(p.params as { layoutId: string }).layoutId} />),
  contents: () => guard("contents", "Contents", <ContentsPane />),
  catalog: () => guard("catalog", "Catalog", <CatalogPane />),
  map: () => guard("map", "Map", <MapView />),
  table: () => guard("table", "Attribute Table", <AttributeTable />),
  model: () => guard("model", "Model", <ModelPane />),
  properties: () => guard("properties", "Properties", <PropertiesPane />),
  tools: () => guard("tools", "Geoprocessing", <ToolsPane />),
  sources: () => guard("sources", "Sources", <SourcesPane />),
};

export function App() {
  const dark = useApp((s) => s.dark);

  useEffect(startProgressPolling, []);
  const basemap = useApp((s) => s.basemap);
  useEffect(() => {
    if (basemap) api.recordBasemap(basemap).catch(() => {});
  }, [basemap]);
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
        // Node-editor keys (undo, run, …) live in nodes/keymap.ts and apply only there.
      } else if (e.altKey && e.key.toLowerCase() === "q") {
        e.preventDefault();
        (document.querySelector(".tellme input") as HTMLInputElement | null)?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    // The Model shows the node tree of the layer selected in Contents.
    useModel.getState().openTree(useApp.getState().activeLayer);
    const unTree = useApp.subscribe((s, prev) => {
      if (s.activeLayer !== prev.activeLayer) useModel.getState().openTree(s.activeLayer);
    });
    const beforeUnload = (e: BeforeUnloadEvent) => {
      if (useApp.getState().dirty) e.preventDefault();
    };
    window.addEventListener("beforeunload", beforeUnload);
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      window.removeEventListener("keydown", onKey);
      unTree();
      window.removeEventListener("beforeunload", beforeUnload);
    };
  }, []);

  const onReady = (e: DockviewReadyEvent) => {
    setDock(e.api);
    registerDock(
      () => e.api.toJSON(),
      (d) => {
        try {
          e.api.fromJSON(d as never);
        } catch {
          defaultLayout();
        }
      },
    );
    defaultLayout();
    // The Layout ribbon tab and Properties follow the active layout tab.
    e.api.onDidActivePanelChange((ev) => {
      const lid = (ev.panel?.params as { layoutId?: string } | undefined)?.layoutId;
      useLayouts.getState().open(lid ?? null);
    });
  };

  return (
    // The layout class goes on our own div, not the FluentProvider: Fluent
    // copies the provider's className onto its popup containers, and a
    // full-height white "app" container would cover the whole window.
    <FluentProvider theme={dark ? officeDark : officeLight} style={{ height: "100%" }}>
      <div className={`app ${dark ? "dark" : ""}`}>
        <Ribbon />
        <div className="dock">
          <DockviewReact components={components} onReady={onReady} theme={dark ? themeDark : themeLight} defaultRenderer="always" />
        </div>
        <StatusBar />
        <LayerPropertiesWindow />
        <Dialogs />
        <ErrorDialog />
      </div>
    </FluentProvider>
  );
}
