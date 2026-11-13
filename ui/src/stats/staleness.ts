// Is a card's result still the layers' numbers? A result keeps the data version of each input layer at the time it ran
// (`StatResult.data_version`); when a layer's version differs now (an edit, a recalculated field, a replaced file) or the layer is gone,
// the card is out of date. Never recomputed silently: the card says so and the user runs it again.
import type { StatCard } from "./types";

export type CardState = "not-run" | "current" | "out-of-date";

export interface StaleReason {
  state: CardState;
  /** Layer ids whose version changed or that no longer exist. */
  changed: string[];
  missing: string[];
}

export function cardState(card: StatCard, layers: Record<string, { version: number } | undefined>): StaleReason {
  const r = card.result;
  if (!r) return { state: "not-run", changed: [], missing: [] };
  const changed: string[] = [];
  const missing: string[] = [];
  for (const [id, v] of Object.entries(r.data_version)) {
    const now = layers[id];
    if (!now) missing.push(id);
    else if (now.version !== v) changed.push(id);
  }
  // A card whose inputs name a layer the result was not computed from can only happen after an edit, which clears the result; stay safe anyway.
  for (const input of Object.values(card.inputs)) if (!(input.layer in r.data_version)) changed.push(input.layer);
  return { state: changed.length || missing.length ? "out-of-date" : "current", changed, missing };
}

/** The sentence under an out-of-date card. */
export function staleSentence(s: StaleReason, name: (id: string) => string): string {
  const parts: string[] = [];
  if (s.changed.length) parts.push(`${s.changed.map(name).join(", ")} changed since this ran`);
  if (s.missing.length) parts.push(`${s.missing.map(name).join(", ")} is no longer in the project`);
  return `Out of date: ${parts.join("; ")}. The numbers below are from before. Run the card again to update them.`;
}
