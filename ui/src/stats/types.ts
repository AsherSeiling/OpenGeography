// Statistics sheet types, mirroring `crates/engine/src/stats/` (docs/STATISTICS_WORKSPACE_PLAN.md, "ST0 result"). Field names are the
// engine's, snake_case, because the same JSON is saved in the project (`__stat_sheets`). `ipc/api.ts` imports these.

export interface Dropped {
  reason: string;
  count: number;
}

export type ColumnKind = "number" | "integer" | "text" | "bool";

export interface StatColumn {
  name: string;
  unit?: string;
  kind: ColumnKind;
}

export interface StatTable {
  title: string;
  columns: StatColumn[];
  /** Full precision; the sheet rounds for display. */
  rows: (number | string | boolean | null)[][];
}

export type ChartKind = "histogram" | "box" | "scatter" | "qq" | "bar" | "line" | "heatmap";

export interface Axis {
  label: string;
  unit?: string;
  min?: number;
  max?: number;
}

export interface Mark {
  x?: number;
  y?: number;
  x2?: number;
  y2?: number;
  label?: string;
  /** Features this mark stands for (linked selection). */
  fids?: number[];
  /** A density bin key, when the mark stands for too many features to list. */
  bin?: string;
}

export interface ChartSpec {
  kind: ChartKind;
  title: string;
  x: Axis;
  y: Axis;
  series: { name: string; marks: Mark[] }[];
  notes: string[];
}

export interface StatResult {
  /** One sentence naming the exact method. */
  method: string;
  citations: string[];
  /** Rows used; null when the tool does not report it (the card says so, it never implies every row was used). */
  n_used: number | null;
  dropped: Dropped[];
  tables: StatTable[];
  charts: ChartSpec[];
  notes: string[];
  seed: number | null;
  /** Layer id -> that layer's data version when this was computed. */
  data_version: Record<string, number>;
}

export interface CardInput {
  layer: string;
  /** "Selected features only": the feature ids picked when the card was set up. */
  fids?: number[] | null;
}

export interface StatCard {
  id: string;
  /** `ToolSpec.id` of a `Statistics › …` tool. */
  tool: string;
  title?: string | null;
  /** Tool input port id -> the layer or table feeding it. */
  inputs: Record<string, CardInput>;
  /** Tool parameters by `ToolParam.id`. */
  options: Record<string, unknown>;
  /** The last run; null until the card has run (and again after its inputs or options change). */
  result: StatResult | null;
}

export interface StatSheet {
  id: string;
  name: string;
  version: number;
  cards: StatCard[];
}

export interface FieldProfile {
  name: string;
  type: string;
  /** NULL (or NaN) values; empty text counts as present. */
  missing: number;
}

export interface DataProfile {
  layer: string;
  rows: number;
  fields: FieldProfile[];
}

export const SHEET_VERSION = 1;
/** A tool is a statistics tool (and so can be a card) when its category starts with this. */
export const CATEGORY_PREFIX = "Statistics ›";
