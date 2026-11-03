// The Shift+A tool menu (plan §5): search head, category rail, list, preview, key hints. Mounted once in Dialogs.tsx so it works with
// the Node Editor closed; reads the catalogue from `tool_specs` only. Where a pick goes: toolPlacement.ts.
import { DatabaseRegular, SearchRegular, StarFilled, StarRegular, LayerRegular } from "@fluentui/react-icons";
import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useShallow } from "zustand/react/shallow";
import { addData } from "../actions";
import { showPane } from "../panes/panes";
import { specs as specsOf, useModel, useRun } from "../store/model";
import { useApp } from "../store/store";
import { addGroup, compareCategories, familyOf, SEP, splitCategory } from "./categories";
import { ToolIcon } from "./toolIcon";
import { useToolMenuShortcut } from "./toolMenuKeys";
import { closeToolMenu, useToolMenu } from "./toolMenuStore";
import { describePlacement, editorHandle, editorVisible, placeTool, whenEditorVisible } from "./toolPlacement";
import { searchTools, type Searchable } from "./toolSearch";

type Kind = "tool" | "output" | "layer";
interface Item extends Searchable {
  kind: Kind;
  group: string;
  /** Tool id to add (`input_layer` for a layer). */
  tool: string;
  params?: Record<string, unknown>;
}
interface Row {
  item: Item;
  hit?: string;
}
interface Section {
  title: string | null;
  rows: Row[];
}

const START = "Start";
const LAYERS = "Layers";
const MENU_W = 680;
const MENU_H = 452;

/** Every tool that can be added, from `tool_specs`. */
function useTools(): Item[] {
  const specs = useRun((s) => s.specs);
  return useMemo(
    () =>
      Object.values(specs)
        .filter((s) => s.role !== "data" && s.id !== "layer_output" && s.id !== "add_to_map")
        .map((s): Item => ({ id: s.id, name: s.name, category: s.role === "output" ? "Save to File" : s.category, description: s.description, keywords: s.keywords, kind: s.role === "output" ? "output" : "tool", group: addGroup(s), tool: s.id })),
    [specs],
  );
}

/** Layers that can be added as extra inputs to the open tree. */
function useLayerItems(): Item[] {
  const owner = useModel((s) => s.layer);
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && !l.temporary && l.id !== owner)));
  return useMemo(
    () => layers.map((l): Item => ({ id: `layer:${l.id}`, name: l.name, category: "Input", description: `${l.geomType} layer · ${l.featureCount.toLocaleString()} ${l.featureCount === 1 ? "feature" : "features"}`, kind: "layer", group: LAYERS, tool: "input_layer", params: { layer: l.id } })),
    [layers, owner],
  );
}

const KIND_NAME = { vector: "Features", raster: "Raster", table: "Table", any: "Anything" } as const;

export function ToolMenu() {
  useToolMenuShortcut();
  const open = useToolMenu((s) => s.open);
  return open ? <MenuBody /> : null;
}

function MenuBody() {
  const tools = useTools();
  const layerItems = useLayerItems();
  const { fav, recent, pos, toggleFav } = useToolMenu(useShallow((s) => ({ fav: s.fav, recent: s.recent, pos: s.pos, toggleFav: s.toggleFav })));
  const treeLayer = useModel((s) => s.layer);
  const allLayers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && !l.temporary)));
  const [q, setQ] = useState("");
  const [cat, setCat] = useState(START);
  const [hi, setHi] = useState(0);
  /** A tool picked while no layer's tree is open: waits for the layer choice. */
  const [pending, setPending] = useState<Item | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const uid = useId();
  const listId = `${uid}-list`;
  const optId = (i: number) => `${uid}-opt${i}`;

  const groups = useMemo(() => Array.from(new Set(tools.map((t) => t.group))).sort(compareCategories), [tools]);
  const rail = useMemo(() => [START, LAYERS, ...groups], [groups]);
  const countOf = (c: string) => (c === START ? null : c === LAYERS ? layerItems.length : tools.filter((t) => t.group === c).length);

  const chooseItems = useMemo(
    () => allLayers.map((l): Item => ({ id: `choose:${l.id}`, name: l.name, category: "Input", description: `${l.geomType} layer · ${l.featureCount.toLocaleString()} ${l.featureCount === 1 ? "feature" : "features"}`, kind: "layer", group: LAYERS, tool: "input_layer", params: { layer: l.id } })),
    [allLayers],
  );

  const sections: Section[] = useMemo(() => {
    const row = (item: Item): Row => ({ item });
    if (pending) return [{ title: "Choose a layer to add it to", rows: chooseItems.map(row) }];
    if (q.trim()) return [{ title: null, rows: searchTools([...tools, ...layerItems], q, fav).map((h) => ({ item: h.item, hit: h.hit })) }];
    if (cat === START) {
      const byId = (id: string) => tools.find((t) => t.id === id);
      const out: Section[] = [
        { title: "Favourites", rows: fav.map(byId).filter((t): t is Item => !!t).map(row) },
        { title: "Recent", rows: recent.map(byId).filter((t): t is Item => !!t).map(row) },
      ];
      for (const g of groups) out.push({ title: g, rows: tools.filter((t) => t.group === g).sort((a, b) => a.name.localeCompare(b.name)).map(row) });
      return out;
    }
    if (cat === LAYERS) return [{ title: "Other layers, as extra inputs", rows: layerItems.map(row) }];
    const subs = new Map<string, Item[]>();
    for (const t of tools.filter((t) => t.group === cat)) {
      const s = splitCategory(t.category)[1] ?? "";
      subs.set(s, [...(subs.get(s) ?? []), t]);
    }
    return [...subs.keys()].sort().map((s) => ({ title: s || null, rows: subs.get(s)!.sort((a, b) => a.name.localeCompare(b.name)).map(row) }));
  }, [pending, q, cat, tools, layerItems, chooseItems, fav, recent, groups]);

  const shown = useMemo(() => sections.flatMap((s) => s.rows), [sections]);
  const cur = shown[Math.min(hi, shown.length - 1)];
  const hiIdx = cur ? shown.indexOf(cur) : -1;

  useEffect(() => setHi(0), [q, cat, pending]);
  useEffect(() => document.getElementById(optId(hiIdx))?.scrollIntoView?.({ block: "nearest" }), [hiIdx]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    input.current?.focus();
    const away = (e: PointerEvent) => !root.current?.contains(e.target as Node) && closeToolMenu();
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      closeToolMenu();
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc, true);
    };
  }, []);

  /** Add a tool or layer: opens the Node Editor first if it is closed or hidden. */
  const add = useCallback(async (item: Item) => {
    const { pos, wire, addRecent } = useToolMenu.getState();
    closeToolMenu();
    if (item.kind !== "layer") addRecent(item.id);
    if (!editorVisible()) {
      showPane("model");
      await whenEditorVisible();
    }
    const r = editorHandle()?.rect();
    const at = pos && r && pos.x >= r.left && pos.x <= r.right && pos.y >= r.top && pos.y <= r.bottom ? pos : null;
    placeTool(item.tool, { at, params: item.params, wire: at ? wire : null });
  }, []);

  const pick = async (item: Item | undefined) => {
    if (!item) return;
    if (pending) {
      // The layer chosen opens its tree (App follows the active layer); then the tool goes in.
      const id = String(item.params?.layer);
      const tool = pending;
      useApp.getState().setActiveLayer(id);
      const t0 = Date.now();
      while (useModel.getState().layer !== id && Date.now() - t0 < 1500) await new Promise((r) => setTimeout(r, 20));
      void add(tool);
      return;
    }
    if (!treeLayer) {
      setPending(item);
      setQ("");
      return;
    }
    void add(item);
  };

  const move = (i: number) => setHi(Math.max(0, Math.min(shown.length - 1, i)));
  const onKey = (e: React.KeyboardEvent<HTMLInputElement>) => {
    const empty = !q;
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === "ArrowDown") move(hiIdx + 1);
    else if (e.key === "ArrowUp") move(hiIdx - 1);
    else if (e.key === "PageDown") move(hiIdx + 8);
    else if (e.key === "PageUp") move(hiIdx - 8);
    else if (e.key === "Home" && empty) move(0);
    else if (e.key === "End" && empty) move(shown.length - 1);
    else if (e.key === "Enter") void pick(cur?.item);
    else if ((e.key === "ArrowRight" || e.key === "ArrowLeft") && empty && !pending) {
      const i = rail.indexOf(cat) + (e.key === "ArrowRight" ? 1 : -1);
      setCat(rail[(i + rail.length) % rail.length]);
    } else if (mod && e.key.toLowerCase() === "d") {
      if (cur && cur.item.kind !== "layer") toggleFav(cur.item.id);
    } else return;
    e.preventDefault();
    e.stopPropagation();
  };

  // Always on screen, whatever the window size.
  const [box, setBox] = useState({ left: 8, top: 8, width: MENU_W, height: MENU_H });
  useLayoutEffect(() => {
    const place = () => {
      const width = Math.min(MENU_W, window.innerWidth - 16);
      const height = Math.min(MENU_H, window.innerHeight - 16);
      const x = pos ? pos.x : (window.innerWidth - width) / 2;
      const y = pos ? pos.y : (window.innerHeight - height) / 2;
      setBox({ width, height, left: Math.max(8, Math.min(x, window.innerWidth - width - 8)), top: Math.max(8, Math.min(y, window.innerHeight - height - 8)) });
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [pos]);

  const searching = !!q.trim();
  const count = pending ? "" : searching ? `${shown.length} ${shown.length === 1 ? "result" : "results"}` : "";
  let k = 0;
  const catLabel = (c: string) => c;

  return createPortal(
    <div ref={root} className="toolmenu" role="dialog" aria-label="Add a tool" style={box} onContextMenu={(e) => e.preventDefault()}>
      <div className="tm-head">
        <SearchRegular />
        <input
          ref={input}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={onKey}
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={hiIdx >= 0 ? optId(hiIdx) : undefined}
          aria-label="Search tools and layers"
          placeholder={pending ? "Search layers" : "Search tools and layers"}
          autoComplete="off"
          spellCheck={false}
        />
        <b role="status" aria-live="polite">{count || (pending ? "" : "Shift+A")}</b>
      </div>
      <div className="tm-body">
        <div className="tm-rail" aria-label="Categories" hidden={!!pending}>
          {rail.map((g) => (
            <button key={g} tabIndex={-1} className={!searching && cat === g ? "on" : ""} aria-current={!searching && cat === g} onClick={() => { setCat(g); setQ(""); input.current?.focus(); }}>
              {g === START ? <StarRegular /> : g === LAYERS ? <LayerRegular /> : <i className="tm-chip" style={{ background: familyOf(g) ? `var(--fam-${familyOf(g)})` : "var(--rule)" }} />}
              <span>{catLabel(g)}</span>
              <small>{countOf(g)}</small>
            </button>
          ))}
        </div>
        <div className="tm-main">
          <select className="tm-cat" aria-label="Category" tabIndex={-1} hidden={!!pending} value={cat} onChange={(e) => { setCat(e.target.value); setQ(""); input.current?.focus(); }}>
            {rail.map((g) => (
              <option key={g} value={g}>{g}{countOf(g) !== null ? ` (${countOf(g)})` : ""}</option>
            ))}
          </select>
          {pending && (
            <div className="tm-note">
              Tools go into a layer's node tree. Choose the layer for <b>{pending.name}</b>.
              {chooseItems.length === 0 && (
                <div><button className="tm-btn" onClick={() => { closeToolMenu(); void addData(); }}>Add Data</button></div>
              )}
            </div>
          )}
          <div className="tm-list" id={listId} role="listbox" aria-label={pending ? "Layers" : "Tools"}>
            {sections.map((s, si) =>
              s.rows.length ? (
                <div key={si} role="presentation">
                  {s.title && <div className="tm-sec" role="presentation">{s.title}</div>}
                  {s.rows.map((r) => {
                    const i = k++;
                    const t = r.item;
                    const fv = fav.includes(t.id);
                    return (
                      <div key={t.id + i} id={optId(i)} role="option" data-tool={t.tool} aria-selected={i === hiIdx} className={`tm-row ${i === hiIdx ? "on" : ""}`} onMouseMove={() => i !== hiIdx && setHi(i)} onClick={() => void pick(t)} title={t.description}>
                        {t.kind === "layer" ? <DatabaseRegular /> : <ToolIcon tool={t.tool} />}
                        <span className="tm-text">
                          <span className="tm-name">{t.name}</span>
                          <span className="tm-desc">{t.description}</span>
                        </span>
                        {r.hit ? <span className="tm-hit" title="Matched this search word">{r.hit}</span> : <span />}
                        {t.kind === "layer" ? <span /> : (
                          <button className={`tm-star ${fv ? "fav" : ""}`} tabIndex={-1} aria-label={`${fv ? "Remove" : "Add"} ${t.name} ${fv ? "from" : "to"} favourites`} aria-pressed={fv} onClick={(e) => { e.stopPropagation(); toggleFav(t.id); input.current?.focus(); }}>
                            {fv ? <StarFilled /> : <StarRegular />}
                          </button>
                        )}
                      </div>
                    );
                  })}
                </div>
              ) : null,
            )}
            {shown.length === 0 && <div className="tm-none">{searching ? `No tool matches “${q.trim()}”. Try a plain word, like “cut” or “join”.` : pending ? "There are no layers." : cat === LAYERS ? "No other layers in this project." : "Nothing here."}</div>}
          </div>
          {cur && !pending && <div className="tm-prevline"><Preview item={cur.item} compact /></div>}
        </div>
        <div className="tm-prev" aria-live="off">{cur && !pending && <Preview item={cur.item} />}</div>
      </div>
      <div className="tm-foot">
        <span><kbd>↑↓</kbd>move</span>
        <span><kbd>←→</kbd>category</span>
        <span><kbd>Enter</kbd>add</span>
        <span><kbd>{/Mac/.test(navigator.platform) ? "⌘D" : "Ctrl+D"}</kbd>favourite</span>
        <span><kbd>Esc</kbd>close</span>
      </div>
    </div>,
    document.body,
  );
}

function Preview({ item, compact }: { item: Item; compact?: boolean }) {
  const spec = specsOf()[item.tool];
  const treeLayer = useModel((s) => s.layer);
  useModel((s) => s.nodes); // the connect line follows the selection
  useToolMenu((s) => s.wire);
  if (item.kind === "layer") {
    return compact ? <span><b>Adds a Layer card</b> you can connect to any tool's second input.</span> : (
      <>
        <div><h4>{item.name}</h4><div className="path">{item.description}</div></div>
        <div className="tm-conn"><b>Adds a Layer card</b> you can connect to any tool's second input.</div>
      </>
    );
  }
  const where = treeLayer ? describePlacement(item.tool) : "Choose a layer first.";
  if (compact) return <span><b>{item.name}</b> · {item.kind === "output" ? "Save to File" : item.category.split(SEP).join(" › ")} · {where}</span>;
  const nparams = spec?.params.filter((p) => !p.hidden).length ?? 0;
  return (
    <>
      <div>
        <h4>{item.name}</h4>
        <div className="path">{item.kind === "output" ? "Save to File" : item.category.split(SEP).join(" › ")}</div>
      </div>
      {!!spec?.inputs.length && (
        <div>
          <h5>Inputs</h5>
          {spec.inputs.map((p) => (
            <div className="tm-pv" key={p.id}><span className={`tm-sock ${p.kind}`} />{p.label}<em>{KIND_NAME[p.kind]}</em></div>
          ))}
        </div>
      )}
      {spec?.output && (
        <div>
          <h5>Output</h5>
          <div className="tm-pv"><span className={`tm-sock ${spec.output} filled`} />{KIND_NAME[spec.output]}</div>
        </div>
      )}
      <div>
        <h5>Settings</h5>
        <div className="tm-pv">{nparams ? `${nparams} inside the card` : "None"}</div>
      </div>
      <div className="tm-conn">{where}</div>
    </>
  );
}
