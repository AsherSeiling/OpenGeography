# Attribute history between nodes ("git for tables") — plan

_Opus design, 2026-10-23. Phase B, task B1. Build sessions: B2.1–B2.3 below._

**Goal:** in a layer's node tree, the user can open the table produced by **any node** and see **what changed** compared with another node: which rows were added, removed or changed, and which fields were added, removed or retyped. This works the same after the project is saved and reopened.

---

## 1. What exists today (the starting point)

- `graph::run` materializes each node's output as `__node_<hash>`. The hash covers the tool, its params and the input hashes, plus the source layer's `id:version:crs` for Layer Input nodes. Outputs are `TABLE`, except Layer Input, which is a `VIEW` over the layer's table.
- `graph::Cache` (hash → `Rel`) is held **in memory only**. `Engine::init_schema` drops every `__node_%` table at startup. `save_project` (Save As) clears the cache before `COPY FROM DATABASE`.
- Intermediate tables have **no stable row id**. Layer Input runs `EXCLUDE (__fid, __g, …)`, and result layers renumber `__fid` with `row_number() OVER ()`.
- `serde_json` is built without `preserve_order`, so params serialize with sorted keys and the hashes are stable between runs. Keep it that way (§3, test H3).
- The UI receives `NodeResult.output` (the cache table name) for each node it runs.

---

## 2. Decisions

### D1. Snapshots are the existing cache tables, persisted
No second copy. A snapshot **is** `__node_<hash>`. Because the hash depends only on the inputs:
- undoing back to earlier params finds the old snapshot again;
- a snapshot can always be rebuilt by running the model up to that node (`run_graph` with `target`).

Snapshots are therefore a **cache with a budget**, not irreplaceable data. Losing one costs a re-run, never correctness.

### D2. An index table in the project database
```sql
CREATE TABLE IF NOT EXISTS __node_cache (
  hash VARCHAR PRIMARY KEY,
  table_name VARCHAR,        -- __node_<hash>
  kind VARCHAR,              -- 'table' | 'view'
  crs VARCHAR, has_geom BOOLEAN,
  rows BIGINT, bytes BIGINT, -- bytes = estimate (D4)
  engine_version VARCHAR,    -- CARGO_PKG_VERSION + CACHE_FORMAT const
  created TIMESTAMP, last_used TIMESTAMP
);
```
- `graph::run` writes a row whenever it creates a cache table, and bumps `last_used` on a cache hit. The write happens **after** the SQL finishes, in the same transaction as the `CREATE`, so a cancelled step leaves neither.
- On open (`init_schema`), replace "drop all `__node_%`" with:
  - load the index into `Cache.entries`;
  - drop entries whose `engine_version` differs from this build's. Tool code may have changed, and stale results must never show as current;
  - drop entries whose table is missing, or a view that fails `DESCRIBE` (its layer was deleted);
  - drop orphan `__node_%` tables with no index row.
- Save As: **stop clearing** the cache before `COPY FROM DATABASE`. The index and the tables copy with everything else.
- Rasters are **out of scope** (no attributes). Raster cache entries stay session-only, as today.
- "Clear model cache" also clears the index.

### D3. Which snapshots are kept: the UI says what is live
The engine doesn't know the trees (the UI holds them). On **save**, and after a tree is deleted, the UI calls `prune_node_cache(live: Vec<String>)`. `live` holds every `output` table name the UI currently holds for any node in any tree. The UI stores `lastOutput` in each node's data, saved in `project.json`. The engine drops index entries not in `live`.

### D4. Size budget and eviction
- Budget: **2 GB per project** by default. It is stored in `project.json` and shown in Project settings ("Node history: keep up to [2] GB"). Set it to 0 to keep nothing between sessions.
- `bytes` estimate, computed once at creation from a 1,000-row sample: `rows × (8 × non-geometry columns + average octet_length(ST_AsWKB(geom)))`. It is cheap and good enough for a budget.
- After each run and on save, while the total exceeds the budget, evict in this order:
  1. entries not in the last `live` list;
  2. then the least recently used.
  The entry currently open in the table pane is never evicted (the UI passes it as `pinned`).
- Dropping tables doesn't shrink the DuckDB file right away (freed blocks are reused). That's acceptable. A "Compact project" command (copy to a fresh file) is a later nicety, not part of B2.

### D5. How rows are matched between two nodes (the core decision)
Nodes don't carry a row id, and most tools can't keep one (dissolve, merge, joins). So the diff is **tool-agnostic**, with this key order:

1. **The user's key field.** The user picks it in the diff bar ("Match rows by: [GEOID ▾]"). It must exist in both tables.
2. **An auto-detected key.** This is a column present in both tables, non-null and unique in both. Prefer names matching `/^(id|fid|objectid|gid|geoid|.*_id|.*id)$/i`, then the first such column in field order. Unique and non-null is checked with one `count(*) = count(DISTINCT col)` query per candidate, at most 5 candidates.
3. **No key: a content diff.** The tables are compared as multisets of row hashes. The hash is `md5` of the common non-geometry columns cast to VARCHAR, with NULLs as a sentinel, plus `md5(ST_AsWKB(geom))`. This reports only **added/removed** rows. The UI says plainly: "No matching key — showing rows that exist in only one table. Pick a key field to see changed rows."

Rules:
- **Changed:** a key is in both tables, and any common column `IS DISTINCT FROM` its counterpart. Geometry is compared by WKB bytes.
- **CRS differs** between the two nodes (e.g. across a Project node): geometry isn't compared. The summary says "Geometry not compared: coordinate systems differ (EPSG:a → EPSG:b)". Never auto-reproject for a diff.
- **Duplicate keys** in either table: fall back to the content diff, and report "Key 'X' isn't unique in <node> (n duplicates)".
- **Schema diff** (always): fields added, removed, and retyped (same name, different type). Names are compared case-insensitively, like `nodes::join_columns`.
- All work runs in DuckDB SQL (hash joins), wrapped in `progress::Task` "Comparing tables" and cancellable through the connection interrupt, the same pattern as `run_graph`.

### D6. Diff results are temporary
`node_diff` materializes `__diff_<hashA>_<hashB>_<key>`, with columns `kind` ('added'/'removed'/'changed'), key, the A values, the B values, and a changed-column bitmask or list. It returns a summary. These tables are **not** indexed or persisted. They are dropped on project close or new project, and when the table pane closes the diff.

### D7. Engine API (Tauri command + devserver route + `api.ts` for each)
| Command | Returns |
|---|---|
| `node_table_info(table)` | fields (name, type), rows, crs, hasGeom, kind |
| `node_rows(table, RowsQuery)` | same `RowsPage` shape as `rows`. For a `TABLE`, order by DuckDB `rowid` (stable for a CTAS table); sort/filter as in `table::rows`. For a `VIEW` (Layer Input), page the view the same way with `ORDER BY` the layer's `__fid` (re-select it from the base table); Sonnet picks the simplest correct form and adds a test for stable paging. `fids` in the page are row positions, not layer fids. |
| `node_diff(a, b, key?)` | `DiffSummary {diffId, keyUsed, keySource: 'user'\|'auto'\|'none', note?, fieldsAdded[], fieldsRemoved[], fieldsRetyped[{name, from, to}], rowsA, rowsB, added, removed, changed, unchanged, geometryCompared}` |
| `node_diff_rows(diffId, kind, offset, limit)` | rows with `{kind, key, values: [{field, a, b, changed}]}` |
| `prune_node_cache(live, pinned?)` | number dropped |

Node snapshots are **read-only**. Editing stays on layers (Output never modifies source data).

### D8. Table pane UX (Fluent v9, existing `AttributeTable.tsx`)
- **Node picker** in the table toolbar: a `Dropdown` labelled "Table of:".
  - Default entry: **"<Layer> (Output)"**, today's behavior.
  - Then the open tree's nodes in **topological order** (same order as `graph::plan`), each with the node title, the tool label, and a status badge:
    - *current*;
    - *out of date*: params changed since the snapshot (reuse the existing changed/stale markers);
    - *not stored*: evicted or never run. Selecting it shows a "Run to this node" button, which calls `run_graph` with `target`.
  - **Muted** nodes are listed greyed and disabled, with "Muted — passes its input through".
- **"Compare with"** toggle and second dropdown. Default: the node's **primary input**, meaning the source of its first input port in `ToolSpec` order. Any node of the same tree can be chosen.
- **Diff bar**: chips "+N added", "−N removed", "~N changed", "Fields: +a −b ~c". Each chip filters the grid. "Match rows by: [auto: GEOID ▾]" key picker. The note line (no key, CRS differs, duplicates) is always shown when set.
- **Grid in diff mode**:
  - added rows: green left bar;
  - removed rows: struck through, red left bar;
  - changed cells: highlighted, with the old value in a tooltip ("was: 1200");
  - added and removed columns: tinted headers.
- **Entry points:**
  - the picker;
  - the node card's context menu: **"View table"**, which opens the table pane on that node;
  - **"Compare with input"**, which opens the pane with the diff on.
- **Selection sync** with the map is only for the Output/layer entry (node rows have no layer fid). The selection buttons are disabled in node mode, with a tooltip.
- **Undo, redo and tree switching:** the picker follows the open tree. Undoing params makes the node *out of date* until it runs again. Because of D1, the run is usually an instant cache hit.

### Rejected / deferred
- **Carrying a lineage id (`__src_fid`) through every tool.** It touches all 48 tools, collides in joins (`join_columns` filters only `__fid`), and is meaningless after dissolve or merge. Revisit only if users need row lineage across aggregations.
- **Full version history over time (a commit log of edits to a layer).** That's a different feature: attribute edits on layers already bump `version`. Deferred, and not promised in B.
- **Persisting raster node outputs.** Deferred (no attributes, large files).

---

## 3. Build sessions (Sonnet)

### B2.1 — Persist the node cache (engine only)
Files: `crates/engine/src/graph.rs`, `lib.rs` (`init_schema`, `save_project`, `clear_model_cache`, new `prune_node_cache`), engine tests.

Acceptance:
- H1. Run a model, save, close, reopen: running the same graph reports every non-raster step as **`cached`**, and the rows are identical.
- H2. Entries from a different `engine_version` are dropped on open, and those steps re-run (`done`).
- H3. Hash stability: building the same graph twice, with params inserted in a different key order, gives the same hash. This is a regression guard against `serde_json/preserve_order` being switched on by feature unification.
- H4. A view whose layer was deleted is dropped on open without error. Orphan `__node_%` tables with no index row are dropped.
- H5. `prune_node_cache(live)` drops the non-live entries. The budget evicts in D4 order and never evicts `pinned`.
- H6. A cancelled step leaves no index row and no table (same transaction).
- H7. Save As keeps the snapshots (the new project folder reopens with cached steps).
- Existing tests are still green, including the cancel test and the result-transaction test.

### B2.2 — Diff and node-rows commands
Files: new `crates/engine/src/history.rs` (diff + node rows), `lib.rs` methods, `src-tauri/src/commands.rs` + `main.rs` registration, `crates/devserver`, `ui/src/ipc/api.ts`.

Acceptance (engine tests on small fixture tables):
- H8. Key diff finds the exact added, removed and changed counts, and the changed cells, including NULL ↔ value (`IS DISTINCT FROM`).
- H9. Field added, removed and retyped are reported. A retyped column still compares its values cast to VARCHAR.
- H10. Auto-key picks `GEOID` over a non-unique `NAME`. A duplicate user key falls back to the content diff, with the note.
- H11. Content diff (no key) handles duplicate identical rows as a multiset: 3 copies → 2 copies = 1 removed.
- H12. Geometry: a moved vertex counts as a changed row. Different CRS gives `geometryCompared=false`, with the note.
- H13. `node_rows` pages stably (same order on repeated calls) for both a `TABLE` and a Layer Input `VIEW`.
- H14. `#[ignore]` performance test: 1M rows × 20 fields, key diff in < 5 s release on an M-series Mac. Record the number in PROJECT_STATUS.
- H15. Diff runs in a `progress::Task` and Cancel stops it (DuckDB interrupt).
- Every command is reachable through the devserver (curl check).

### B2.3 — Table pane UI
Files: `ui/src/panes/AttributeTable.tsx` (or a new `ui/src/panes/table/NodePicker.tsx` + `DiffBar.tsx` if the file grows past ~700 lines), `ui/src/store/model.ts` (`lastOutput` in node data, `prune` on save), node card context menu in `ui/src/nodes/`, vitest.

Acceptance:
- H16. The picker lists the tree's nodes in topological order. Muted nodes are disabled. Out-of-date and not-stored badges follow the node state (vitest on a pure `pickerEntries(tree, results)` function).
- H17. "Compare with" defaults to the primary input (vitest).
- H18. On save, the UI calls `prune_node_cache` with every tree's `lastOutput` (vitest with a mocked api).
- H19. Browser check (devserver + vite): import a test layer, then Buffer → Calculate Field → Filter. Open Filter's table, compare it with Calculate Field, and the chips show the right removed count and the changed field. One screenshot.
- H20. Node mode is read-only: the edit and selection buttons are disabled, with tooltips.
- Then rebuild the Mac app (`scripts/package-macos.sh`) and run `--self-test`; update ROADMAP row "Attribute history" to ✅ and PROJECT_STATUS.

**Exit for B2:** H1–H20 pass, CI green on all three platforms.

**Order:** B2.1 → B2.2 → B2.3. B2.1 and B2.2 both edit `lib.rs`, so they don't run in parallel. B2.2 adds Tauri commands, so it doesn't run in parallel with any other command-adding task (MASTER_PLAN §5).
