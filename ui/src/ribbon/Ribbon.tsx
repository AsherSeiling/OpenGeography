// Office / ArcGIS Pro style ribbon: a project cell at the top left, a tab row (tabs, Save / Undo / Redo, search) and a band of labelled
// groups of large and small buttons. The open tab and its band are outlined as one shape. Contextual tabs appear for a layer and a layout.
import { Menu, MenuItem, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import { RIcon } from "./icons/RIcon";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { useDialog } from "../actions";
import { describePage, PAGE_SIZES, pageFor, type Orientation } from "../layout/types";
import { useNav } from "../nav/navStore";
import { usePanes } from "../panes/panes";
import { useLayouts } from "../store/layout";
import { useLayoutView } from "../layout/view";
import { useApp } from "../store/store";
import { isAppearanceOpen } from "../theme/appearance";
import { commands } from "./commands";
import { ProjectBox } from "./projectBox";
import { QuickAccess } from "./quickAccess";
import { RButton } from "./rbutton";
import { fitRibbon, type Variant } from "./ribbonFit";
import { TopSearch } from "./search";
import { KEY_TIPS, LAYOUT_ORDER, MAIN_TABS, TABS } from "./tabsDef";
import { useRibbonTabs, visibleGroups, type Group } from "./tabs";
import { useSettings } from "../store/settings";
import { useEdit } from "../edit/editStore";
import { useRun } from "../store/model";
import { useSaved } from "../edit/editActions";

/** The contextual tab for the active layer: its name follows what the layer is. */
const layerTabLabel = (geom: string) => (geom === "Table" ? "Table" : geom === "Raster" ? "Raster Layer" : "Feature Layer");

export function Ribbon() {
  const [tab, setTab] = useState("Map");
  // Re-render when state that affects enabled/checked changes.
  useApp((s) => s.tool);
  useApp((s) => s.selection);
  useApp((s) => s.showFps);
  useApp((s) => s.dark);
  useNav((s) => s.navPanel);
  useNav((s) => s.measureMode);
  usePanes((s) => s.open); // pressed state of the View › Windows buttons
  isAppearanceOpen(); // pressed state of View › Colours
  const active = useApp((s) => (s.activeLayer ? s.layers[s.activeLayer] : null));
  // Any layer gets its tab (tables and rasters too), so the field, filter and export commands are always on a tab.
  const contextual = !!active;
  // The layout tabs show while a layout is the active tab of the window; Back to Map (or any map tab) brings the map's tabs back.
  const layoutId = useLayouts((s) => s.id);
  const workspace = useLayoutView((s) => s.workspace);
  const inLayout = !!layoutId && workspace;
  useLayouts((s) => s.doc);
  // Shape editing commands read the edit session; redraw when it changes.
  useEdit((s) => s.status);
  useEdit((s) => s.tool);
  useEdit((s) => s.snapping);
  useEdit((s) => s.layerId);
  useSaved((s) => s.saved);
  useLayouts((s) => s.selected);
  useLayoutView((s) => s.fit); // pressed state of View › Fit Page
  useLayoutView((s) => s.preflightOpen);
  // The Map Frame tab is there only while a map frame is selected.
  const frameSelected = useLayouts((s) => !!s.doc?.elements.some((e) => e.kind === "map_frame" && s.selected.includes(e.id)));
  const toolCount = useRun((s) => Object.keys(s.specs).length); // the Tools group on Analysis is as wide as the tool list is long
  const developer = useSettings((s) => !!s.settings.showDeveloperTools);
  const registered = useRibbonTabs((s) => s.tabs);
  // Opening a layout brings its Page tab forward; the map's tabs (Map, Analysis, Feature Layer) leave while a layout is open.
  useEffect(() => {
    if (inLayout) setTab("Page");
    else setTab((t) => (LAYOUT_ORDER.includes(t) && !MAIN_TABS.includes(t) ? "Map" : t));
  }, [inLayout, layoutId]);
  const extra = Object.values(registered).filter((t) => t.visible);
  const allowed = inLayout ? LAYOUT_ORDER.includes(tab) || (tab === "Map Frame" && frameSelected) : MAIN_TABS.includes(tab) || (tab === "Feature Layer" && contextual);
  const current = allowed || extra.some((t) => t.id === tab) ? tab : inLayout ? "Page" : "Map";
  const groups: Group[] = useMemo(() => visibleGroups(TABS[current] ?? Object.values(registered).find((t) => t.visible && t.id === current)?.groups ?? [], { inLayout: !!inLayout, developer }), [current, registered, inLayout, developer]);

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
  const layoutKey = `${current}|${groups.length}|${toolCount}`;
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
      <div className="rib">
      <div className="ribbon-tabs" role="tablist" aria-label="Ribbon">
        <ProjectBox />
        {inLayout && (
          <button className="rtab-back" title="Go back to the map" onClick={() => commands.paneMap.run()}>
            <RIcon name="back" />
            <span>Back to Map</span>
          </button>
        )}
        <div className="rtabs">
          {(inLayout ? LAYOUT_ORDER : MAIN_TABS).map((t) => (
            <button key={t} role="tab" aria-selected={current === t} className={`rtab ${current === t ? "on" : ""}`} onClick={() => pickTab(t)} onDoubleClick={() => { setCollapsed((c) => !c); setPeek(false); }} title="Double-click to collapse or expand the ribbon (Ctrl+F1)">
              {t}
              {tips && KEY_TIPS[t] && <span className="keytip">{KEY_TIPS[t]}</span>}
            </button>
          ))}
          {inLayout && frameSelected && (
            <div className="rtab-context">
              <span className="rtab-context-label">Selected</span>
              <button role="tab" aria-selected={current === "Map Frame"} className={`rtab ctx ${current === "Map Frame" ? "on" : ""}`} onClick={() => pickTab("Map Frame")}>
                Map Frame
              </button>
            </div>
          )}
          {contextual && !inLayout && (
            <div className="rtab-context">
              <span className="rtab-context-label">Layer</span>
              <button role="tab" aria-selected={current === "Feature Layer"} className={`rtab ctx ${current === "Feature Layer" ? "on" : ""}`} onClick={() => pickTab("Feature Layer")}>
                {layerTabLabel(active!.geomType)}
              </button>
            </div>
          )}
          {extra.map((t) => (
            <div key={t.id} className="rtab-context">
              <span className="rtab-context-label">{t.band}</span>
              <button role="tab" aria-selected={current === t.id} className={`rtab ctx ${current === t.id ? "on" : ""}`} onClick={() => pickTab(t.id)}>
                {t.label}
              </button>
            </div>
          ))}
        </div>
        <div className="ribbon-spacer" />
        <QuickAccess />
        <TopSearch />
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

// Kept here, not in controls.tsx: the white-paper default below is the one raw colour the hex test allows in this file.
/** Layout → Page: size and orientation of the open layout's page. */
export function PageMenu() {
  const setup = () => useDialog.getState().show({ kind: "layoutPage" });
  const page = useLayouts((s) => s.doc?.page);
  const cur = page ? describePage(page) : null;
  const set = (size: string, o: Orientation) => useLayouts.getState().setPage({ ...pageFor(size, o, page?.margin), background: page?.background ?? "#ffffff", bleed: page?.bleed, cropMarks: page?.cropMarks });
  return (
    <div className="rcol">
      <Menu>
        <MenuTrigger>
          <button className="rbtn small" disabled={!page}>
            <span className="rbtn-icon"><RIcon name="fit-page" /></span>
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
            <span className="rbtn-icon"><RIcon name="orientation" /></span>
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
        <span className="rbtn-icon"><RIcon name="page-setup" /></span>
        <span className="rbtn-label">Page Setup…</span>
      </button>
    </div>
  );
}
