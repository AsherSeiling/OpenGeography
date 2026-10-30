// Office / ArcGIS Pro style ribbon: tabs → labeled groups → large and small
// buttons. Contextual tabs appear when a layer is selected.
import { Combobox, Menu, MenuDivider, MenuItem, MenuItemRadio, MenuList, MenuPopover, MenuTrigger, Option, Tooltip } from "@fluentui/react-components";
import { AlignLeftRegular, ArrowRotateClockwiseRegular, ColorRegular, DeleteRegular, DocumentOnePageMultipleRegular, DocumentPdfRegular, DocumentRegular, DocumentSaveRegular, DocumentSettingsRegular, LayerRegular, PageFitRegular, PaintBrushRegular, TextAlignLeftRegular, ImageMultipleRegular, SearchRegular } from "@fluentui/react-icons";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { api, errorText, type LayoutTemplate } from "../ipc/api";
import { pageCaption, useSeriesView } from "../layout/series";
import { useDialog } from "../actions";
import { applyTheme, exportLayoutSeries, deleteLayoutTemplate, newLayout, newLayoutFromTemplate, saveLayoutAsTemplate } from "../layout/actions";
import { describePage, PAGE_SIZES, pageFor, type Orientation } from "../layout/types";
import { useLayouts } from "../store/layout";
import type { AlignOp, OrderOp } from "../layout/arrange";
import { DPI_MAX, DPI_MIN, parseDpi, useExportDpi } from "../layout/exportDpi";
import { useRun } from "../store/model";
import { useNav } from "../nav/navStore";
import { useApp } from "../store/store";
import { BASEMAP_OPTIONS, commands, TOOL_BUTTONS, toolCommand, type Command } from "./commands";
import { fitRibbon, type Variant } from "./ribbonFit";
import { useRibbonTabs, type Group, type Size } from "./tabs";


const TABS: Record<string, Group[]> = {
  Map: [
    { label: "Navigate", items: [{ cmd: "explore" }, { cmd: "fullExtent" }] },
    { label: "Layer", items: [{ cmd: "addData" }, { cmd: "catalog" }, { cmd: "tiger" }, { cmd: "nationalMap" }, { cmd: "elevation" }, { custom: <BasemapGallery /> }] },
    {
      label: "Selection",
      items: [{ cmd: "select" }, { cmd: "selectBox", size: "small" }, { cmd: "selectByAttributes", size: "small" }, { cmd: "selectByLocation", size: "small" }, { cmd: "zoomSelection", size: "small" }, { cmd: "clearSelection" }],
    },
    { label: "Inquiry", items: [{ cmd: "identify" }, { cmd: "measure" }, { cmd: "measureArea", size: "small" }] },
    { label: "Land Nav", items: [{ cmd: "resection" }, { cmd: "intersection" }, { cmd: "usngGrid" }] },
    { label: "Coordinate System", items: [{ cmd: "mapCrs" }, { cmd: "datumGrids" }, { cmd: "projectionBuilder" }, { cmd: "tissot" }, { cmd: "responsibleUse" }] },
    { label: "World", items: [{ cmd: "worldView" }, { cmd: "worldGraticule" }, { cmd: "worldOutline" }] },
  ],
  Insert: [
    { label: "Layouts", items: [{ custom: <NewLayoutGallery /> }] },
    { label: "Maps", items: [{ cmd: "topoMap" }] },
  ],
  Analysis: [
    { label: "Geoprocessing", items: [{ cmd: "modelBuilder" }, { cmd: "runModel" }, { cmd: "tools" }] },
    { label: "Tools", items: [{ custom: <ToolGallery /> }] },
  ],
  Data: [
    { label: "Add", items: [{ cmd: "addData" }, { cmd: "catalog" }, { cmd: "tiger" }, { cmd: "nationalMap" }, { cmd: "arcgis" }, { cmd: "stac" }, { cmd: "hazardsSoils" }, { cmd: "ogc" }, { cmd: "overture" }, { cmd: "elevation" }] },
    { label: "Fields", items: [{ cmd: "addField" }, { cmd: "calcField" }, { cmd: "calcGeometry" }, { cmd: "census" }] },
    { label: "Coordinate Systems", items: [{ cmd: "defineProjection" }, { cmd: "mapCrs" }] },
    { label: "Export", items: [{ cmd: "exportLayer" }, { cmd: "exportTable" }, { cmd: "layerFromSelection" }, { cmd: "splitByAttribute" }] },
    { label: "Filter", items: [{ cmd: "definitionQuery" }] },
    { label: "Sources", items: [{ cmd: "sources" }] },
    { label: "Testing", items: [{ cmd: "testData" }] },
  ],
  View: [
    {
      label: "Windows",
      items: [
        { cmd: "paneContents", size: "small" },
        { cmd: "paneTable", size: "small" },
        { cmd: "paneModel", size: "small" },
        { cmd: "paneProperties", size: "small" },
        { cmd: "paneTools", size: "small" },
        { cmd: "paneSources", size: "small" },
        { cmd: "paneCatalog", size: "small" },
        { cmd: "paneMap", size: "small" },
        { cmd: "resetPanes" },
      ],
    },
    { label: "Performance", items: [{ cmd: "fps" }, { cmd: "benchmark" }] },
    { label: "Appearance", items: [{ cmd: "theme" }] },
  ],
  Layout: [
    { label: "Page", items: [{ custom: <PageMenu /> }] },
    { label: "Template", items: [{ custom: <TemplateGroup /> }] },
    { label: "Theme", items: [{ custom: <ThemeMenu /> }] },
    { label: "Map Series", items: [{ custom: <SeriesGroup /> }] },
  ],
  Elements: [
    { label: "Insert", items: [{ cmd: "layoutMapFrame" }, { cmd: "layoutOverview" }, { cmd: "layoutText" }, { cmd: "layoutTable" }, { cmd: "layoutChart" }, { cmd: "layoutLegend" }, { cmd: "layoutScaleBar" }, { cmd: "layoutNorthArrow" }, { cmd: "layoutDeclination" }, { cmd: "layoutSources" }] },
    { label: "Graphics", items: [{ cmd: "layoutRect" }, { cmd: "layoutEllipse" }, { cmd: "layoutTriangle" }, { cmd: "layoutLine" }, { cmd: "layoutArrow" }, { cmd: "layoutPicture" }] },
  ],
  Format: [
    { label: "Map Frame", items: [{ cmd: "layoutFromMap" }, { cmd: "layoutFrameCrs" }, { custom: <ScaleBox /> }] },
    { label: "Arrange", items: [{ custom: <ArrangeMenu /> }] },
  ],
  Export: [
    { label: "Export", items: [{ cmd: "layoutPdf" }, { cmd: "layoutPng" }, { cmd: "layoutSvg" }, { custom: <DpiBox /> }] },
  ],
  "Feature Layer": [
    { label: "Layer", items: [{ cmd: "layerProps" }, { cmd: "zoomLayer" }, { cmd: "symbology" }, { cmd: "openTable" }] },
    { label: "Fields", items: [{ cmd: "addField" }, { cmd: "calcField" }, { cmd: "calcGeometry" }] },
    { label: "Data", items: [{ cmd: "definitionQuery" }, { cmd: "defineProjection" }, { cmd: "exportLayer" }, { cmd: "exportTable" }, { cmd: "layerFromSelection" }, { cmd: "splitByAttribute" }] },
    { label: "Remove", items: [{ cmd: "removeLayer" }] },
  ],
};

/** The layout workspace's contextual tabs (LW1), all under one "Layout" band. */
const LAYOUT_TABS = ["Layout", "Elements", "Format", "Export"];
const MAIN_TABS = ["Map", "Insert", "Analysis", "Data", "View"];
const KEY_TIPS: Record<string, string> = { Map: "M", Insert: "I", Analysis: "A", Data: "D", View: "V" };

export function Ribbon() {
  const [tab, setTab] = useState("Map");
  // Re-render when state that affects enabled/checked changes.
  useApp((s) => s.tool);
  useApp((s) => s.selection);
  useApp((s) => s.showFps);
  useApp((s) => s.dark);
  useNav((s) => s.navPanel);
  useNav((s) => s.measureMode);
  const active = useApp((s) => (s.activeLayer ? s.layers[s.activeLayer] : null));
  const contextual = active && active.geomType !== "Table";
  const layoutId = useLayouts((s) => s.id);
  useLayouts((s) => s.doc);
  useLayouts((s) => s.selected);
  const registered = useRibbonTabs((s) => s.tabs);
  // Opening a layout tab brings its contextual ribbon tab forward, as in ArcGIS Pro.
  useEffect(() => {
    if (layoutId) setTab("Layout");
  }, [layoutId]);
  const extra = Object.values(registered).filter((t) => t.visible);
  const current = (tab === "Feature Layer" && !contextual) || (LAYOUT_TABS.includes(tab) && !layoutId) || (!(tab in TABS) && !extra.some((t) => t.id === tab)) ? "Map" : tab;
  const groups: Group[] = TABS[current] ?? extra.find((t) => t.id === current)?.groups ?? [];

  // Collapse to the tab row (double-click a tab or Ctrl+F1); a click on a tab while collapsed shows the body until you click elsewhere.
  const [collapsed, setCollapsed] = useState(false);
  const [peek, setPeek] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "F1" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        setCollapsed((c) => !c);
        setPeek(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  useEffect(() => {
    if (!peek) return;
    const off = (e: MouseEvent) => {
      if (!(e.target as Element)?.closest?.(".ribbon")) setPeek(false);
    };
    window.addEventListener("mousedown", off);
    return () => window.removeEventListener("mousedown", off);
  }, [peek]);

  // Key tips: Alt pressed and released on its own (no mouse action in between) shows a letter on each tab; the next plain letter opens that
  // tab. They go away on any mouse action, on any other key, and after 4 seconds, so they never swallow a shortcut meant for another pane.
  const [tips, setTips] = useState(false);
  useEffect(() => {
    let alone = false;
    let timer = 0;
    const close = () => {
      window.clearTimeout(timer);
      setTips(false);
    };
    const down = (e: KeyboardEvent) => {
      if (e.key === "Alt") {
        alone = true;
        return;
      }
      alone = false;
      if (!tipsOpen.current) return;
      const plain = !e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey;
      const t = plain ? Object.entries(KEY_TIPS).find(([, k]) => k.toLowerCase() === e.key.toLowerCase()) : undefined;
      close();
      if (t) {
        e.preventDefault();
        e.stopPropagation();
        setTab(t[0]);
      }
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === "Alt" && alone) {
        window.clearTimeout(timer);
        setTips((v) => {
          if (!v) timer = window.setTimeout(() => setTips(false), 4000);
          return !v;
        });
      }
      alone = false;
    };
    const mouse = () => {
      alone = false;
      if (tipsOpen.current) close();
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    window.addEventListener("pointerdown", mouse, true);
    window.addEventListener("wheel", mouse, { capture: true, passive: true });
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
      window.removeEventListener("pointerdown", mouse, true);
      window.removeEventListener("wheel", mouse, true);
    };
  }, []);
  const tipsOpen = useRef(false);
  tipsOpen.current = tips;
  const pickTab = (t: string) => {
    setTab(t);
    if (collapsed) setPeek(true);
  };

  // Fit the groups of the current tab into the width: measure each group in each size variant once per tab, then choose variants.
  const body = useRef<HTMLDivElement>(null);
  const moreBtn = useRef<HTMLDivElement>(null);
  const [forced, setForced] = useState<Variant | null>(null);
  const [fit, setFit] = useState<{ variants: Variant[]; shown: number } | null>(null);
  const widths = useRef<Record<string, number[][]>>({});
  const layoutKey = `${current}|${groups.length}`;
  const measure = useCallback(() => {
    const el = body.current;
    if (!el) return;
    if (!widths.current[layoutKey]) {
      const w: number[][] = groups.map(() => [0, 0, 0, 0]);
      for (const v of [0, 1, 2, 3] as Variant[]) {
        flushSync(() => setForced(v));
        el.querySelectorAll<HTMLElement>(":scope > .rg-slot").forEach((slot, gi) => {
          if (w[gi]) w[gi][v] = slot.getBoundingClientRect().width;
        });
      }
      flushSync(() => setForced(null));
      widths.current[layoutKey] = w;
    }
    const cs = getComputedStyle(el);
    const avail = el.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
    const more = moreBtn.current?.getBoundingClientRect().width || 76;
    const result = fitRibbon({ widths: widths.current[layoutKey], avail, moreWidth: more });
    setFit(result);
  }, [layoutKey, groups]);
  useEffect(() => {
    const el = body.current;
    if (!el) return;
    // A short timer, not requestAnimationFrame: a hidden or background window never runs animation frames.
    let timer = 0;
    const run = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(measure, 16);
    };
    run();
    const ro = new ResizeObserver(run);
    ro.observe(el);
    // Fonts change group widths: measure again once they are loaded.
    (document as Document & { fonts?: { ready: Promise<unknown> } }).fonts?.ready.then(() => { delete widths.current[layoutKey]; run(); });
    return () => {
      window.clearTimeout(timer);
      ro.disconnect();
    };
  }, [measure, layoutKey, collapsed, peek]);
  // A layout or template change can change a custom control's width: measure that tab again.
  const doc = useLayouts((s) => s.doc);
  useLayoutEffect(() => {
    delete widths.current[layoutKey];
  }, [doc?.page?.width, doc?.page?.height, layoutKey]);

  const variants = (i: number): Variant => forced ?? fit?.variants[i] ?? 0;
  const shown = forced !== null ? groups.length : fit?.shown ?? groups.length;
  const hidden = groups.slice(shown);

  return (
    <div className={`ribbon${collapsed ? " collapsed" : ""}${collapsed && peek ? " peek" : ""}`}>
      <div className="ribbon-tabs">
        <Menu>
          <MenuTrigger>
            <button className="rtab file">Project</button>
          </MenuTrigger>
          <MenuPopover>
            <MenuList>
              {["new", "open", "save", "saveAs", "sources"].map((id) => (
                <MenuItem key={id} icon={commands[id].icon} secondaryContent={commands[id].keys} onClick={() => commands[id].run()}>
                  {commands[id].label}
                </MenuItem>
              ))}
            </MenuList>
          </MenuPopover>
        </Menu>
        <div className="qat" role="toolbar" aria-label="Quick access">
          {["open", "save"].map((id) => (
            <Tooltip key={id} content={`${commands[id].label}${commands[id].keys ? ` (${commands[id].keys})` : ""}`} relationship="label" withArrow>
              <button className="qat-btn" onClick={() => commands[id].run()}>{commands[id].icon}</button>
            </Tooltip>
          ))}
        </div>
        {MAIN_TABS.map((t) => (
          <button key={t} className={`rtab ${current === t ? "on" : ""}`} onClick={() => pickTab(t)} onDoubleClick={() => { setCollapsed((c) => !c); setPeek(false); }} title="Double-click to collapse or expand the ribbon (Ctrl+F1)">
            {t}
            {tips && <span className="keytip">{KEY_TIPS[t]}</span>}
          </button>
        ))}
        {layoutId && (
          <div className="rtab-context">
            <span className="rtab-context-label">Layout</span>
            <div className="rtab-context-row">
              {LAYOUT_TABS.map((t) => (
                <button key={t} className={`rtab ctx ${current === t ? "on" : ""}`} onClick={() => pickTab(t)}>
                  {t}
                </button>
              ))}
            </div>
          </div>
        )}
        {contextual && (
          <div className="rtab-context">
            <span className="rtab-context-label">Layer</span>
            <button className={`rtab ctx ${current === "Feature Layer" ? "on" : ""}`} onClick={() => pickTab("Feature Layer")}>
              Feature Layer
            </button>
          </div>
        )}
        {extra.map((t) => (
          <div key={t.id} className="rtab-context">
            <span className="rtab-context-label">{t.band}</span>
            <button className={`rtab ctx ${current === t.id ? "on" : ""}`} onClick={() => pickTab(t.id)}>
              {t.label}
            </button>
          </div>
        ))}
        <div className="ribbon-spacer" />
        <TellMe />
      </div>
      <div className="ribbon-body" ref={body}>
        {groups.slice(0, shown).map((g, i) => (
          <div key={g.label} className="rg-slot" data-group={g.label}>
            <RibbonGroup group={g} variant={variants(i)} />
          </div>
        ))}
        {hidden.length > 0 && forced === null && (
          <div className="rg-slot rg-more" ref={moreBtn}>
            <Menu>
              <MenuTrigger>
                <button className="rbtn large rbtn-more" title="Commands that do not fit at this window width">
                  <span className="rbtn-icon">⋯</span>
                  <span className="rbtn-label">More ▾</span>
                </button>
              </MenuTrigger>
              <MenuPopover>
                <div className="rmenu">
                  {hidden.map((g) => (
                    <GroupMenuBody key={g.label} group={g} heading />
                  ))}
                </div>
              </MenuPopover>
            </Menu>
          </div>
        )}
        {hidden.length === 0 && <div className="rg-slot rg-more rg-measure" ref={moreBtn} aria-hidden><span className="rbtn large rbtn-more"><span className="rbtn-icon">⋯</span><span className="rbtn-label">More ▾</span></span></div>}
      </div>
    </div>
  );
}

/**
 * One group in the given size variant (see ribbonFit.ts). The group's own controls stay mounted in every variant (hidden when the group
 * is collapsed into a menu), so measuring the variants never rebuilds a control or closes a menu the user has just opened.
 */
function RibbonGroup({ group, variant = 0 }: { group: Group; variant?: Variant }) {
  const collapsed = variant === 3;
  // Consecutive small buttons stack in columns of three, as in Office.
  const cols: ReactNode[] = [];
  let smalls: string[] = [];
  const small = (id: string) => <RButton key={id} cmd={commands[id]} size="small" iconOnly={variant === 2} />;
  const flush = () => {
    if (smalls.length) cols.push(<div key={`s${cols.length}`} className="rcol">{smalls.map(small)}</div>);
    smalls = [];
  };
  group.items.forEach((it, i) => {
    if ("custom" in it) {
      flush();
      cols.push(<div key={`c${i}`}>{it.custom}</div>);
    } else if (it.size === "small" || variant === 1 || variant === 2) {
      smalls.push(it.cmd);
      if (smalls.length === 3) flush();
    } else {
      flush();
      cols.push(<RButton key={it.cmd} cmd={commands[it.cmd]} size="large" />);
    }
  });
  flush();
  return (
    <div className={`rgroup${collapsed ? " rgroup-collapsed" : ""}`}>
      <div className="rgroup-items">
        {collapsed && (
          <Menu>
            <MenuTrigger>
              <button className="rbtn large" title={group.label}>
                <span className="rbtn-icon">{firstIcon(group)}</span>
                <span className="rbtn-label">{group.label} ▾</span>
              </button>
            </MenuTrigger>
            <MenuPopover>
              <div className="rmenu">
                <GroupMenuBody group={group} />
              </div>
            </MenuPopover>
          </Menu>
        )}
        <div className={collapsed ? "rg-hidden" : "rg-cols"}>{cols}</div>
      </div>
      <div className="rgroup-label">{group.label}</div>
    </div>
  );
}

function firstIcon(g: Group): ReactNode {
  for (const it of g.items) if ("cmd" in it && commands[it.cmd]) return commands[it.cmd].icon;
  return "▦";
}

/** A group's commands as menu rows (for a collapsed group and for the More menu); custom controls are shown as they are. */
function GroupMenuBody({ group, heading }: { group: Group; heading?: boolean }) {
  return (
    <div className="rmenu-group">
      {heading && <div className="rmenu-heading">{group.label}</div>}
      {group.items.map((it, i) =>
        "custom" in it ? (
          <div key={`c${i}`} className="rmenu-custom">{it.custom}</div>
        ) : commands[it.cmd] ? (
          <button
            key={it.cmd}
            className="rmenu-item"
            disabled={commands[it.cmd].enabled ? !commands[it.cmd].enabled!() : false}
            onClick={() => commands[it.cmd].run()}
          >
            <span className="rbtn-icon">{commands[it.cmd].icon}</span>
            <span>{commands[it.cmd].label}</span>
          </button>
        ) : null,
      )}
    </div>
  );
}

export function RButton({ cmd, size, iconOnly }: { cmd: Command; size: Size; iconOnly?: boolean }) {
  const disabled = cmd.enabled ? !cmd.enabled() : false;
  const checked = cmd.checked?.() ?? false;
  const btn = (
    <button className={`rbtn ${size}${iconOnly ? " iconly" : ""} ${checked ? "checked" : ""}`} disabled={disabled} onClick={() => cmd.run()} aria-pressed={cmd.checked ? checked : undefined} aria-label={iconOnly ? cmd.label : undefined}>
      <span className="rbtn-icon">{cmd.icon}</span>
      <span className="rbtn-label">{cmd.label}</span>
    </button>
  );
  const tip = [iconOnly ? cmd.label : null, cmd.tip, cmd.keys && `(${cmd.keys})`].filter(Boolean).join(" · ");
  return tip ? (
    <Tooltip content={tip} relationship="description" withArrow>
      {btn}
    </Tooltip>
  ) : (
    btn
  );
}

/** Insert → Layouts: pick a page size and orientation for a new layout. */
function NewLayoutGallery() {
  return (
    <Menu>
      <MenuTrigger>
        <button className="rbtn large" title="Create a print layout">
          <span className="rbtn-icon"><DocumentPdfRegular /></span>
          <span className="rbtn-label">New Layout ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {Object.keys(PAGE_SIZES).flatMap((size) =>
            (["landscape", "portrait"] as Orientation[]).map((o) => (
              <MenuItem key={`${size}${o}`} icon={<DocumentRegular />} onClick={() => newLayout(size, o)}>
                {size} {o}
              </MenuItem>
            )),
          )}
          <MenuDivider />
          <TemplateSubmenu />
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** New Layout → From template: bundled page templates, then the user's own (C3.1). */
function TemplateSubmenu() {
  const [list, setList] = useState<{ templates: LayoutTemplate[]; skipped: string[] } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const load = () => api.listLayoutTemplates().then((l) => { setList(l); setErr(null); }, (e) => setErr(errorText(e)));
  const mine = list?.templates.filter((t) => !t.builtin) ?? [];
  return (
    <Menu onOpenChange={(_, d) => d.open && void load()}>
      <MenuTrigger disableButtonEnhancement>
        <MenuItem icon={<DocumentRegular />}>From template</MenuItem>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {err && <MenuItem disabled>{err}</MenuItem>}
          {!list && !err && <MenuItem disabled>Loading…</MenuItem>}
          {list?.templates.filter((t) => t.builtin).map((t) => (
            <MenuItem key={t.id} icon={<DocumentRegular />} onClick={() => void newLayoutFromTemplate(t.id)}>{t.name}</MenuItem>
          ))}
          {mine.length > 0 && <MenuDivider />}
          {mine.map((t) => (
            <MenuItem key={t.id} icon={<DocumentRegular />} onClick={() => void newLayoutFromTemplate(t.id)}>{t.name}</MenuItem>
          ))}
          {list && list.skipped.length > 0 && <MenuItem disabled>{list.skipped.length} template file{list.skipped.length === 1 ? "" : "s"} could not be read</MenuItem>}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** Layout → Theme: a coordinated look for the page (and optionally the layers' colours). */
function ThemeMenu() {
  const open = useLayouts((s) => !!s.doc);
  const [themes, setThemes] = useState<{ id: string; name: string; description: string }[]>([]);
  return (
    <div className="rcol">
      <Menu onOpenChange={(_, d) => d.open && void api.layoutThemes().then(setThemes, () => setThemes([]))}>
        <MenuTrigger>
          <button className="rbtn small" disabled={!open} title="Topographic, thematic or minimalist: page colours, frame, legend box, scale bar and north arrow style">
            <span className="rbtn-icon"><PaintBrushRegular /></span>
            <span className="rbtn-label">Page look ▾</span>
          </button>
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            {themes.map((t) => (
              <MenuItem key={t.id} title={t.description} onClick={() => void applyTheme(t.id, false)}>{t.name}</MenuItem>
            ))}
          </MenuList>
        </MenuPopover>
      </Menu>
      <Menu onOpenChange={(_, d) => d.open && void api.layoutThemes().then(setThemes, () => setThemes([]))}>
        <MenuTrigger>
          <button className="rbtn small" disabled={!open} title="The page look plus the theme's colours on single-symbol layers (changes the map too)">
            <span className="rbtn-icon"><ColorRegular /></span>
            <span className="rbtn-label">Look + layer colours ▾</span>
          </button>
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            {themes.map((t) => (
              <MenuItem key={t.id} title={t.description} onClick={() => void applyTheme(t.id, true)}>{t.name}</MenuItem>
            ))}
          </MenuList>
        </MenuPopover>
      </Menu>
    </div>
  );
}

/** Layout → Template: keep the open layout as a template, or delete one of yours. */
function TemplateGroup() {
  const open = useLayouts((s) => !!s.doc);
  const [mine, setMine] = useState<LayoutTemplate[]>([]);
  const load = () => api.listLayoutTemplates().then((l) => setMine(l.templates.filter((t) => !t.builtin)), () => setMine([]));
  return (
    <div className="rcol">
      <button className="rbtn small" disabled={!open} title="Keep this page as a template for new layouts" onClick={() => void saveLayoutAsTemplate()}>
        <span className="rbtn-icon"><DocumentSaveRegular /></span>
        <span className="rbtn-label">Save as Template</span>
      </button>
      <Menu onOpenChange={(_, d) => d.open && void load()}>
        <MenuTrigger>
          <button className="rbtn small" title="Delete one of your templates">
            <span className="rbtn-icon"><DeleteRegular /></span>
            <span className="rbtn-label">Delete Template ▾</span>
          </button>
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            {mine.length === 0 && <MenuItem disabled>No templates of yours yet</MenuItem>}
            {mine.map((t) => (
              <MenuItem key={t.id} onClick={() => void deleteLayoutTemplate(t)}>{t.name}</MenuItem>
            ))}
          </MenuList>
        </MenuPopover>
      </Menu>
    </div>
  );
}

/** Layout → Page: size and orientation of the open layout's page. */
function PageMenu() {
  const setup = () => useDialog.getState().show({ kind: "layoutPage" });
  const page = useLayouts((s) => s.doc?.page);
  const cur = page ? describePage(page) : null;
  const set = (size: string, o: Orientation) => useLayouts.getState().setPage({ ...pageFor(size, o, page?.margin), background: page?.background ?? "#ffffff", bleed: page?.bleed, cropMarks: page?.cropMarks });
  return (
    <div className="rcol">
      <Menu>
        <MenuTrigger>
          <button className="rbtn small" disabled={!page}>
            <span className="rbtn-icon"><PageFitRegular /></span>
            <span className="rbtn-label">Size: {cur?.size ?? "Custom"} ▾</span>
          </button>
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            {Object.keys(PAGE_SIZES).map((s) => (
              <MenuItem key={s} onClick={() => set(s, cur?.orientation ?? "landscape")}>{s}</MenuItem>
            ))}
          </MenuList>
        </MenuPopover>
      </Menu>
      <Menu>
        <MenuTrigger>
          <button className="rbtn small" disabled={!page}>
            <span className="rbtn-icon"><ArrowRotateClockwiseRegular /></span>
            <span className="rbtn-label">Orientation: {cur?.orientation ?? (page && page.width > page.height ? "landscape" : "portrait")} ▾</span>
          </button>
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            {(["landscape", "portrait"] as Orientation[]).map((o) => (
              <MenuItem key={o} onClick={() => set(cur?.size ?? "Letter", o)}>{o}</MenuItem>
            ))}
          </MenuList>
        </MenuPopover>
      </Menu>
      <button className="rbtn small" disabled={!page} title="Size, margin, bleed and crop marks, background, author and guides" onClick={setup}>
        <span className="rbtn-icon"><DocumentSettingsRegular /></span>
        <span className="rbtn-label">Page Setup…</span>
      </button>
    </div>
  );
}

/** Layout → Map Series: set it up, step through its pages in the view, export all pages (C3.2). */
function SeriesGroup() {
  const series = useLayouts((s) => s.doc?.series ?? null);
  const open = useLayouts((s) => !!s.doc);
  const { pages, page, error } = useSeriesView();
  const show = useSeriesView((s) => s.show);
  return (
    <div className="rcol">
      <button className="rbtn small" disabled={!open} title="One page per feature of an index layer" onClick={() => useDialog.getState().show({ kind: "layoutSeries" })}>
        <span className="rbtn-icon"><DocumentOnePageMultipleRegular /></span>
        <span className="rbtn-label">{series ? "Map Series…" : "Set Up Map Series…"}</span>
      </button>
      {series && (
        <div className="btn-row" title={error ?? "Show a page of the series in the Layout view"}>
          <button className="rbtn small" aria-label="Previous series page" disabled={!pages.length || page === 0} onClick={() => show(page === null ? pages.length - 1 : page - 1)}>◀</button>
          <span className="muted small" data-testid="series-caption">{error ? "Series can't be shown" : pageCaption(page, pages)}</span>
          <button className="rbtn small" aria-label="Next series page" disabled={!pages.length || page === pages.length - 1} onClick={() => show(page === null ? 0 : page + 1)}>▶</button>
          {page !== null && <button className="rbtn small" onClick={() => show(null)} title="Show the frames as saved">As saved</button>}
        </div>
      )}
      {series && (
        <Menu>
          <MenuTrigger>
            <button className="rbtn small" disabled={!pages.length} title="Export every page of the series">
              <span className="rbtn-icon"><DocumentPdfRegular /></span>
              <span className="rbtn-label">Export Series ▾</span>
            </button>
          </MenuTrigger>
          <MenuPopover>
            <MenuList>
              <MenuItem onClick={() => void exportLayoutSeries("pdf")}>One PDF with every page</MenuItem>
              <MenuItem onClick={() => void exportLayoutSeries("png")}>One PNG per page (choose a folder)</MenuItem>
            </MenuList>
          </MenuPopover>
        </Menu>
      )}
    </div>
  );
}

/** Layout → Arrange: stacking order and alignment of the selected elements (C3.0). */
function ArrangeMenu() {
  const n = useLayouts((s) => s.selected.length);
  const order: [OrderOp, string][] = [["front", "Bring to Front"], ["forward", "Bring Forward"], ["backward", "Send Backward"], ["back", "Send to Back"]];
  const align: [AlignOp, string][] = [["left", "Align Left"], ["center", "Align Centre"], ["right", "Align Right"], ["top", "Align Top"], ["middle", "Align Middle"], ["bottom", "Align Bottom"]];
  const menu = (label: string, items: [string, string][], run: (op: string) => void, tip: string) => (
    <Menu>
      <MenuTrigger>
        <button className="rbtn small" disabled={!n} title={tip}>
          <span className="rbtn-icon">{label === "Align" ? <AlignLeftRegular /> : label === "Spread" ? <TextAlignLeftRegular /> : <LayerRegular />}</span>
          <span className="rbtn-label">{label} ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {items.map(([op, text]) => (
            <MenuItem key={op} onClick={() => run(op)}>{text}</MenuItem>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
  const more: [string, string][] = [["distH", "Distribute Horizontally"], ["distV", "Distribute Vertically"], ["sameW", "Same Width as First"], ["sameH", "Same Height as First"], ["group", "Group"], ["ungroup", "Ungroup"]];
  const runMore = (op: string) => {
    const s = useLayouts.getState();
    if (op === "distH") s.distribute("horizontal");
    else if (op === "distV") s.distribute("vertical");
    else if (op === "sameW") s.matchSize("w");
    else if (op === "sameH") s.matchSize("h");
    else if (op === "group") s.group();
    else s.ungroup();
  };
  return (
    <div className="rcol">
      {menu("Order", order, (op) => useLayouts.getState().arrange(op as OrderOp), "Change which selected elements are in front")}
      {menu("Align", align, (op) => useLayouts.getState().align(op as AlignOp), "Line up the selected elements (one element aligns to the page margins)")}
      {menu("Spread", more, runMore, "Equal gaps (3 or more), same size, group so they move together")}
    </div>
  );
}

/** Layout → Export: the resolution for PNG pixels and the images inside a PDF. */
function DpiBox() {
  const dpi = useExportDpi((s) => s.dpi);
  const [text, setText] = useState(String(dpi));
  useEffect(() => setText((t) => (parseDpi(t) === dpi ? t : String(dpi))), [dpi]);
  const bad = parseDpi(text) === null;
  return (
    <div className="rcol scale-box">
      <label className="muted">Resolution (dpi)</label>
      <input
        className="scale-input"
        value={text}
        inputMode="numeric"
        aria-invalid={bad}
        title={`${DPI_MIN}–${DPI_MAX} dpi. PNG pixels, and the images in a PDF (map images are capped at 300 dpi).`}
        onChange={(e) => {
          setText(e.target.value);
          const v = parseDpi(e.target.value);
          if (v !== null) useExportDpi.getState().set(v);
        }}
        onBlur={() => setText(String(dpi))}
        aria-label="Export resolution in dots per inch"
      />
    </div>
  );
}

/** Layout → Map Frame: the scale of the selected (or first) frame, "1 : [24,000]". */
function ScaleBox() {
  const frame = useLayouts((s) => {
    const fs = s.doc?.elements.filter((e) => e.kind === "map_frame") ?? [];
    return (fs.find((f) => s.selected.includes(f.id)) ?? fs[0]) as { id: string; scale: number } | undefined;
  });
  const [text, setText] = useState("");
  useEffect(() => setText(frame ? String(Math.round(frame.scale)) : ""), [frame?.id, frame?.scale]);
  return (
    <div className="rcol scale-box">
      <label className="muted">Scale 1 :</label>
      <input
        className="scale-input"
        disabled={!frame}
        value={text}
        inputMode="numeric"
        onChange={(e) => {
          setText(e.target.value);
          const v = Number(e.target.value.replace(/[,\s]/g, ""));
          if (frame && Number.isFinite(v) && v >= 1) useLayouts.getState().update(frame.id, { scale: v }, "scale");
        }}
        aria-label="Map frame scale denominator"
      />
    </div>
  );
}

function BasemapGallery() {
  const basemap = useApp((s) => s.basemap);
  return (
    <Menu checkedValues={{ basemap: [basemap] }} onCheckedValueChange={(_, d) => useApp.getState().setBasemap(d.checkedItems[0] as never)}>
      <MenuTrigger>
        <button className="rbtn large">
          <span className="rbtn-icon"><ImageMultipleRegular /></span>
          <span className="rbtn-label">Basemap ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {BASEMAP_OPTIONS.map(([id, label]) => (
            <MenuItemRadio key={id} name="basemap" value={id}>
              {label}
            </MenuItemRadio>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

function ToolGallery() {
  const specs = useRun((s) => s.specs);
  const tools = TOOL_BUTTONS.map((id) => specs[id]).filter(Boolean);
  return (
    <div className="gallery">
      {tools.map((t) => {
        const c = toolCommand(t.id, t.name, t.description);
        return <RButton key={t.id} cmd={c} size="small" />;
      })}
    </div>
  );
}

/** "Tell me what you want to do" command search. */
function TellMe() {
  const specs = useRun((s) => s.specs);
  const [value, setValue] = useState("");
  const all = useMemo(() => {
    const tools = Object.values(specs).filter((s) => s.role !== "data").map((s) => toolCommand(s.id, s.name, s.description));
    return [...Object.values(commands), ...tools];
  }, [specs]);
  const q = value.toLowerCase();
  const matches = q ? all.filter((c) => `${c.label} ${c.tip ?? ""}`.toLowerCase().includes(q)).slice(0, 12) : [];
  return (
    <Combobox
      className="tellme"
      size="small"
      placeholder="Command search (Alt+Q)"
      value={value}
      freeform
      expandIcon={<SearchRegular />}
      onChange={(e) => setValue(e.target.value)}
      onOptionSelect={(_, d) => {
        const c = all.find((x) => x.id === d.optionValue);
        setValue("");
        c?.run();
      }}
    >
      {matches.map((c) => (
        <Option key={c.id} value={c.id} text={c.label}>
          {c.label}
        </Option>
      ))}
    </Combobox>
  );
}
