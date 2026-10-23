// Sources pane: every data source the project has used, as an annotated
// bibliography (APA-style reference, annotation, layers it was used for,
// license). Local files start incomplete and can be edited; the list exports
// to Markdown or BibTeX.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Field, Input, Menu, MenuItem, MenuList, MenuPopover, MenuTrigger, Textarea, Tooltip } from "@fluentui/react-components";
import { ArrowClockwiseRegular, ArrowDownloadRegular, CopyRegular, DeleteRegular, EditRegular, WarningRegular } from "@fluentui/react-icons";
import { save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, IN_TAURI, type Citation, type SourceEntry } from "../ipc/api";
import { reference, referenceText, toBibtex, toMarkdown } from "../sources/cite";
import { useApp } from "../store/store";

const projectName = () => useApp.getState().projectDir?.split(/[\\/]/).pop()?.replace(/\.gproj$/, "") ?? "Untitled project";

async function saveTextFile(text: string, name: string, ext: string, label: string) {
  if (IN_TAURI) {
    const path = await save({ title: `Export ${label}`, defaultPath: name, filters: [{ name: label, extensions: [ext] }] });
    if (!path) return;
    await api.saveText(path, text);
    useApp.getState().setStatus(`Saved ${label.toLowerCase()} to ${path}`);
  } else {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
    a.download = name;
    a.click();
    URL.revokeObjectURL(a.href);
  }
}

export function SourcesPane() {
  const [entries, setEntries] = useState<SourceEntry[]>([]);
  const [editing, setEditing] = useState<Citation | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // Refresh when layers come and go or are renamed.
  const layerKey = useApp(useShallow((s) => Object.values(s.layers).map((l) => `${l.id}:${l.name}`).join("|")));
  const basemap = useApp((s) => s.basemap);
  const present = useApp(useShallow((s) => Object.keys(s.layers)));

  const refresh = useCallback(() => {
    api.sources().then(setEntries).catch((e) => setErr(errorText(e)));
  }, []);
  useEffect(() => {
    // Give a just-started import a moment to log its sources.
    const t = setTimeout(refresh, 300);
    return () => clearTimeout(t);
  }, [layerKey, basemap, refresh]);

  const incomplete = entries.filter((e) => !e.citation.complete).length;
  const copyAll = async () => {
    const text = entries.map((e) => `${referenceText(e.citation, e.firstUsed)}\n    ${e.citation.annotation}`).join("\n\n");
    await navigator.clipboard.writeText(text);
    useApp.getState().setStatus(`Copied ${entries.length} citation(s)`);
  };
  const slug = projectName().replace(/[^\w-]+/g, "_");

  return (
    <div className="pane sources-pane">
      <div className="pane-toolbar">
        <Tooltip content="Copy all citations" relationship="label">
          <Button size="small" appearance="subtle" icon={<CopyRegular />} disabled={!entries.length} onClick={copyAll} />
        </Tooltip>
        <Menu>
          <MenuTrigger>
            <Button size="small" appearance="subtle" icon={<ArrowDownloadRegular />} disabled={!entries.length}>
              Export
            </Button>
          </MenuTrigger>
          <MenuPopover>
            <MenuList>
              <MenuItem onClick={() => saveTextFile(toMarkdown(entries, projectName()), `${slug}_sources.md`, "md", "Annotated bibliography")}>Annotated bibliography (Markdown)…</MenuItem>
              <MenuItem onClick={() => saveTextFile(toBibtex(entries), `${slug}_sources.bib`, "bib", "BibTeX")}>BibTeX (.bib)…</MenuItem>
            </MenuList>
          </MenuPopover>
        </Menu>
        <Tooltip content="Refresh" relationship="label">
          <Button size="small" appearance="subtle" icon={<ArrowClockwiseRegular />} onClick={refresh} />
        </Tooltip>
        <span className="muted small" style={{ marginLeft: "auto" }}>
          {entries.length} source{entries.length === 1 ? "" : "s"}
        </span>
      </div>
      {incomplete > 0 && (
        <div className="src-warn">
          <WarningRegular /> {incomplete} local file{incomplete === 1 ? " needs" : "s need"} a citation — click Edit to add the author, year and origin.
        </div>
      )}
      {err && <div className="src-warn">{err}</div>}
      {entries.length === 0 && <div className="muted pane-empty">Data you add — files, TIGER, National Map, elevation, basemaps — is listed here with a citation, ready to export as an annotated bibliography.</div>}
      <div className="src-list">
        {entries.map((e) => {
          const r = reference(e.citation, e.firstUsed);
          const names = [...new Map(e.uses.map((u) => [u.layerName, u])).values()];
          return (
            <div key={e.citation.key} className={`src-entry ${e.citation.complete ? "" : "incomplete"}`}>
              <div className="src-ref">
                {r.before}
                <i>{r.title}</i>
                {r.after}
              </div>
              {e.citation.annotation && <div className="src-note">{e.citation.annotation}</div>}
              {names.length > 0 && (
                <div className="src-uses">
                  Used for:{" "}
                  {names.map((u, i) => (
                    <span key={u.layerName} className={!u.layerId || present.includes(u.layerId) ? "" : "removed"} title={!u.layerId || present.includes(u.layerId) ? "" : "Layer since removed"}>
                      {u.layerName}
                      {i < names.length - 1 ? "; " : ""}
                    </span>
                  ))}
                </div>
              )}
              {e.citation.license && <div className="src-license">License: {e.citation.license}</div>}
              <div className="src-actions">
                <span className="src-kind">{e.citation.kind}</span>
                <Button size="small" appearance="subtle" icon={<CopyRegular />} onClick={() => navigator.clipboard.writeText(referenceText(e.citation, e.firstUsed))}>
                  Copy
                </Button>
                <Button size="small" appearance="subtle" icon={<EditRegular />} onClick={() => setEditing(e.citation)}>
                  Edit
                </Button>
                <Button
                  size="small"
                  appearance="subtle"
                  icon={<DeleteRegular />}
                  title="Remove from the bibliography"
                  onClick={() => api.removeSource(e.citation.key).then(refresh, (x) => setErr(errorText(x)))}
                />
              </div>
            </div>
          );
        })}
      </div>
      {editing && (
        <CitationDialog
          citation={editing}
          onClose={() => setEditing(null)}
          onSave={async (c) => {
            await api.updateSource(c);
            setEditing(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

function CitationDialog({ citation, onClose, onSave }: { citation: Citation; onClose: () => void; onSave: (c: Citation) => Promise<void> }) {
  const [c, setC] = useState(citation);
  const [err, setErr] = useState<string | null>(null);
  const field = (k: keyof Citation, label: string, hint?: string) => (
    <Field label={label} hint={hint}>
      <Input value={String(c[k])} onChange={(_, d) => setC({ ...c, [k]: d.value })} />
    </Field>
  );
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ maxWidth: 560 }}>
        <DialogBody>
          <DialogTitle>Edit citation</DialogTitle>
          <DialogContent className="stack">
            {field("author", "Author or organization", "e.g. Oklahoma Department of Transportation")}
            <div className="row2">
              {field("year", "Year", "Publication year or vintage; n.d. if none")}
              {field("version", "Version or edition")}
            </div>
            {field("title", "Title")}
            {field("publisher", "Publisher")}
            {field("url", "Web address (where you got it)")}
            {field("license", "License or terms of use")}
            <Field label="Annotation" hint="What the data is and how you used it">
              <Textarea rows={3} value={c.annotation} onChange={(_, d) => setC({ ...c, annotation: d.value })} />
            </Field>
            <div className="muted small">Preview: {referenceText(c, new Date().toISOString().slice(0, 10))}</div>
            {err && <div className="src-warn">{err}</div>}
          </DialogContent>
          <DialogActions>
            <Button onClick={onClose}>Cancel</Button>
            <Button appearance="primary" onClick={() => onSave(c).catch((e) => setErr(errorText(e)))}>
              Save
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
