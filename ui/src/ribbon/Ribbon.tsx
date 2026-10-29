// Office / ArcGIS Pro style ribbon: tabs → labeled groups → large and small
// buttons. Contextual tabs appear when a layer is selected.
import { Combobox, Menu, MenuDivider, MenuItem, MenuItemRadio, MenuList, MenuPopover, MenuTrigger, Option, Tooltip } from "@fluentui/react-components";
import { DocumentPdfRegular, DocumentRegular, ImageMultipleRegular, SearchRegular } from "@fluentui/react-icons";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { api, errorText, type LayoutTemplate } from "../ipc/api";
import { pageCaption, useSeriesView } from "../layout/series";
import { useDialog } from "../actions";
import { exportLayoutSeries, deleteLayoutTemplate, newLayout, newLayoutFromTemplate, saveLayoutAsTemplate } from "../layout/actions";
import { describePage, PAGE_SIZES, pageFor, type Orientation } from "../layout/types";
import { useLayouts } from "../store/layout";
import type { AlignOp, OrderOp } from "../layout/arrange";
import { DPI_MAX, DPI_MIN, parseDpi, useExportDpi } from "../layout/exportDpi";
import { useRun } from "../store/model";
import { useNav } from "../nav/navStore";
import { useApp } from "../store/store";
import { BASEMAP_OPTIONS, commands, TOOL_BUTTONS, toolCommand, type Command } from "./commands";

type Size = "large" | "small";
type Item = { cmd: string; size?: Size } | { custom: ReactNode };
interface Group {
  label: string;
  items: Item[];
}

const TABS: Record<string, Group[]> = {
  Map: [
    { label: "Navigate", items: [{ cmd: "explore" }, { cmd: "fullExtent" }] },
    { label: "Layer", items: [{ cmd: "addData" }, { cmd: "catalog" }, { cmd: "tiger" }, { cmd: "nationalMap" }, { cmd: "elevation" }, { custom: <BasemapGallery /> }] },
    {
      label: "Selection",
      items: [{ cmd: "select" }, { cmd: "selectBox", size: "small" }, { cmd: "selectByAttributes", size: "small" }, { cmd: "selectByLocation", size: "small" }, { cmd: "zoomSelection", size: "small" }, { cmd: "clearSelection" }],
    },
    { label: "Inquiry", items: [{ cmd: "identify" }, { cmd: "measure" }, { cmd: "measureArea", size: "small" }] },
    { label: "Land Nav", items: [{ cmd: "resection" }, { cmd: "intersection" }] },
    { label: "Coordinate System", items: [{ cmd: "mapCrs" }, { cmd: "projectionBuilder" }, { cmd: "tissot" }] },
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
    { label: "Add", items: [{ cmd: "addData" }, { cmd: "catalog" }, { cmd: "tiger" }, { cmd: "nationalMap" }, { cmd: "arcgis" }, { cmd: "stac" }, { cmd: "ogc" }, { cmd: "overture" }, { cmd: "elevation" }] },
    { label: "Fields", items: [{ cmd: "addField" }, { cmd: "calcField" }, { cmd: "calcGeometry" }, { cmd: "census" }] },
    { label: "Coordinate Systems", items: [{ cmd: "defineProjection" }, { cmd: "mapCrs" }] },
    { label: "Export", items: [{ cmd: "exportLayer" }, { cmd: "layerFromSelection" }, { cmd: "splitByAttribute" }] },
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
    { label: "Insert", items: [{ cmd: "layoutMapFrame" }, { cmd: "layoutText" }, { cmd: "layoutLegend" }, { cmd: "layoutScaleBar" }, { cmd: "layoutNorthArrow" }, { cmd: "layoutSources" }] },
    { label: "Map Frame", items: [{ cmd: "layoutFromMap" }, { cmd: "layoutFrameCrs" }, { custom: <ScaleBox /> }] },
    { label: "Arrange", items: [{ custom: <ArrangeMenu /> }] },
    { label: "Map Series", items: [{ custom: <SeriesGroup /> }] },
    { label: "Export", items: [{ cmd: "layoutPdf" }, { cmd: "layoutPng" }, { cmd: "layoutSvg" }, { custom: <DpiBox /> }] },
  ],
  "Feature Layer": [
    { label: "Layer", items: [{ cmd: "layerProps" }, { cmd: "zoomLayer" }, { cmd: "symbology" }, { cmd: "openTable" }] },
    { label: "Fields", items: [{ cmd: "addField" }, { cmd: "calcField" }, { cmd: "calcGeometry" }] },
    { label: "Data", items: [{ cmd: "definitionQuery" }, { cmd: "defineProjection" }, { cmd: "exportLayer" }, { cmd: "layerFromSelection" }, { cmd: "splitByAttribute" }] },
    { label: "Remove", items: [{ cmd: "removeLayer" }] },
  ],
};

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
  // Opening a layout tab brings its contextual ribbon tab forward, as in ArcGIS Pro.
  useEffect(() => {
    if (layoutId) setTab("Layout");
  }, [layoutId]);
  const current = (tab === "Feature Layer" && !contextual) || (tab === "Layout" && !layoutId) ? "Map" : tab;

  return (
    <div className="ribbon">
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
        {["Map", "Insert", "Analysis", "Data", "View"].map((t) => (
          <button key={t} className={`rtab ${current === t ? "on" : ""}`} onClick={() => setTab(t)}>
            {t}
          </button>
        ))}
        {layoutId && (
          <div className="rtab-context">
            <span className="rtab-context-label">Layout</span>
            <button className={`rtab ctx ${current === "Layout" ? "on" : ""}`} onClick={() => setTab("Layout")}>
              Layout
            </button>
          </div>
        )}
        {contextual && (
          <div className="rtab-context">
            <span className="rtab-context-label">Layer</span>
            <button className={`rtab ctx ${current === "Feature Layer" ? "on" : ""}`} onClick={() => setTab("Feature Layer")}>
              Feature Layer
            </button>
          </div>
        )}
        <div className="ribbon-spacer" />
        <TellMe />
      </div>
      <div className="ribbon-body">
        {TABS[current].map((g) => (
          <RibbonGroup key={g.label} group={g} />
        ))}
      </div>
    </div>
  );
}

function RibbonGroup({ group }: { group: Group }) {
  // Consecutive small buttons stack in columns of three, as in Office.
  const cols: ReactNode[] = [];
  let smalls: string[] = [];
  const flush = () => {
    if (smalls.length) cols.push(<div key={`s${cols.length}`} className="rcol">{smalls.map((id) => <RButton key={id} cmd={commands[id]} size="small" />)}</div>);
    smalls = [];
  };
  group.items.forEach((it, i) => {
    if ("custom" in it) {
      flush();
      cols.push(<div key={`c${i}`}>{it.custom}</div>);
    } else if (it.size === "small") {
      smalls.push(it.cmd);
      if (smalls.length === 3) flush();
    } else {
      flush();
      cols.push(<RButton key={it.cmd} cmd={commands[it.cmd]} size="large" />);
    }
  });
  flush();
  return (
    <div className="rgroup">
      <div className="rgroup-items">{cols}</div>
      <div className="rgroup-label">{group.label}</div>
    </div>
  );
}

export function RButton({ cmd, size }: { cmd: Command; size: Size }) {
  const disabled = cmd.enabled ? !cmd.enabled() : false;
  const checked = cmd.checked?.() ?? false;
  const btn = (
    <button className={`rbtn ${size} ${checked ? "checked" : ""}`} disabled={disabled} onClick={() => cmd.run()} aria-pressed={cmd.checked ? checked : undefined}>
      <span className="rbtn-icon">{cmd.icon}</span>
      <span className="rbtn-label">{cmd.label}</span>
    </button>
  );
  const tip = [cmd.tip, cmd.keys && `(${cmd.keys})`].filter(Boolean).join(" ");
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

/** Layout → Template: keep the open layout as a template, or delete one of yours. */
function TemplateGroup() {
  const open = useLayouts((s) => !!s.doc);
  const [mine, setMine] = useState<LayoutTemplate[]>([]);
  const load = () => api.listLayoutTemplates().then((l) => setMine(l.templates.filter((t) => !t.builtin)), () => setMine([]));
  return (
    <div className="rcol">
      <button className="rbtn small" disabled={!open} title="Keep this page as a template for new layouts" onClick={() => void saveLayoutAsTemplate()}>
        <span className="rbtn-icon"><DocumentRegular /></span>
        <span className="rbtn-label">Save as Template</span>
      </button>
      <Menu onOpenChange={(_, d) => d.open && void load()}>
        <MenuTrigger>
          <button className="rbtn small" title="Delete one of your templates">
            <span className="rbtn-icon"><DocumentRegular /></span>
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
  const page = useLayouts((s) => s.doc?.page);
  const cur = page ? describePage(page) : null;
  const set = (size: string, o: Orientation) => useLayouts.getState().setPage({ ...pageFor(size, o, page?.margin), background: page?.background ?? "#ffffff" });
  return (
    <div className="rcol">
      <Menu>
        <MenuTrigger>
          <button className="rbtn small" disabled={!page}>
            <span className="rbtn-icon"><DocumentRegular /></span>
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
            <span className="rbtn-icon"><DocumentRegular /></span>
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
        <span className="rbtn-icon"><DocumentRegular /></span>
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
          <span className="rbtn-icon"><DocumentRegular /></span>
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
  return (
    <div className="rcol">
      {menu("Order", order, (op) => useLayouts.getState().arrange(op as OrderOp), "Change which selected elements are in front")}
      {menu("Align", align, (op) => useLayouts.getState().align(op as AlignOp), "Line up the selected elements (one element aligns to the page margins)")}
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
