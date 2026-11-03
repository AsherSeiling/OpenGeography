// Search over tools (and layers) for the Shift+A menu; the ribbon search shares it.
// Whole-word starts, not substrings: "cut" finds Clip (keyword) but not "Execute". Query and text split on anything that is not a
// letter or digit, so "Warp (Reproject Raster)" and "Add USNG / MGRS Field" are found by their full names.
export interface Searchable {
  id: string;
  name: string;
  category: string;
  description: string;
  keywords?: string[];
}

export interface SearchHit<T> {
  item: T;
  /** Sum of the per-word ranks; lower is better. */
  rank: number;
  /** The keyword that made it match, when no name word did. */
  hit?: string;
}

export const words = (s: string): string[] => s.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);

/** Rank of one query word against one item: 0 name starts with it, 1 a name word, 2 keyword, 3 category, 4 description; null = no match. */
export function scoreWord(t: Searchable, w: string): { rank: number; hit?: string } | null {
  const name = t.name.toLowerCase();
  if (name.startsWith(w)) return { rank: 0 };
  if (words(name).some((x) => x.startsWith(w))) return { rank: 1 };
  const k = (t.keywords ?? []).find((x) => words(x).some((y) => y.startsWith(w)));
  if (k) return { rank: 2, hit: k };
  const inWords = (s: string) => words(s).some((x) => x.startsWith(w));
  if (inWords(t.category)) return { rank: 3 };
  if (inWords(t.description)) return { rank: 4 };
  return null;
}

/** Items matching every word of `q`, best first; favourites break ties, then name. Empty for a query with no letters or digits. */
export function searchTools<T extends Searchable>(items: T[], q: string, favourites: ReadonlySet<string> | readonly string[] = []): SearchHit<T>[] {
  const ws = words(q);
  if (!ws.length) return [];
  const fav = favourites instanceof Set ? favourites : new Set(favourites);
  const out: SearchHit<T>[] = [];
  for (const item of items) {
    let rank = 0;
    let hit: string | undefined;
    let ok = true;
    for (const w of ws) {
      const m = scoreWord(item, w);
      if (!m) {
        ok = false;
        break;
      }
      rank += m.rank;
      hit ??= m.hit;
    }
    if (ok) out.push({ item, rank, hit });
  }
  return out.sort((a, b) => a.rank - b.rank || Number(fav.has(b.item.id)) - Number(fav.has(a.item.id)) || a.item.name.localeCompare(b.item.name));
}
