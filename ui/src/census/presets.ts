// Census variables people ask for most, and the checks the dialog makes before anything is sent.

export interface Preset {
  code: string;
  label: string;
  dataset: "acs" | "dhc";
}

export const PRESETS: Preset[] = [
  { code: "B01003_001E", label: "Total population", dataset: "acs" },
  { code: "B01002_001E", label: "Median age", dataset: "acs" },
  { code: "B19013_001E", label: "Median household income (dollars)", dataset: "acs" },
  { code: "B17001_002E", label: "People below the poverty level", dataset: "acs" },
  { code: "B25001_001E", label: "Housing units", dataset: "acs" },
  { code: "B25003_002E", label: "Owner-occupied housing units", dataset: "acs" },
  { code: "B25077_001E", label: "Median home value (dollars)", dataset: "acs" },
  { code: "P1_001N", label: "Total population (2020 Census)", dataset: "dhc" },
  { code: "H1_001N", label: "Housing units (2020 Census)", dataset: "dhc" },
];

/** Variable codes typed by the user: split on commas, spaces and new lines, upper-cased, de-duplicated. Returns the codes and the ones that are not codes. */
export function parseVariables(text: string): { codes: string[]; invalid: string[] } {
  const codes: string[] = [];
  const invalid: string[] = [];
  for (const raw of text.split(/[\s,;]+/)) {
    const v = raw.trim().toUpperCase();
    if (!v) continue;
    if (!/^[A-Z0-9_]+$/.test(v)) invalid.push(raw.trim());
    else if (!codes.includes(v)) codes.push(v);
  }
  return { codes, invalid };
}

/** The years a dataset offers. */
export function vintages(dataset: string, thisYear = new Date().getFullYear()): number[] {
  if (dataset.startsWith("dec_")) return [2020];
  const last = thisYear - 2; // the most recent release is about a year and a half old
  const first = dataset === "acs1" ? 2005 : 2009;
  return Array.from({ length: last - first + 1 }, (_, i) => last - i);
}

/** The field most likely to hold the GEOID. */
export function guessGeoidField(fields: { name: string }[]): string | null {
  const names = fields.map((f) => f.name);
  for (const want of ["GEOID", "GEOID20", "GEOID10", "GEO_ID", "FIPS", "GEOIDFQ"]) {
    const hit = names.find((n) => n.toUpperCase() === want);
    if (hit) return hit;
  }
  return names.find((n) => /geoid|fips/i.test(n)) ?? null;
}
