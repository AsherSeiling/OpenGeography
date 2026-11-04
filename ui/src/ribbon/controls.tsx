// The ribbon's custom controls: galleries, menus and boxes that sit in a group beside plain command buttons.
import { Menu, MenuDivider, MenuItem, MenuItemRadio, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import { AlignLeftRegular, ResizeRegular, MapRegular, ShapesRegular, ColorRegular, DeleteRegular, DocumentOnePageMultipleRegular, DocumentPdfRegular, DocumentRegular, DocumentSaveRegular, LayerRegular, PaintBrushRegular, TextAlignLeftRegular, ImageMultipleRegular } from "@fluentui/react-icons";
import { useEffect, useState, type ReactElement } from "react";
import { api, errorText, type LayoutTemplate } from "../ipc/api";
import { pageCaption, useSeriesView } from "../layout/series";
import { useDialog } from "../actions";
import { applyTheme, exportLayoutSeries, deleteLayoutTemplate, insertMapFrame, newLayout, newLayoutFromTemplate, saveLayoutAsTemplate } from "../layout/actions";
import { followedMap, PAGE_SIZES, type MapFrame, type Orientation } from "../layout/types";
import { useLayouts } from "../store/layout";
import type { AlignOp, OrderOp } from "../layout/arrange";
import { DPI_MAX, DPI_MIN, parseDpi, useExportDpi } from "../layout/exportDpi";
import { useApp } from "../store/store";
import { BASEMAP_OPTIONS, commands } from "./commands";

/** Insert → Layouts: pick a page size and orientation for a new layout. */
export function NewLayoutGallery() {
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
export function TemplateSubmenu() {
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
export function ThemeMenu() {
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
export function TemplateGroup() {
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

/** Page › Map Series: set it up and step through its pages in the view (C3.2). Exporting every page is in Export › Map Series. */
export function SeriesGroup() {
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
    </div>
  );
}

const ORDER_OPS: [OrderOp, string][] = [["front", "Bring to Front"], ["forward", "Bring Forward"], ["backward", "Send Backward"], ["back", "Send to Back"]];
const ALIGN_OPS: [AlignOp, string][] = [["left", "Align Left"], ["center", "Align Centre"], ["right", "Align Right"], ["top", "Align Top"], ["middle", "Align Middle"], ["bottom", "Align Bottom"]];

/** One Arrange command: a button that opens a short list of operations on the selected elements. Greyed out until something is selected. */
function ArrangeDrop({ label, icon, tip, items, run, need = 1 }: { label: string; icon: ReactElement; tip: string; items: [string, string][]; run: (op: string) => void; need?: number }) {
  const n = useLayouts((s) => s.selected.length);
  return (
    <Menu>
      <MenuTrigger>
        <button className="rbtn large" disabled={n < need} title={tip}>
          <span className="rbtn-icon">{icon}</span>
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
}

/** Arrange › Order: which selected elements are in front. */
export const OrderMenu = () => <ArrangeDrop label="Order" icon={<LayerRegular />} tip="Change which selected elements are in front" items={ORDER_OPS} run={(op) => useLayouts.getState().arrange(op as OrderOp)} />;
/** Arrange › Line Up › Align (one element aligns to the page margins). */
export const AlignMenu = () => <ArrangeDrop label="Align" icon={<AlignLeftRegular />} tip="Line up the selected elements (one element aligns to the page margins)" items={ALIGN_OPS} run={(op) => useLayouts.getState().align(op as AlignOp)} />;
/** Arrange › Line Up › Distribute: equal gaps, three or more elements. */
export const DistributeMenu = () => <ArrangeDrop label="Distribute" icon={<TextAlignLeftRegular />} tip="Equal gaps between three or more selected elements" need={3} items={[["h", "Distribute Horizontally"], ["v", "Distribute Vertically"]]} run={(op) => useLayouts.getState().distribute(op === "h" ? "horizontal" : "vertical")} />;
/** Arrange › Line Up › Same Size, matched to the first selected element. */
export const SameSizeMenu = () => <ArrangeDrop label="Same Size" icon={<ResizeRegular />} tip="Give the selected elements the width or height of the first one" need={2} items={[["w", "Same Width as First"], ["h", "Same Height as First"]]} run={(op) => useLayouts.getState().matchSize(op as "w" | "h")} />;

/** Export › Map Series: every page of the series as one PDF or one PNG each (C3.2). */
export function SeriesExport() {
  const series = useLayouts((s) => s.doc?.series ?? null);
  const pages = useSeriesView((s) => s.pages);
  return (
    <Menu>
      <MenuTrigger>
        <button className="rbtn large" disabled={!series || !pages.length} title={series ? "Export every page of the series" : "Set up a map series first (Page › Map Series)"}>
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
  );
}

/** Map Frame › Map › Which Map: the map whose layers the selected frame draws. */
export function WhichMapMenu() {
  const frame = useLayouts((s) => {
    const fs = s.doc?.elements.filter((e) => e.kind === "map_frame") ?? [];
    return (fs.find((f) => s.selected.includes(f.id)) ?? null) as MapFrame | null;
  });
  const maps = useApp((s) => s.maps);
  const followed = frame ? followedMap(frame.layers) : null;
  const current = frame ? (followed ? `map:${followed}` : Array.isArray(frame.layers) ? "list" : "active") : "";
  return (
    <Menu checkedValues={{ map: current ? [current] : [] }} onCheckedValueChange={(_, d) => {
      const v = d.checkedItems[0];
      if (!frame || !v) return;
      useLayouts.getState().update(frame.id, { layers: v === "active" ? "map" : { map: v.slice(4) } });
    }}>
      <MenuTrigger>
        <button className="rbtn large" disabled={!frame} title="Choose which map's layers this frame draws">
          <span className="rbtn-icon"><MapRegular /></span>
          <span className="rbtn-label">Which Map ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          <MenuItemRadio name="map" value="active">The active map</MenuItemRadio>
          {maps.map((m) => (
            <MenuItemRadio key={m.id} name="map" value={`map:${m.id}`}>{m.name}</MenuItemRadio>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** Insert › Maps › Map Frame: with more than one map the button asks which map the new frame draws; with one it just inserts. */
export function MapFrameInsert() {
  const maps = useApp((s) => s.maps);
  const inLayout = useLayouts((s) => !!s.doc);
  const cmd = commands.layoutMapFrame;
  const button = (
    <button className="rbtn rail-btn" disabled={!inLayout} title={cmd.tip} aria-label={maps.length > 1 ? "Map Frame, choose a map" : "Map Frame"} onClick={maps.length > 1 ? undefined : () => cmd.run()}>
      <span className="rbtn-icon">{cmd.icon}</span>
      <span className="rbtn-label">Map Frame{maps.length > 1 ? " ▾" : ""}</span>
    </button>
  );
  if (maps.length < 2) return button;
  return (
    <Menu>
      <MenuTrigger>{button}</MenuTrigger>
      <MenuPopover>
        <MenuList>
          <MenuItem onClick={() => insertMapFrame()}>The active map</MenuItem>
          {maps.map((m) => (
            <MenuItem key={m.id} onClick={() => insertMapFrame({ layers: { map: m.id } })}>{m.name}</MenuItem>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** Insert › Graphics › Shape: rectangle, ellipse, triangle, line or arrow. */
export function ShapeMenu() {
  const inLayout = useLayouts((s) => !!s.doc);
  const ids = ["layoutRect", "layoutEllipse", "layoutTriangle", "layoutLine", "layoutArrow"];
  return (
    <Menu>
      <MenuTrigger>
        <button className="rbtn rail-btn" disabled={!inLayout} title="Insert a rectangle, ellipse, triangle, line or arrow">
          <span className="rbtn-icon"><ShapesRegular /></span>
          <span className="rbtn-label">Shape ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {ids.map((id) => (
            <MenuItem key={id} icon={commands[id].icon} onClick={() => commands[id].run()}>{commands[id].label}</MenuItem>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** Layout → Export: the resolution for PNG pixels and the images inside a PDF. */
export function DpiBox() {
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
export function ScaleBox() {
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

export function BasemapGallery() {
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
