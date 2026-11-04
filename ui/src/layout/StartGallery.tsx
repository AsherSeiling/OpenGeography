// Add-on 1: the start gallery shown on an empty layout. Each start adds ordinary elements to this page (everything stays editable);
// the two sheet starts open the Topo Map builder, which makes a new layout of its own.
import { useDialog } from "../actions";
import { addMapFrame, addSurround, addText, beginStep, useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { insertOverview, insertSurround, setFrameFromMapView } from "./actions";
import { useLayoutView } from "./view";
import { frames } from "./types";

type Box = [number, number, number, number];
interface Start {
  id: string;
  name: string;
  what: string;
  /** Thumbnail: map boxes and text lines as percentages of the thumbnail. */
  maps: Box[];
  lines: [number, number, number][];
  inset?: Box;
  run: () => void | Promise<void>;
}

/** Page geometry a frame starts with, the same as Insert › Map Frame uses. */
function frameBox() {
  const p = useLayouts.getState().doc!.page;
  const y = Math.min(30, p.height / 4);
  return { y, h: p.height - y - p.margin - 9.5, m: p.margin, w: p.width };
}

function thematic() {
  const end = beginStep();
  addText("{layout_name}");
  const f = addMapFrame();
  if (f) {
    for (const k of ["legend", "scale_bar", "north_arrow"] as const) insertSurround(k);
    void setFrameFromMapView(f);
  }
  end();
}

function twoMaps() {
  const maps = useApp.getState().maps;
  if (!maps.length) return void useApp.getState().setStatus("This project has no map yet. Add a map (Insert › New Map) first.");
  const { y, h, m, w } = frameBox();
  const gap = 6;
  const fw = (w - 2 * m - gap) / 2;
  const end = beginStep();
  addText("{layout_name}");
  const ids = [0, 1].map((i) => addMapFrame({ x: m + i * (fw + gap), y, w: fw, h, layers: { map: (maps[i] ?? maps[0]).id }, name: (maps[i] ?? maps[0]).name }));
  end();
  for (const id of ids) if (id) void setFrameFromMapView(id);
  const s = useLayouts.getState();
  if (s.doc && frames(s.doc).length) s.select([]);
}

function inset() {
  const end = beginStep();
  const f = addMapFrame();
  if (f) {
    addSurround("scale_bar", f);
    insertOverview();
    void setFrameFromMapView(f);
  }
  end();
}

const topo = () => useDialog.getState().show({ kind: "topoMap" });

export const STARTS: Start[] = [
  { id: "thematic", name: "Thematic map", what: "Title, map, legend, scale bar, north arrow, sources", maps: [[6, 18, 62, 74]], lines: [[6, 8, 50], [72, 20, 20], [72, 30, 16], [72, 40, 18]], run: thematic },
  { id: "landnav", name: "Land navigation", what: "Opens the Topo Map builder: UTM and MGRS grid, declination, coordinate scales", maps: [[5, 6, 90, 70]], lines: [[10, 82, 24], [40, 82, 24], [70, 82, 20]], run: topo },
  { id: "quad", name: "USGS quadrangle", what: "Opens the Topo Map builder: 1:24,000 collar, choose the USGS preset", maps: [[10, 8, 80, 72]], lines: [[10, 86, 30], [50, 86, 40]], run: topo },
  { id: "two", name: "Two maps side by side", what: "Compare two of the project's maps", maps: [[5, 18, 43, 72], [52, 18, 43, 72]], lines: [[5, 8, 60]], run: twoMaps },
  { id: "inset", name: "Map with inset", what: "Main map plus a small locator map", maps: [[5, 6, 90, 88]], lines: [], inset: [68, 60, 24, 30], run: inset },
  { id: "blank", name: "Blank page", what: "Start empty. Shift+A adds an element", maps: [], lines: [[40, 50, 20]], run: () => {} },
];

export function StartGallery({ layoutId }: { layoutId: string }) {
  const hasCrs = useApp((s) => !!s.mapCrs);
  const pick = (s: Start) => {
    useLayoutView.getState().closeGallery(layoutId);
    void s.run();
  };
  return (
    <div className="start-gallery" role="dialog" aria-label="Start this layout" data-testid="start-gallery" onPointerDown={(e) => e.stopPropagation()} onKeyDown={(e) => { if (e.key === "Escape") useLayoutView.getState().closeGallery(layoutId); }}>
      <div className="gal">
        <h2>Start this layout</h2>
        <p>Each start uses your layers and your project's sources, and everything stays editable.{!hasCrs && " Set the map's coordinate system first (Map › Coordinate System); the map frame needs one."}</p>
        {STARTS.map((s, i) => (
          <button key={s.id} className="tile" data-start={s.id} autoFocus={i === 0} onClick={() => pick(s)}>
            <span className="thumb" aria-hidden>
              {s.maps.map((b, j) => <i key={j} style={{ left: `${b[0]}%`, top: `${b[1]}%`, width: `${b[2]}%`, height: `${b[3]}%` }} />)}
              {s.inset && <i className="inset" style={{ left: `${s.inset[0]}%`, top: `${s.inset[1]}%`, width: `${s.inset[2]}%`, height: `${s.inset[3]}%` }} />}
              {s.lines.map((l, j) => <u key={j} style={{ left: `${l[0]}%`, top: `${l[1]}%`, width: `${l[2]}%` }} />)}
            </span>
            <b>{s.name}</b>
            <small>{s.what}</small>
          </button>
        ))}
      </div>
    </div>
  );
}
