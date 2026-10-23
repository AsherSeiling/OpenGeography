// Geodesic measurement and military land-navigation math.
//
// - Distances, bearings and areas on the WGS 84 ellipsoid with GeographicLib
//   (Karney 2013), accurate to nanometers.
// - Magnetic declination from the World Magnetic Model (WMM2025).
// - Grid azimuths on the UTM/MGRS grid, G-M angle, mils (6400 per circle).
// - Intersection and resection solved on the UTM grid, as on a military map.

import { Geodesic } from "geographiclib-geodesic";
import geomagnetism from "geomagnetism";
import { forward as toMgrsRaw, toPoint as fromMgrs } from "mgrs";
import proj4 from "proj4";

const WGS84 = Geodesic.WGS84;

export type LonLat = [number, number];

// ---------- units ----------

export const LENGTH_UNITS: Record<string, [string, number]> = {
  m: ["Meters", 1],
  km: ["Kilometers", 1000],
  ft: ["Feet", 0.3048],
  usft: ["US survey feet", 1200 / 3937],
  mi: ["Miles", 1609.344],
  nmi: ["Nautical miles", 1852],
  yd: ["Yards", 0.9144],
};

export const AREA_UNITS: Record<string, [string, number]> = {
  sqm: ["Square meters", 1],
  ha: ["Hectares", 1e4],
  sqkm: ["Square kilometers", 1e6],
  acres: ["Acres", 4046.8564224],
  sqmi: ["Square miles", 2589988.110336],
  sqft: ["Square feet", 0.09290304],
};

export const MILS_PER_CIRCLE = 6400; // NATO mils

export const degToMils = (d: number) => (norm360(d) * MILS_PER_CIRCLE) / 360;
export const milsToDeg = (m: number) => norm360((m * 360) / MILS_PER_CIRCLE);
export const norm360 = (a: number) => ((a % 360) + 360) % 360;
export const norm180 = (a: number) => ((((a + 180) % 360) + 360) % 360) - 180;

// ---------- geodesics ----------

/** Geodesic distance (m) and forward/back azimuths (true, degrees) between two points. */
export function inverse(a: LonLat, b: LonLat): { distance: number; azimuth: number; backAzimuth: number } {
  const r = WGS84.Inverse(a[1], a[0], b[1], b[0]);
  return { distance: r.s12!, azimuth: norm360(r.azi1!), backAzimuth: norm360(r.azi2! + 180) };
}

/** Point reached going `distance` m from `a` on true azimuth `azimuth`. */
export function direct(a: LonLat, azimuth: number, distance: number): LonLat {
  const r = WGS84.Direct(a[1], a[0], azimuth, distance);
  return [r.lon2!, r.lat2!];
}

/** Total geodesic length (m) of a path. */
export function pathLength(pts: LonLat[]): number {
  let s = 0;
  for (let i = 1; i < pts.length; i++) s += inverse(pts[i - 1], pts[i]).distance;
  return s;
}

/** Geodesic area (m², always positive) and perimeter (m) of a polygon ring. */
export function polygonArea(pts: LonLat[]): { area: number; perimeter: number } {
  if (pts.length < 3) return { area: 0, perimeter: pathLength(pts) };
  const p = WGS84.Polygon(false);
  for (const [lon, lat] of pts) p.AddPoint(lat, lon);
  const r = p.Compute(false, true);
  return { area: Math.abs(r.area!), perimeter: r.perimeter };
}

/** Points along the geodesic from a to b (for drawing true great-ellipse lines). */
export function geodesicLine(a: LonLat, b: LonLat, steps = 64): LonLat[] {
  const { distance, azimuth } = inverse(a, b);
  if (distance < 1) return [a, b];
  const n = Math.max(2, Math.min(steps, Math.ceil(distance / 2000)));
  return Array.from({ length: n + 1 }, (_, i) => direct(a, azimuth, (distance * i) / n));
}

// ---------- coordinates ----------

export function toMgrs(p: LonLat, digits: 1 | 2 | 3 | 4 | 5 = 5): string {
  const m = toMgrsRaw(p, digits);
  const r = m.match(/^(\d{1,2}[A-Z])([A-Z]{2})(\d+)$/);
  if (!r) return m;
  const half = r[3].length / 2;
  return `${r[1]} ${r[2]} ${r[3].slice(0, half)} ${r[3].slice(half)}`;
}

/** Parse an MGRS grid reference ("14S PF 12345 67890") or "lat, lon" decimal degrees. */
export function parsePosition(text: string): LonLat {
  const t = text.trim();
  const dd = t.match(/^(-?\d+(?:\.\d+)?)\s*[, ]\s*(-?\d+(?:\.\d+)?)$/);
  if (dd) {
    const lat = Number(dd[1]);
    const lon = Number(dd[2]);
    if (Math.abs(lat) > 90 || Math.abs(lon) > 180) throw new Error("Latitude must be −90…90 and longitude −180…180");
    return [lon, lat];
  }
  const compact = t.replace(/\s+/g, "").toUpperCase();
  if (!/^\d{1,2}[C-X][A-Z]{2}(\d{2}|\d{4}|\d{6}|\d{8}|\d{10})$/.test(compact)) {
    throw new Error('Enter an MGRS grid reference (e.g. "14S PF 12345 67890") or "lat, lon"');
  }
  const [lon, lat] = fromMgrs(compact);
  return [lon, lat];
}

export function utmZone(p: LonLat): { zone: number; south: boolean; proj: string } {
  const zone = Math.min(60, Math.max(1, Math.floor((p[0] + 180) / 6) + 1));
  const south = p[1] < 0;
  return { zone, south, proj: `+proj=utm +zone=${zone}${south ? " +south" : ""} +datum=WGS84 +units=m +no_defs` };
}

export const toUtm = (p: LonLat, proj: string): [number, number] => proj4("EPSG:4326", proj, p) as [number, number];
export const fromUtm = (xy: [number, number], proj: string): LonLat => proj4(proj, "EPSG:4326", xy) as LonLat;

// ---------- north references ----------

/** Magnetic declination (degrees, east positive) from WMM2025. */
export function declination(p: LonLat, date = new Date()): number {
  return geomagnetism.model(date).point([p[1], p[0]]).decl;
}

/** Grid azimuth of true north at a point on its UTM grid (degrees, east positive). */
export function gridConvergence(p: LonLat, proj = utmZone(p).proj): number {
  const a = toUtm(p, proj);
  const b = toUtm([p[0], Math.min(p[1] + 0.001, 89.999)], proj);
  return norm180((Math.atan2(b[0] - a[0], b[1] - a[1]) * 180) / Math.PI);
}

/** G-M angle: grid azimuth of magnetic north (degrees, east positive). */
export function gmAngle(p: LonLat, date = new Date()): number {
  return norm180(declination(p, date) + gridConvergence(p));
}

export type North = "grid" | "magnetic" | "true";

/** Convert an azimuth between north references at point p. */
export function convertAzimuth(az: number, from: North, to: North, p: LonLat, gm?: number): number {
  const conv = gridConvergence(p);
  const gmA = gm ?? gmAngle(p);
  const toGrid = from === "grid" ? az : from === "magnetic" ? az + gmA : az + conv;
  return norm360(to === "grid" ? toGrid : to === "magnetic" ? toGrid - gmA : toGrid - conv);
}

// ---------- intersection / resection ----------

export interface Sighting {
  /** Known position. */
  point: LonLat;
  /** Azimuth in degrees, measured with `north` reference. */
  azimuth: number;
  north: North;
}

export interface Fix {
  position: LonLat;
  mgrs: string;
  /** Pairwise intersections of the rays (the "triangle of error" for 3 sightings). */
  intersections: LonLat[];
  /** Largest distance (m) between pairwise intersections (0 for two sightings). */
  errorSpread: number;
  /** Rays used, as [start, end] lines on the map. */
  rays: [LonLat, LonLat][];
}

function lineIntersect(p: [number, number], a: number, q: [number, number], b: number): [number, number] | null {
  // Lines p + t·(sin a, cos a) and q + s·(sin b, cos b) on the grid (azimuths in degrees).
  const d1 = [Math.sin((a * Math.PI) / 180), Math.cos((a * Math.PI) / 180)];
  const d2 = [Math.sin((b * Math.PI) / 180), Math.cos((b * Math.PI) / 180)];
  const den = d1[0] * d2[1] - d1[1] * d2[0];
  if (Math.abs(den) < Math.sin((2 * Math.PI) / 180)) return null; // nearly parallel (< 2°)
  const t = ((q[0] - p[0]) * d2[1] - (q[1] - p[1]) * d2[0]) / den;
  if (t < 0) return null; // behind the observer
  return [p[0] + t * d1[0], p[1] + t * d1[1]];
}

/**
 * Solve on the UTM grid (the map method). `lines` are rays from known points
 * along grid azimuths. With 3+ rays the answer is the least-squares point
 * nearest all of them.
 */
function solve(lines: { from: LonLat; gridAz: number }[], gm?: number): Fix {
  if (lines.length < 2) throw new Error("At least two known points are needed");
  const { proj } = utmZone(lines[0].from);
  const pts = lines.map((l) => ({ xy: toUtm(l.from, proj), az: l.gridAz }));
  const inters: [number, number][] = [];
  for (let i = 0; i < pts.length; i++) {
    for (let j = i + 1; j < pts.length; j++) {
      const x = lineIntersect(pts[i].xy, pts[i].az, pts[j].xy, pts[j].az);
      if (x) inters.push(x);
    }
  }
  if (!inters.length) throw new Error("The lines don't cross: check the azimuths (they may be nearly parallel or pointing away from each other).");
  // Least squares: minimize the sum of squared perpendicular distances to all lines.
  let sxx = 0, sxy = 0, syy = 0, bx = 0, by = 0;
  for (const { xy, az } of pts) {
    const n = [Math.cos((az * Math.PI) / 180), -Math.sin((az * Math.PI) / 180)]; // line normal
    sxx += n[0] * n[0]; sxy += n[0] * n[1]; syy += n[1] * n[1];
    const c = n[0] * xy[0] + n[1] * xy[1];
    bx += n[0] * c; by += n[1] * c;
  }
  const det = sxx * syy - sxy * sxy;
  const best: [number, number] = pts.length === 2 || Math.abs(det) < 1e-9 ? inters[0] : [(syy * bx - sxy * by) / det, (sxx * by - sxy * bx) / det];
  let spread = 0;
  for (const a of inters) for (const b of inters) spread = Math.max(spread, Math.hypot(a[0] - b[0], a[1] - b[1]));
  const position = fromUtm(best, proj);
  const reach = Math.max(1000, ...pts.map((p) => Math.hypot(p.xy[0] - best[0], p.xy[1] - best[1]) * 1.25));
  const rays = pts.map((p): [LonLat, LonLat] => [
    fromUtm(p.xy, proj),
    fromUtm([p.xy[0] + reach * Math.sin((p.az * Math.PI) / 180), p.xy[1] + reach * Math.cos((p.az * Math.PI) / 180)], proj),
  ]);
  void gm;
  return { position, mgrs: toMgrs(position), intersections: inters.map((x) => fromUtm(x, proj)), errorSpread: spread, rays };
}

/**
 * Intersection: locate an unseen point from two or more known positions,
 * given the azimuth from each known position TO the point.
 */
export function intersection(sightings: Sighting[], gm?: number): Fix {
  return solve(sightings.map((s) => ({ from: s.point, gridAz: convertAzimuth(s.azimuth, s.north, "grid", s.point, gm) })), gm);
}

/**
 * Resection: locate yourself from two or more known landmarks, given the
 * azimuth you measured FROM your position TO each landmark. The back azimuth
 * from each landmark is drawn and the lines cross at your position.
 */
export function resection(sightings: Sighting[], gm?: number): Fix {
  return solve(sightings.map((s) => ({ from: s.point, gridAz: norm360(convertAzimuth(s.azimuth, s.north, "grid", s.point, gm) + 180) })), gm);
}

export function formatLength(m: number, unit: string): string {
  const [, f] = LENGTH_UNITS[unit] ?? LENGTH_UNITS.m;
  const v = m / f;
  return `${v.toLocaleString("en-US", { maximumFractionDigits: v < 10 ? 3 : v < 1000 ? 2 : 1 })} ${unit === "usft" ? "US ft" : unit}`;
}

export function formatArea(m2: number, unit: string): string {
  const [label, f] = AREA_UNITS[unit] ?? AREA_UNITS.sqm;
  const v = m2 / f;
  return `${v.toLocaleString("en-US", { maximumFractionDigits: v < 10 ? 4 : v < 1000 ? 2 : 1 })} ${label.toLowerCase()}`;
}
