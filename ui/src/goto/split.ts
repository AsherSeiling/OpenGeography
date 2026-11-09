// Go To Coordinates: pasting one combined string ("34.05, -118.25", "34°03′N 118°15′W", "N 34 03 08 W 118 15 00") fills the latitude
// box and then the longitude box. N, S, E and W letters override the order. A string that can't be split safely is reported, never
// guessed. Reading the numbers themselves (and refusing values beyond 90° or 180°) is the engine's job (`parse_location`).

export type Split = { lat: string; lon: string; note: string } | { error: string } | null;

const LAT = /[NSns]/;
const LON = /[EWew]/;
const letters = (s: string) => s.match(/[NSEWnsew]/g) ?? [];

/** Split a pasted position into its two halves, or null when it holds only one coordinate (so the paste is left alone). */
export function splitPasted(raw: string): Split {
  const text = raw.trim();
  if (!text) return null;
  let parts: string[] | null = null;

  // 1. A comma, semicolon, tab or new line between the two halves.
  const sep = text.split(/[,;\t\n]+/).map((p) => p.trim()).filter(Boolean);
  if (sep.length === 2) parts = sep;
  else if (sep.length > 2) return { error: "That has more than two parts. Paste one latitude and one longitude, for example 34.05, -118.25." };

  // 2. Hemisphere letters mark the ends: "34 03 08 N 118 15 00 W" splits after the first letter, "N 34 03 08 W 118 15 00" before the second.
  if (!parts) {
    const found = [...text.matchAll(/[NSEWnsew]/g)];
    if (found.length === 2) {
      const [a, b] = found.map((m) => m.index!);
      const cut = a === 0 || /^[NSEWnsew]/.test(text) ? b : a + 1;
      parts = [text.slice(0, cut).trim(), text.slice(cut).trim()];
    } else if (found.length > 2) return { error: "That has more than two of the letters N, S, E and W. Paste one latitude and one longitude." };
  }

  // 3. Plain numbers separated by spaces: two numbers, or two degrees-minutes(-seconds) groups of the same size.
  if (!parts) {
    const tok = text.replace(/[°º˚′″'"’”]/g, " ").split(/\s+/).filter(Boolean);
    if (tok.length === 2) parts = tok;
    else if (tok.length === 4) parts = [tok.slice(0, 2).join(" "), tok.slice(2).join(" ")];
    else if (tok.length === 6) parts = [tok.slice(0, 3).join(" "), tok.slice(3).join(" ")];
    else if (tok.length === 1) return null;
    else return { error: "I can't tell where the latitude ends and the longitude begins. Put a comma between them, or give each its N, S, E or W letter." };
  }

  let [first, second] = parts;
  const l1 = letters(first);
  const l2 = letters(second);
  if (l1.length > 1 || l2.length > 1) return { error: "Each half may carry only one of the letters N, S, E and W." };
  const axis = (l: string[]) => (l.length === 0 ? null : LAT.test(l[0]) ? "lat" : LON.test(l[0]) ? "lon" : null);
  const a1 = axis(l1);
  const a2 = axis(l2);
  if (a1 && a1 === a2) return { error: `Both halves are ${a1 === "lat" ? "latitudes (N or S)" : "longitudes (E or W)"}. One must be a latitude and the other a longitude.` };
  let note = "Filled as latitude, then longitude.";
  if (a1 === "lon" || a2 === "lat") {
    [first, second] = [second, first];
    note = "The letters say which is which: filled the latitude from the part with N or S and the longitude from the part with E or W.";
  }
  return { lat: first, lon: second, note };
}

/** True when a box holds something the engine could read as an angle at all (a quick check while typing; the engine decides). */
export function looksLikeAngle(s: string): boolean {
  return /^[\s0-9.+\-°º˚′″'"’”:NSEWnsew]+$/.test(s) && /\d/.test(s);
}
