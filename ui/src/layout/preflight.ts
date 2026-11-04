// Pre-flight check before an export: does the page say what the map is drawn in, where the data came from, and do the scale bar and the
// legend belong to a frame that is still there? Pure, so it is unit-tested; the panel (Preflight.tsx) only shows the result.
import { followedMap, type Layout, type LegendElement, type MapFrame, type ScaleBarElement, type SourcesElement } from "./types";

export interface Check {
  id: "frames" | "crs" | "maps" | "sources" | "scale" | "legend";
  /** ok = passes; otherwise the page can still be exported, but the reader may be misled. */
  ok: boolean;
  /** One short sentence: what was checked and what was found. */
  text: string;
}

export interface PreflightContext {
  /** Whether a map of the project still has this id. */
  mapExists: (id: string) => boolean;
  /** A readable name for a CRS id ("" if unknown). */
  crsName: (id: string) => string;
}

const label = (e: { name?: string; id: string }) => (e.name?.trim() ? e.name.trim() : e.id);

export function preflight(layout: Layout, ctx: PreflightContext): Check[] {
  const frames = layout.elements.filter((e): e is MapFrame => e.kind === "map_frame");
  const out: Check[] = [];
  if (!frames.length) {
    return [{ id: "frames", ok: false, text: "The page has no map frame, so there is no map to export. Insert one from the Insert rail." }];
  }
  const noCrs = frames.filter((f) => !f.crs);
  out.push(
    noCrs.length
      ? { id: "crs", ok: false, text: `No coordinate system is set for ${noCrs.map(label).join(", ")}; the export would be refused.` }
      : { id: "crs", ok: true, text: frames.length === 1 ? `The map frame is drawn in ${ctx.crsName(frames[0].crs) || frames[0].crs}.` : `All ${frames.length} map frames name their coordinate system.` },
  );
  const gone = frames.filter((f) => {
    const m = followedMap(f.layers);
    return m !== null && !ctx.mapExists(m);
  });
  if (gone.length) out.push({ id: "maps", ok: false, text: `${gone.map(label).join(", ")} follow${gone.length === 1 ? "s" : ""} a map that was deleted and would draw no layers.` });
  const sources = layout.elements.filter((e): e is SourcesElement => e.kind === "sources" && e.visible);
  const unsourced = frames.filter((f) => f.visible && !sources.some((s) => s.frame === f.id));
  out.push(
    unsourced.length
      ? { id: "sources", ok: false, text: `No sources line for ${unsourced.map(label).join(", ")}. Insert › Sources lists the data it draws.` }
      : { id: "sources", ok: true, text: "Sources are listed for every map frame." },
  );
  const bars = layout.elements.filter((e): e is ScaleBarElement => e.kind === "scale_bar");
  const orphanBars = bars.filter((b) => !frames.some((f) => f.id === b.frame));
  if (bars.length || orphanBars.length) {
    out.push(
      orphanBars.length
        ? { id: "scale", ok: false, text: `${orphanBars.map(label).join(", ")} measure${orphanBars.length === 1 ? "s" : ""} a frame that is not on the page.` }
        : { id: "scale", ok: true, text: "Each scale bar belongs to a map frame on the page, so it is true at that frame's centre." },
    );
  }
  const legends = layout.elements.filter((e): e is LegendElement => e.kind === "legend");
  if (legends.length) {
    const orphans = legends.filter((l) => !frames.some((f) => f.id === l.frame));
    out.push(
      orphans.length
        ? { id: "legend", ok: false, text: `${orphans.map(label).join(", ")} list${orphans.length === 1 ? "s" : ""} the layers of a frame that is not on the page.` }
        : { id: "legend", ok: true, text: "Each legend lists the layers of a map frame on the page." },
    );
  }
  return out;
}

/** True when nothing needs the author's attention. */
export const allClear = (checks: Check[]) => checks.every((c) => c.ok);
