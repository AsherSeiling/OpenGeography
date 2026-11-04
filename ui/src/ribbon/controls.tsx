// The ribbon's custom controls: galleries, menus and boxes that sit in a group beside plain command buttons.
import { Menu, MenuDivider, MenuItem, MenuItemRadio, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import { AlignLeftRegular, ColorRegular, DeleteRegular, DocumentOnePageMultipleRegular, DocumentPdfRegular, DocumentRegular, DocumentSaveRegular, LayerRegular, PaintBrushRegular, TextAlignLeftRegular, ImageMultipleRegular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { api, errorText, type LayoutTemplate } from "../ipc/api";
import { pageCaption, useSeriesView } from "../layout/series";
import { useDialog } from "../actions";
import { applyTheme, exportLayoutSeries, deleteLayoutTemplate, newLayout, newLayoutFromTemplate, saveLayoutAsTemplate } from "../layout/actions";
import { PAGE_SIZES, type Orientation } from "../layout/types";
import { useLayouts } from "../store/layout";
import type { AlignOp, OrderOp } from "../layout/arrange";
import { DPI_MAX, DPI_MIN, parseDpi, useExportDpi } from "../layout/exportDpi";
import { useApp } from "../store/store";
import { BASEMAP_OPTIONS } from "./commands";

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

/** Layout → Map Series: set it up, step through its pages in the view, export all pages (C3.2). */
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
export function ArrangeMenu() {
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
