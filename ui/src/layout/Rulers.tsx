// Rulers for the Layout view (C3.0): millimetres from the page's top-left corner, along the top and left edges of the canvas.

/** Major and minor tick spacing (mm) for a zoom; majors are at least 60 px apart. */
const STEPS: [major: number, minor: number][] = [[1, 0.5], [2, 1], [5, 1], [10, 2], [20, 5], [50, 10], [100, 20], [200, 50], [500, 100]];

export interface Tick {
  mm: number;
  major: boolean;
}

/** Ticks between `fromMm` and `toMm` for `zoom` px per mm. */
export function rulerTicks(fromMm: number, toMm: number, zoom: number): Tick[] {
  if (!(zoom > 0) || !Number.isFinite(fromMm) || !Number.isFinite(toMm) || toMm < fromMm) return [];
  const [major, minor] = STEPS.find(([m]) => m * zoom >= 60) ?? STEPS[STEPS.length - 1];
  const out: Tick[] = [];
  const first = Math.ceil(fromMm / minor - 1e-9);
  const last = Math.floor(toMm / minor + 1e-9);
  for (let i = first; i <= last && out.length < 2000; i++) {
    const mm = Math.round(i * minor * 1000) / 1000 || 0; // `|| 0` turns -0 into 0
    out.push({ mm, major: Math.abs(mm / major - Math.round(mm / major)) < 1e-9 });
  }
  return out;
}

export const RULER_PX = 18;

/**
 * One ruler. `origin` is where the page's 0 sits along the ruler, in px from the ruler's start; `length` is the visible
 * length in px.
 */
export function Ruler({ vertical, length, origin, zoom }: { vertical: boolean; length: number; origin: number; zoom: number }) {
  const ticks = rulerTicks(-origin / zoom, (length - origin) / zoom, zoom);
  const at = (mm: number) => origin + mm * zoom;
  return (
    <svg className={`ruler ${vertical ? "v" : "h"}`} width={vertical ? RULER_PX : length} height={vertical ? length : RULER_PX} aria-hidden>
      {ticks.map((t) => {
        const p = at(t.mm);
        const len = t.major ? RULER_PX * 0.7 : RULER_PX * 0.35;
        return (
          <g key={t.mm}>
            {vertical ? <line x1={RULER_PX} x2={RULER_PX - len} y1={p} y2={p} /> : <line x1={p} x2={p} y1={RULER_PX} y2={RULER_PX - len} />}
            {t.major &&
              (vertical ? (
                <text x={2} y={p + 3} transform={`rotate(-90 ${2} ${p + 3})`} fontSize="8">{t.mm}</text>
              ) : (
                <text x={p + 2} y={9} fontSize="8">{t.mm}</text>
              ))}
          </g>
        );
      })}
    </svg>
  );
}
