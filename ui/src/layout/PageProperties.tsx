// Properties of the page itself (nothing selected): Sheet, Guides, Info, and the layout's named text styles.
import { Button, Checkbox, Dropdown, Field, Input, Option } from "@fluentui/react-components";
import { useState } from "react";
import { beginStep, useLayouts } from "../store/layout";
import { InspectorTabs } from "./Inspector";
import { Num } from "./ElementProperties";
import { DEFAULT_STYLES, parseStyleSize, restyle, useNamedStyles, usesOf, type NamedStyle } from "./namedStyles";
import { describePage, PAGE_SIZES, pageFor, PAPER, type Layout, type Orientation } from "./types";

/** The id Properties uses for "the page" (a layout element never has this id). */
export const PAGE_TARGET = "__page";

export function PageProperties({ layout }: { layout: string }) {
  const doc = useLayouts((s) => (s.id === layout ? s.doc : s.docs[layout]));
  if (!doc) return <div className="empty-hint pad">This layout no longer exists.</div>;
  return (
    <div className="props">
      <div className="props-title">Page <span className="muted">({doc.name})</span></div>
      <div className="props-body stack">
        <InspectorTabs
          kind="page"
          tabs={[
            { id: "sheet", label: "Sheet", body: <Sheet doc={doc} /> },
            { id: "guides", label: "Guides", body: <Guides doc={doc} /> },
            { id: "styles", label: "Text Styles", body: <TextStyles doc={doc} /> },
            { id: "info", label: "Info", body: <Info doc={doc} /> },
          ]}
        />
      </div>
    </div>
  );
}

function Sheet({ doc }: { doc: Layout }) {
  const page = doc.page;
  const cur = describePage(page);
  const set = (p: Partial<typeof page>) => useLayouts.getState().setPage({ ...page, ...p });
  const size = (name: string, o: Orientation) => useLayouts.getState().setPage({ ...pageFor(name, o, page.margin), background: page.background, bleed: page.bleed, cropMarks: page.cropMarks });
  return (
    <>
      <Field label="Size" size="small">
        <Dropdown size="small" aria-label="Page size" value={cur?.size ?? "Custom"} selectedOptions={cur ? [cur.size] : []} onOptionSelect={(_, d) => d.optionValue && size(d.optionValue, cur?.orientation ?? "landscape")}>
          {Object.keys(PAGE_SIZES).map((n) => <Option key={n} value={n}>{n}</Option>)}
        </Dropdown>
      </Field>
      <Field label="Orientation" size="small">
        <Dropdown size="small" aria-label="Page orientation" value={cur?.orientation ?? (page.width > page.height ? "landscape" : "portrait")} selectedOptions={cur ? [cur.orientation] : []} onOptionSelect={(_, d) => cur && d.optionValue && size(cur.size, d.optionValue as Orientation)}>
          <Option value="landscape">landscape</Option>
          <Option value="portrait">portrait</Option>
        </Dropdown>
      </Field>
      <div className="props-grid">
        <Num label="Margin (mm)" value={page.margin} min={0} max={Math.min(page.width, page.height) / 2 - 1} step={0.5} decimals={1} onChange={(margin) => set({ margin })} />
        <Num label="Bleed (mm)" value={page.bleed ?? 0} min={0} max={10} step={0.5} decimals={1} onChange={(bleed) => set({ bleed })} />
      </div>
      <Checkbox label="Crop marks outside the bleed" checked={!!page.cropMarks} onChange={(_, d) => set({ cropMarks: !!d.checked })} />
      <div className="props-hint">{page.width} × {page.height} mm. Width and height can be set freely in Page Setup (Page tab).</div>
    </>
  );
}

function Guides({ doc }: { doc: Layout }) {
  const guides = doc.guides ?? [];
  const setGuides = (g: typeof guides) => useLayouts.getState().setMeta({ guides: g });
  return (
    <>
      <div className="props-hint">Drag from a ruler onto the page to add a guide; double-click a guide to remove it. Alt while dragging turns snapping off.</div>
      {guides.map((g, i) => (
        <div key={i} className="btn-row">
          <span className="grow">{g.axis === "x" ? "Vertical at x =" : "Horizontal at y ="}</span>
          <Input size="small" style={{ width: 70 }} type="number" aria-label={`Guide ${i + 1} position (mm)`} value={String(g.pos)} onChange={(_, d) => { const v = Number(d.value); if (d.value.trim() !== "" && Number.isFinite(v)) setGuides(guides.map((x, j) => (j === i ? { ...x, pos: v } : x))); }} />
          <Button size="small" appearance="subtle" aria-label={`Remove guide ${i + 1}`} onClick={() => setGuides(guides.filter((_, j) => j !== i))}>✕</Button>
        </div>
      ))}
      {!guides.length && <div className="muted">No guides yet.</div>}
      <div className="btn-row">
        <Button size="small" onClick={() => setGuides([...guides, { axis: "x", pos: Math.round(doc.page.width / 2) }])}>Add vertical</Button>
        <Button size="small" onClick={() => setGuides([...guides, { axis: "y", pos: Math.round(doc.page.height / 2) }])}>Add horizontal</Button>
      </div>
    </>
  );
}

function Info({ doc }: { doc: Layout }) {
  return (
    <>
      <Field label="Author (the {author} text token)" size="small">
        <Input size="small" aria-label="Author" value={doc.author ?? ""} onChange={(_, d) => useLayouts.getState().setMeta({ author: d.value })} />
      </Field>
      <Checkbox label="PNG export also writes a world file (.pgw) and .prj" checked={!!doc.worldFile} onChange={(_, d) => useLayouts.getState().setMeta({ worldFile: !!d.checked })} />
      <Field label="Page background" size="small">
        <input type="color" aria-label="Page background" value={/^#[0-9a-f]{6}$/i.test(doc.page.background) ? doc.page.background : PAPER.white} onChange={(e) => useLayouts.getState().setPage({ ...doc.page, background: e.target.value })} />
      </Field>
    </>
  );
}

/** Add-on 9: change a named style once and every Text element and text slot with that look follows. */
function TextStyles({ doc }: { doc: Layout }) {
  const store = useNamedStyles();
  const styles = store.byLayout[doc.id] ?? store.get(doc.id);
  const [msg, setMsg] = useState("");
  const edit = (i: number, patch: Partial<NamedStyle>) => {
    const from = styles[i];
    const to = { ...from, ...patch };
    const edits = restyle(doc, from, to);
    const s = useLayouts.getState();
    const end = beginStep(); // one undo step for everything this changes
    for (const [id, p] of Object.entries(edits)) s.update(id, p);
    end();
    store.set(doc.id, styles.map((x, j) => (j === i ? to : x)));
    setMsg(Object.keys(edits).length ? `Changed ${Object.keys(edits).length} element${Object.keys(edits).length === 1 ? "" : "s"}.` : "");
  };
  return (
    <>
      <div className="props-hint">A named style is a size, weight and colour. Change it here and everything that has that look changes with it. Apply one to a Text element in its Text tab, or from the bar above it.</div>
      {styles.map((st, i) => {
        const n = usesOf(doc, st).length;
        return (
          <div key={st.name} className="text-slot" data-style={st.name}>
            <div className="btn-row"><b className="grow">{st.name}</b><span className="muted">{n} in use</span></div>
            <div className="props-grid">
              <Field label="Size (pt)" size="small">
                <Input size="small" type="number" aria-label={`${st.name} size`} defaultValue={String(st.size)} key={st.size} onBlur={(e) => { const v = parseStyleSize(e.target.value); if (v === null) e.target.value = String(st.size); else if (v !== st.size) edit(i, { size: v }); }} />
              </Field>
              <Field label="Colour" size="small">
                <input type="color" aria-label={`${st.name} colour`} value={st.color ?? PAPER.black} onChange={(e) => edit(i, { color: e.target.value })} />
              </Field>
            </div>
            <div className="row2">
              <Checkbox label="Bold" checked={!!st.bold} onChange={(_, d) => edit(i, { bold: !!d.checked, italic: d.checked ? false : st.italic })} />
              <Checkbox label="Italic" checked={!!st.italic} onChange={(_, d) => edit(i, { italic: !!d.checked, bold: d.checked ? false : st.bold })} />
            </div>
          </div>
        );
      })}
      <div className="btn-row">
        <Button size="small" appearance="subtle" onClick={() => store.set(doc.id, DEFAULT_STYLES)}>Reset the list</Button>
        <span className="muted" role="status">{msg}</span>
      </div>
    </>
  );
}
