// Bibliography formatting for the project's data sources: references in APA 7,
// MLA 9 and Chicago 17 (author-date), annotated bibliographies (Markdown and
// HTML), BibTeX and RIS. Every reference carries the date the source was
// accessed, since web data services change over time.
import type { Citation, SourceEntry } from "../ipc/api";

export type Style = "apa" | "mla" | "chicago";

export const STYLES: { id: Style; label: string; file: string }[] = [
  { id: "apa", label: "APA 7th edition", file: "APA" },
  { id: "mla", label: "MLA 9th edition", file: "MLA" },
  { id: "chicago", label: "Chicago 17th (author-date)", file: "Chicago" },
];

const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const MLA_MONTHS = ["Jan.", "Feb.", "Mar.", "Apr.", "May", "June", "July", "Aug.", "Sept.", "Oct.", "Nov.", "Dec."];

const parts = (iso: string) => iso.split("-").map(Number) as [number, number, number];

/** "October 22, 2026" (APA, Chicago). */
export function usDate(iso: string): string {
  const [y, m, d] = parts(iso);
  return y && m && d ? `${MONTHS[m - 1]} ${d}, ${y}` : iso;
}

/** "22 Oct. 2026" (MLA). */
export function mlaDate(iso: string): string {
  const [y, m, d] = parts(iso);
  return y && m && d ? `${d} ${MLA_MONTHS[m - 1]} ${y}` : iso;
}

/** "22 October 2026" (used in plain notes). */
export function longDate(iso: string): string {
  const [y, m, d] = parts(iso);
  return y && m && d ? `${d} ${MONTHS[m - 1]} ${y}` : iso;
}

const end = (s: string) => (/[.?!]$/.test(s.trim()) ? s.trim() : `${s.trim()}.`);
const noYear = (y: string) => !y.trim() || /^n\.?\s?d\.?$/i.test(y.trim());
const bare = (url: string) => url.replace(/^https?:\/\//, "").replace(/\/$/, "");

/** A reference split around its italic title, so callers can style it. */
export interface Ref {
  before: string;
  title: string;
  after: string;
}

/** Format one citation in a style. `accessed` is an ISO date. */
export function reference(c: Citation, accessed: string, style: Style = "apa"): Ref {
  const author = c.author.trim();
  const title = c.title.trim() || "[Untitled]";
  const version = c.version.trim();
  const publisher = c.publisher.trim();
  const url = c.url.trim();
  const local = c.kind === "Local file";
  const samePub = !!publisher && publisher === author;
  switch (style) {
    case "apa": {
      // Author. (Year). *Title* (Version) [Data set]. Publisher. Retrieved Month D, YYYY, from URL
      const year = `(${noYear(c.year) ? "n.d." : c.year.trim()}).`;
      const desc = `${version ? ` (${version})` : ""} [${local ? "Unpublished data set" : "Data set"}].`;
      let rest = "";
      if (publisher && !samePub) rest += ` ${end(publisher)}`;
      if (url) rest += ` Retrieved ${usDate(accessed)}, from ${url}`;
      // With no author, the title takes the author's place: *Title* [Data set]. (Year).
      return author ? { before: `${end(author)} ${year} `, title, after: desc + rest } : { before: "", title, after: `${desc} ${year}${rest}` };
    }
    case "mla": {
      // Author. *Title*. Version, Publisher, Year, URL. Accessed D Mon. YYYY.
      // MLA: skip the author when the organization is also the publisher.
      const who = author && !samePub ? `${end(author)} ` : "";
      const bits = [version, publisher, noYear(c.year) ? "" : c.year.trim(), url ? bare(url) : ""].filter(Boolean);
      let after = bits.length ? `. ${bits.join(", ")}.` : ".";
      after += local && !url ? ` Unpublished data set, used ${mlaDate(accessed)}.` : ` Accessed ${mlaDate(accessed)}.`;
      return { before: who, title, after };
    }
    case "chicago": {
      // Author. Year. *Title*. Version. Publisher. Accessed Month D, YYYY. URL.
      const year = noYear(c.year) ? "n.d." : c.year.trim();
      let rest = "";
      if (version) rest += ` ${end(version)}`;
      if (publisher && !samePub) rest += ` ${end(publisher)}`;
      rest += ` Accessed ${usDate(accessed)}.`;
      if (url) rest += ` ${end(url)}`;
      // With no author, the title leads and the year follows it.
      return author ? { before: `${end(author)} ${end(year)} `, title, after: `.${rest}` } : { before: "", title, after: `. ${end(year)}${rest}` };
    }
  }
}

export function referenceText(c: Citation, accessed: string, style: Style = "apa"): string {
  const r = reference(c, accessed, style);
  return `${r.before}${r.title}${r.after}`;
}

function usedFor(e: SourceEntry): string {
  const names = [...new Set(e.uses.map((u) => u.layerName))];
  return names.length ? `Used for: ${names.join("; ")}.` : "";
}

/** The annotation paragraph: what it is and how it was used, plus license. */
export function annotation(e: SourceEntry): string {
  return [e.citation.annotation.trim(), usedFor(e), e.citation.license.trim() ? `License: ${e.citation.license.trim()}` : ""].filter(Boolean).join(" ");
}

/** Entries in the style's alphabetical order (by what each reference starts with). */
export function sorted(entries: SourceEntry[], style: Style): SourceEntry[] {
  const key = (e: SourceEntry) => {
    const r = reference(e.citation, e.firstUsed, style);
    return (r.before || r.title).replace(/^(the|a|an)\s+/i, "").toLowerCase();
  };
  // Incomplete entries (no author yet) go last, so they're easy to find and fix.
  return [...entries].sort((a, b) => Number(!a.citation.author) - Number(!b.citation.author) || key(a).localeCompare(key(b)));
}

const today = () => new Date().toISOString().slice(0, 10);

/** Annotated bibliography in Markdown. */
export function toMarkdown(entries: SourceEntry[], project: string, style: Style = "apa"): string {
  const label = STYLES.find((s) => s.id === style)!.label;
  const lines = [`# Data sources — ${project}`, "", `Annotated bibliography (${label}), generated by GIS Studio on ${longDate(today())}.`, ""];
  for (const e of sorted(entries, style)) {
    const r = reference(e.citation, e.firstUsed, style);
    lines.push(`${r.before}*${r.title}*${r.after}`, "");
    const note = annotation(e);
    if (note) lines.push(`> ${note}`, "");
  }
  return lines.join("\n");
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Annotated bibliography as a standalone HTML page (opens in Word/Docs with italics and hanging indents). */
export function toHtml(entries: SourceEntry[], project: string, style: Style = "apa"): string {
  const label = STYLES.find((s) => s.id === style)!.label;
  const items = sorted(entries, style)
    .map((e) => {
      const r = reference(e.citation, e.firstUsed, style);
      const note = annotation(e);
      return `<p class="ref">${esc(r.before)}<i>${esc(r.title)}</i>${esc(r.after)}</p>${note ? `\n<p class="note">${esc(note)}</p>` : ""}`;
    })
    .join("\n");
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>Data sources — ${esc(project)}</title>
<style>
body { font-family: "Times New Roman", Times, serif; font-size: 12pt; line-height: 2; max-width: 6.5in; margin: 1in auto; }
h1 { font-size: 12pt; text-align: center; font-weight: bold; }
.ref { padding-left: 0.5in; text-indent: -0.5in; margin: 0; }
.note { margin: 0 0 1em 0.5in; }
.meta { font-size: 10pt; color: #555; text-align: center; }
</style></head><body>
<h1>Annotated Bibliography: Data Sources</h1>
<p class="meta">${esc(project)} · ${esc(label)} · generated by GIS Studio on ${esc(longDate(today()))}</p>
${items}
</body></html>
`;
}

const bibEscape = (s: string) => s.replace(/([{}&%$#_])/g, "\\$1");

function bibKeys(entries: SourceEntry[]): string[] {
  const used = new Set<string>();
  return entries.map((e) => {
    const c = e.citation;
    let key = (c.key.startsWith("file:") ? `file-${c.title}` : c.key).replace(/[^A-Za-z0-9-]+/g, "-").replace(/^-|-$/g, "").toLowerCase() || "source";
    for (let i = 2; used.has(key); i++) key = `${key.replace(/-\d+$/, "")}-${i}`;
    used.add(key);
    return key;
  });
}

/** BibTeX @misc entries (biblatex-friendly `urldate`). */
export function toBibtex(entries: SourceEntry[]): string {
  const keys = bibKeys(entries);
  return entries
    .map((e, i) => {
      const c = e.citation;
      const fields: [string, string][] = [
        ["author", c.author ? `{${bibEscape(c.author)}}` : ""],
        ["title", bibEscape(c.title)],
        ["year", noYear(c.year) ? "" : c.year],
        ["edition", bibEscape(c.version)],
        ["publisher", bibEscape(c.publisher)],
        ["howpublished", c.url ? `\\url{${c.url}}` : ""],
        ["url", c.url],
        ["urldate", e.firstUsed],
        ["note", bibEscape(annotation(e))],
      ];
      const body = fields.filter(([, v]) => v).map(([k, v]) => `  ${k} = {${v}}`).join(",\n");
      return `@misc{${keys[i]},\n${body}\n}`;
    })
    .join("\n\n");
}

/** RIS records (Zotero, EndNote, Mendeley). */
export function toRis(entries: SourceEntry[]): string {
  return entries
    .map((e) => {
      const c = e.citation;
      const lines = [
        "TY  - DATA",
        c.author && `AU  - ${c.author}`,
        `TI  - ${c.title}`,
        !noYear(c.year) && `PY  - ${c.year}`,
        c.version && `ET  - ${c.version}`,
        c.publisher && `PB  - ${c.publisher}`,
        c.url && `UR  - ${c.url}`,
        `Y2  - ${e.firstUsed.replace(/-/g, "/")}`,
        annotation(e) && `N1  - ${annotation(e)}`,
        "ER  - ",
      ];
      return lines.filter(Boolean).join("\r\n");
    })
    .join("\r\n\r\n");
}

/** Every bibliography file for a project: annotated bibliographies in each style, plus BibTeX and RIS. */
export function bibliographyFiles(entries: SourceEntry[], project: string): Record<string, string> {
  const files: Record<string, string> = {};
  for (const s of STYLES) {
    files[`Data sources (${s.file}).md`] = toMarkdown(entries, project, s.id);
    files[`Data sources (${s.file}).html`] = toHtml(entries, project, s.id);
  }
  files["Data sources.bib"] = toBibtex(entries);
  files["Data sources.ris"] = toRis(entries);
  return files;
}
