// Page Setup (CG6): size, margin, bleed and crop marks, background, author, and the editor's guides.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Field, Input } from "@fluentui/react-components";
import { useState } from "react";
import type { Guide, Page } from "../layout/types";
import { useLayouts } from "../store/layout";

const num = (s: string): number | null => (s.trim() !== "" && Number.isFinite(Number(s)) ? Number(s) : null);

/** The page with the typed values applied; null with a message when something is not usable. */
export function parsePageSetup(f: { width: string; height: string; margin: string; bleed: string }, page: Page): { page: Page } | { error: string } {
  const [w, h, m, b] = [num(f.width), num(f.height), num(f.margin), num(f.bleed)];
  if (w === null || h === null || w < 20 || h < 20 || w > 5000 || h > 5000) return { error: "Page width and height must be between 20 and 5000 mm." };
  if (m === null || m < 0 || m * 2 >= Math.min(w, h)) return { error: "The margin must be 0 or more and leave room on the page." };
  if (b === null || b < 0 || b > 10) return { error: "Bleed must be between 0 and 10 mm." };
  return { page: { ...page, width: w, height: h, margin: m, bleed: b } };
}

export function PageSetupDialog({ onClose }: { onClose: () => void }) {
  const doc = useLayouts((s) => s.doc);
  const [f, setF] = useState(() => ({ width: String(doc?.page.width ?? ""), height: String(doc?.page.height ?? ""), margin: String(doc?.page.margin ?? 0), bleed: String(doc?.page.bleed ?? 0) }));
  const [marks, setMarks] = useState(!!doc?.page.cropMarks);
  const [bg, setBg] = useState(doc?.page.background ?? "#ffffff");
  const [author, setAuthor] = useState(doc?.author ?? "");
  const [guides, setGuides] = useState<Guide[]>(doc?.guides ?? []);
  if (!doc) return null;
  const parsed = parsePageSetup(f, doc.page);
  const apply = () => {
    if ("error" in parsed) return;
    const s = useLayouts.getState();
    s.setPage({ ...parsed.page, background: bg, cropMarks: marks });
    s.setMeta({ author, guides });
    onClose();
  };
  const field = (k: keyof typeof f, label: string) => (
    <Field label={label} size="small">
      <Input size="small" value={f[k]} onChange={(_, d) => setF((x) => ({ ...x, [k]: d.value }))} />
    </Field>
  );
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Page Setup</DialogTitle>
          <DialogContent className="stack">
            <div className="props-grid">
              {field("width", "Width (mm)")}
              {field("height", "Height (mm)")}
              {field("margin", "Margin (mm)")}
              {field("bleed", "Bleed (mm)")}
            </div>
            <Checkbox label="Crop marks outside the bleed" checked={marks} onChange={(_, d) => setMarks(!!d.checked)} />
            <Field label="Page background" size="small">
              <input type="color" value={/^#[0-9a-f]{6}$/i.test(bg) ? bg : "#ffffff"} onChange={(e) => setBg(e.target.value)} aria-label="Page background" />
            </Field>
            <Field label="Author (the {author} text token)" size="small">
              <Input size="small" value={author} onChange={(_, d) => setAuthor(d.value)} />
            </Field>
            <Field label={`Guides (${guides.length}) — drag from a ruler to add; Alt while dragging turns snapping off`} size="small">
              <div className="stack">
                {guides.map((g, i) => (
                  <div key={i} className="class-row">
                    <span>{g.axis === "x" ? "Vertical at x =" : "Horizontal at y ="}</span>
                    <Input size="small" style={{ width: 80 }} value={String(g.pos)} aria-label={`Guide ${i + 1} position`} onChange={(_, d) => num(d.value) !== null && setGuides(guides.map((x, j) => (j === i ? { ...x, pos: num(d.value)! } : x)))} />
                    <Button size="small" appearance="subtle" aria-label={`Remove guide ${i + 1}`} onClick={() => setGuides(guides.filter((_, j) => j !== i))}>✕</Button>
                  </div>
                ))}
                <div className="class-row">
                  <Button size="small" onClick={() => setGuides([...guides, { axis: "x", pos: Math.round((num(f.width) ?? 100) / 2) }])}>Add vertical</Button>
                  <Button size="small" onClick={() => setGuides([...guides, { axis: "y", pos: Math.round((num(f.height) ?? 100) / 2) }])}>Add horizontal</Button>
                </div>
              </div>
            </Field>
            {"error" in parsed && <div className="field-error">{parsed.error}</div>}
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" disabled={"error" in parsed} onClick={apply}>OK</Button>
            <Button onClick={onClose}>Cancel</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
