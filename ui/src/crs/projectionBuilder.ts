// Projection builder (R8, RD-10): the classic families as a PROJ string, with the checks a map-maker needs.

export type Family = "lcc" | "aea" | "eqdc" | "merc" | "eqc" | "cea" | "laea" | "aeqd" | "stere" | "robin" | "moll" | "eck4";

export interface FamilySpec {
  id: Family;
  label: string;
  group: "Conic" | "Cylindrical" | "Azimuthal" | "World";
  /** Which parameters the family takes. */
  uses: { lat1?: boolean; lat2?: boolean; lat0?: boolean; latTs?: boolean };
  note: string;
}

export const FAMILIES: FamilySpec[] = [
  { id: "lcc", label: "Lambert conformal conic", group: "Conic", uses: { lat1: true, lat2: true, lat0: true }, note: "Keeps shapes (conformal); good for east–west regions." },
  { id: "aea", label: "Albers equal-area conic", group: "Conic", uses: { lat1: true, lat2: true, lat0: true }, note: "Keeps areas; the usual choice for thematic maps of a country." },
  { id: "eqdc", label: "Equidistant conic", group: "Conic", uses: { lat1: true, lat2: true, lat0: true }, note: "Correct distances along meridians." },
  { id: "merc", label: "Mercator", group: "Cylindrical", uses: {}, note: "Conformal; areas grow toward the poles, which are not shown." },
  { id: "eqc", label: "Equirectangular", group: "Cylindrical", uses: { latTs: true }, note: "Plain lon/lat grid; correct scale along the standard parallel." },
  { id: "cea", label: "Cylindrical equal-area", group: "Cylindrical", uses: { latTs: true }, note: "Keeps areas; shapes squash toward the poles." },
  { id: "laea", label: "Lambert azimuthal equal-area", group: "Azimuthal", uses: { lat0: true }, note: "Keeps areas; centred on a point." },
  { id: "aeqd", label: "Azimuthal equidistant", group: "Azimuthal", uses: { lat0: true }, note: "True distances and directions from the centre." },
  { id: "stere", label: "Stereographic", group: "Azimuthal", uses: { lat0: true }, note: "Conformal; the opposite pole is out of reach." },
  { id: "robin", label: "Robinson", group: "World", uses: {}, note: "A compromise for whole-world maps." },
  { id: "moll", label: "Mollweide", group: "World", uses: {}, note: "Equal-area oval world map." },
  { id: "eck4", label: "Eckert IV", group: "World", uses: {}, note: "Equal-area world map with straight poles." },
];

export interface BuilderParams {
  family: Family;
  lon0: number;
  lat0: number;
  lat1: number;
  lat2: number;
  latTs: number;
  datum: "WGS84" | "NAD83";
}

export const DEFAULTS: BuilderParams = { family: "aea", lon0: -96, lat0: 23, lat1: 29.5, lat2: 45.5, latTs: 0, datum: "WGS84" };

const num = (v: number) => String(Math.round(v * 1e6) / 1e6);

/** Problems with the parameters, as sentences; empty when the string can be built. */
export function validate(p: BuilderParams): string[] {
  const f = FAMILIES.find((x) => x.id === p.family);
  if (!f) return ["Choose a projection family."];
  const out: string[] = [];
  if (!Number.isFinite(p.lon0) || p.lon0 < -180 || p.lon0 > 180) out.push("The central meridian must be between −180° and 180°.");
  const lat = (v: number, what: string) => {
    if (!Number.isFinite(v) || v < -90 || v > 90) out.push(`${what} must be between −90° and 90°.`);
  };
  if (f.uses.lat0) lat(p.lat0, "The latitude of origin");
  if (f.uses.latTs) lat(p.latTs, "The standard parallel");
  if (f.uses.lat1 && f.uses.lat2) {
    lat(p.lat1, "The first standard parallel");
    lat(p.lat2, "The second standard parallel");
    if (p.lat1 === -p.lat2 && p.lat1 !== 0) out.push("The two standard parallels cannot be equal and opposite (the cone would have no shape).");
    if (p.lat1 === 0 && p.lat2 === 0 && p.family === "lcc") out.push("A Lambert conformal conic needs a standard parallel away from the equator.");
    if (Math.abs(p.lat1) === 90 || Math.abs(p.lat2) === 90) out.push("A standard parallel cannot be a pole.");
  }
  if (f.id === "eqc" && Math.abs(p.latTs) === 90) out.push("The standard parallel cannot be a pole.");
  if (f.id === "cea" && Math.abs(p.latTs) >= 90) out.push("The standard parallel must be inside ±90°.");
  return out;
}

/** The PROJ string for the parameters, or null when they are invalid. */
export function buildProjString(p: BuilderParams): string | null {
  if (validate(p).length) return null;
  const f = FAMILIES.find((x) => x.id === p.family)!;
  const parts = [`+proj=${f.id}`];
  if (f.uses.lat1) parts.push(`+lat_1=${num(p.lat1)}`);
  if (f.uses.lat2) parts.push(`+lat_2=${num(p.lat2)}`);
  if (f.uses.latTs) parts.push(`+lat_ts=${num(p.latTs)}`);
  if (f.uses.lat0) parts.push(`+lat_0=${num(p.lat0)}`);
  parts.push(`+lon_0=${num(p.lon0)}`, "+x_0=0", "+y_0=0", `+datum=${p.datum}`, "+units=m", "+no_defs");
  return parts.join(" ");
}

/** A short name for the layer list and status bar. */
export function builderName(p: BuilderParams): string {
  const f = FAMILIES.find((x) => x.id === p.family);
  const std = f?.uses.lat1 ? `, parallels ${num(p.lat1)}°/${num(p.lat2)}°` : f?.uses.latTs ? `, parallel ${num(p.latTs)}°` : "";
  return `${f?.label ?? p.family} (central meridian ${num(p.lon0)}°${std})`;
}
