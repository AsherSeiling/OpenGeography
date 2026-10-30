// Fitting ribbon groups into the window width without a scroll bar. Pure, so it is unit-tested: it never drops a command (every group is
// either shown inline, in some size variant, or listed in the "More" menu).
//
// Variants of a group, widest first: 0 as declared (large and small buttons), 1 every button small (stacked three high),
// 2 small icon-only buttons, 3 collapsed into a single button that opens a menu.
export type Variant = 0 | 1 | 2 | 3;
export const LAST_VARIANT: Variant = 3;

export interface FitInput {
  /** Natural width in px of each group in each variant (index = group, then variant 0..3; narrower or equal as the variant grows). */
  widths: number[][];
  /** Space available for the groups, px. */
  avail: number;
  /** Width of the "More" button, px. */
  moreWidth: number;
}

export interface FitResult {
  /** Variant of each group (for groups shown inline). */
  variants: Variant[];
  /** The first `shown` groups are inline; the rest are in the More menu. */
  shown: number;
}

/**
 * Shrinks the right-most groups first, one variant step at a time, until everything fits; if even the fully collapsed groups do not
 * fit, the right-most groups move into the More menu. Always keeps at least one group inline.
 */
export function fitRibbon({ widths, avail, moreWidth }: FitInput): FitResult {
  const n = widths.length;
  const variants: Variant[] = widths.map(() => 0);
  const total = (shown: number) => {
    let t = 0;
    for (let i = 0; i < shown; i++) t += widths[i][variants[i]];
    return t + (shown < n ? moreWidth : 0);
  };
  // The next variant of a group that is really narrower than its current one (a collapsed button can be wider than icon-only buttons).
  const narrower = (g: number): Variant | null => {
    for (let v = variants[g] + 1; v <= LAST_VARIANT; v++) if (widths[g][v] < widths[g][variants[g]]) return v as Variant;
    return null;
  };
  while (total(n) > avail) {
    let g = -1;
    for (let i = n - 1; i >= 0; i--) {
      if (narrower(i) !== null) {
        g = i;
        break;
      }
    }
    if (g < 0) break;
    variants[g] = narrower(g)!;
  }
  let shown = n;
  while (shown > 1 && total(shown) > avail) shown--;
  return { variants, shown };
}
