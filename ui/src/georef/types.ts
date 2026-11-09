// Georeferencing control points (GR1). Mirrors crates/engine/src/georef/mod.rs.
// Conventions: pixel/line are GDAL image coordinates (pixel-is-area, (0,0) = top-left corner of the top-left pixel, line grows
// downwards); x/y are in the set's own target CRS, x = easting/longitude, y = northing/latitude.

export interface ControlPoint {
  id: number;
  pixel: number;
  line: number;
  x: number;
  y: number;
  enabled: boolean;
}

export interface ControlSet {
  id: string;
  name: string;
  /** Chosen by the user; never guessed. */
  targetCrs: string;
  image?: string | null;
  points: ControlPoint[];
}

export interface ParsedControlFile {
  format: "csv" | "qgis";
  /** Null when the file names no CRS: the user must be asked. */
  crs: string | null;
  points: ControlPoint[];
  notes: string[];
}

/** The next free point id. */
export function nextPointId(points: ControlPoint[]): number {
  return points.reduce((m, p) => Math.max(m, p.id), 0) + 1;
}

/** A fresh, empty set (no CRS yet: the user picks it before anything is saved). */
export function newControlSet(name: string, image: string | null): ControlSet {
  return { id: `cs-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`, name, targetCrs: "", image, points: [] };
}

// ---------- raster warp (GR2); mirrors crates/engine/src/georef/warp.rs ----------

export type WarpMethod = { kind: "polynomial"; order: 1 | 2 | 3 } | { kind: "tps" };

export const METHODS: { id: string; label: string; method: WarpMethod; min: number }[] = [
  { id: "p1", label: "1st-order polynomial (affine), 3+ points", method: { kind: "polynomial", order: 1 }, min: 3 },
  { id: "p2", label: "2nd-order polynomial, 6+ points", method: { kind: "polynomial", order: 2 }, min: 6 },
  { id: "p3", label: "3rd-order polynomial, 10+ points", method: { kind: "polynomial", order: 3 }, min: 10 },
  { id: "tps", label: "Thin plate spline, 3+ points", method: { kind: "tps" }, min: 3 },
];

export type Resampling = "nearest" | "bilinear" | "cubic" | "cubicspline" | "lanczos";

export interface PointError {
  id: number;
  enabled: boolean;
  /** Left out of the fit: its error is an independent accuracy figure. */
  check: boolean;
  /** Where the transform puts the point minus where it was said to be (target CRS units). */
  dx: number;
  dy: number;
  length: number;
  lengthM: number;
  dpixel: number;
  dline: number;
  lengthPx: number;
}

export interface ErrorReport {
  method: WarpMethod;
  methodLabel: string;
  minPoints: number;
  enabledCount: number;
  points: PointError[];
  rms: number;
  rmsM: number;
  rmsPx: number;
  checkRms: number | null;
  checkRmsM: number | null;
  checkRmsPx: number | null;
  unit: string;
  metresKind: "geodesic" | "grid";
  formula: string;
  warnings: string[];
}

export interface ImageInfo {
  width: number;
  height: number;
  bands: number;
  hasGeoref: boolean;
  crs: string | null;
}

export interface WarpPreview {
  /** PNG, base64. */
  png: string;
  /** [xmin, ymin, xmax, ymax] in `crs`. */
  extent: [number, number, number, number];
  width: number;
  height: number;
  crs: string;
  report: ErrorReport;
}

// ---------- vector adjustment (GR3); mirrors crates/engine/src/georef/adjust.rs ----------
// In a control point used for adjustment, pixel/line hold the FROM position (x, y in the layer's CRS) and x/y the TO position in
// the set's target CRS.

export type AdjustMethod = "affine" | "similarity" | "rubberSheet";

export const ADJUST_METHODS: { id: AdjustMethod; label: string; min: number }[] = [
  { id: "affine", label: "Affine (3+ points)", min: 3 },
  { id: "similarity", label: "Similarity: scale, rotation, shift (2+ points)", min: 2 },
  { id: "rubberSheet", label: "Rubber sheet: thin plate spline (3+ points)", min: 3 },
];

export interface AdjustReport {
  method: AdjustMethod;
  methodLabel: string;
  minPoints: number;
  enabledCount: number;
  points: PointError[];
  rms: number;
  rmsM: number;
  checkRms: number | null;
  checkRmsM: number | null;
  unit: string;
  metresKind: "geodesic" | "grid";
  formula: string;
  warnings: string[];
  outputCrs: string;
  scale: number | null;
  rotationDeg: number | null;
}

export interface AdjustRequest {
  layer: string;
  set: ControlSet;
  method: AdjustMethod;
  name?: string | null;
}

export interface AdjustResult {
  layer: import("../ipc/api").LayerInfo;
  dropped: { fid: number; reason: string }[];
  moved: number;
  report: AdjustReport;
  notes: string[];
}

export interface WarpRequest {
  set: ControlSet;
  method: WarpMethod;
  resampling?: Resampling;
  image: string;
  sourceLayer?: string | null;
  name?: string | null;
}
