# 3. Attribute table, model editor, executor, history, progress and cancel

Existing: TP §5, §6, §8, §13. Conventions: [`../TEST_PLAN_FULL.md`](../TEST_PLAN_FULL.md#conventions).

## 3.1 Attribute table and editing (`get_rows`, `add_field`, `delete_field`, `rename_field`, `update_cell`, `calculate_*`, `delete_rows`, `unique_values`, `quantile_breaks`)

| ID | Pre | Steps | Expected (user-facing) | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| TBL-01 | 5 M-row layer | Open table, scroll to the end, sort, filter | Virtualised; each page < 1 s; sort on a text field with NULLs works | E+2E (perf) | partial: TP 5.1; to automate perf | P1 |
| TBL-02 | — | Undo/redo of edits in the table (Ctrl+Z) | **No undo exists for table edits.** Ctrl+Z in the table must do nothing destructive and the UI must say edits are immediate | M | manual (document behaviour) | P1 |
| TBL-03 | — | Edit a cell, then immediately close the project / quit | Edit saved or "unsaved changes" prompt includes table edits | 2E | to automate | P0 |
| TBL-04 | Layer in a node-tree result | Edit result cell | Read-only message (TP 5.13) shown in the table pane, not pop-up | 2E | partial: TP 5.13 | P1 |
| TBL-05 | — | Edit a cell while a model run / tile build is in flight | Edit succeeds, run result unaffected (or the run is flagged stale) | E | to automate (CON-04) | P0 |
| TBL-06 | — | Cell with value 1e308, −0, NaN, "Infinity", 10 000 chars, emoji, `\0`, leading/trailing spaces, newline | Stored faithfully or rejected with "not a valid … value" | E | partial: TP 5.4–5.6; to automate extremes | P1 |
| TBL-07 | — | Add field name 63/64/255 chars, `select`, quoted, same as existing differing by accent | Accepted/rejected deterministically, tools still run (quoting) | E | partial: TP 5.2; to automate | P1 |
| TBL-08 | — | Calculate Field on 1 M rows; cancel | Progress + cancel; cancel leaves field unchanged | E | to automate (not cancellable today? verify) | P1 |
| TBL-09 | — | Calculate Field expression using geometry functions, string concat with NULL, integer overflow, `1/0` | Messages in plain language; no `__node_` or SQL parser dump | E | partial: TP 5.9; to automate overflow | P0 |
| TBL-10 | Table layer (no geometry) | Calculate Geometry / Zoom to layer / Symbology / Select by location | Disabled with reason; if forced via API, readable error | E+2E | to automate | P1 |
| TBL-11 | Layer with no CRS | Calculate Geometry area | "needs a coordinate system" | E | partial: TP 2.4 | P0 |
| TBL-12 | — | Delete the last field; delete all rows; add field to empty layer | Allowed; table renders empty state | E+2E | partial: TP 5.12; to automate UI | P1 |
| TBL-13 | — | Paste a column of 100 000 values / multi-line paste | Applied in one transaction; progress; error on first bad row with row number | 2E | manual M-11; to automate | P1 |
| TBL-14 | — | Table pane when layer removed, renamed, or project reopened | Pane shows empty state, no error boundary | 2E | to automate | P1 |
| TBL-15 | — | Node picker table (history) shows rows for a stale node | Stale badge, rows from cache | 2E | partial: TP 8.12 e2e M-18 | P2 |
| QRY-01 | — | Definition-query dialog: builder rows with 0 rows, 200 rows, each operator on each type, `IS NULL`, `LIKE` with `%` and `'` | SQL safe; row-numbered errors | V | partial: TP 5.20 | P1 |
| QRY-02 | — | Query dialog on a layer with no fields (Table with only geom) | Builder disabled with message | V | to automate | P2 |

## 3.2 Node editor (`ModelPane`, `store/model.ts`, `nodes/`)

Existing: TP 6.1–6.24 (strong). New:

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| MDL-01 | — | Tree with 500 nodes / 1 000 edges; pan, zoom, box select, run | UI stays > 20 FPS; run order correct | 2E (perf) | to automate | P2 |
| MDL-02 | — | Connect a vector output to a raster input; raster to Buffer | Connection refused with a sentence (port rules) | V | auto: TP 6.4 | P1 |
| MDL-03 | — | Create a cycle by connecting last → first | Refused "would create a loop" | V | auto: TP 6.4 | P1 |
| MDL-04 | — | Undo after run; undo after deleting the layer the tree uses; undo while a run is active | History consistent; run not corrupted; B5 `lastOutputs` kept | V+2E | partial: TP 8.13 | P0 |
| MDL-05 | — | Redo stack cleared by a new edit; 1 000 undo steps memory | Cap defined; oldest dropped without error | V | to automate | P2 |
| MDL-06 | Save/Reopen | Save with unrun nodes, muted nodes, selection, extra Layer Input from another layer; reopen | Identical tree; stale markers recomputed; extra input resolved by layer id | V+E | partial: TP 6.2, 12.1 | P0 |
| MDL-07 | — | Project saved by an older version (no `lastOutputs`, missing node params) | Opens; migration message; defaults filled | V | partial: B5 `load()` test; to automate older fixtures | P0 |
| MDL-08 | — | Param inputs: tool param number with min/max, units, string with unicode, field picker when the upstream has no fields yet, enum with removed option | Inline validation; stale option reset with note | V+2E | partial: TP 6.17 (engine) | P1 |
| MDL-09 | — | Shortcuts: rebind to an OS reserved combo (⌘Q, Ctrl+W), IME composition | Rejected with explanation or works without closing window | V | partial: TP 6.19–6.20 | P2 |
| MDL-10 | — | Drag a layer from Contents onto a tree of another layer; drop a table layer into a raster input | Extra Layer Input node, port type check | 2E | manual M-14 | P1 |
| MDL-11 | — | Paste nodes copied from a different project / garbage clipboard JSON | Garbage ignored with message; no exception | V | to automate | P1 |
| MDL-12 | — | Add Menu search with 0 results, 1 000 chars, regex chars | "No tools match" | V | to automate | P2 |
| MDL-13 | — | Switching layers while tree runs | Run continues for the original tree; result applied to the right layer | 2E | to automate | P0 |
| MDL-14 | — | Dirty tracking: select node, move node, change view (should not dirty) vs param change (dirty) | Dirty marker exact | V | partial: TP 6.2, 6.14 | P1 |

## 3.3 Graph executor and cache (`run_graph`, `preview_output`, `clear_model_cache`, `prune_node_cache`, `set_node_cache_budget`)

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| EXE-01 | — | Graph with a failing middle node | Downstream skipped (greyed), failing node shows a readable error, upstream results kept | E | auto: TP 6.15 | P0 |
| EXE-02 | — | Graph with two Outputs / zero Outputs / disconnected branch | Defined behaviour with message; disconnected branch not run | E | to automate | P1 |
| EXE-03 | — | Diamond (A→B, A→C, B+C→Merge) | A runs once; deterministic order; result identical across 20 runs | E | to automate | P1 |
| EXE-04 | — | Same graph with the layer edited (cell edit, row delete, new field) | Cache key changes; no stale result | E | partial: TP 8.3; to automate edit invalidation | P0 |
| EXE-05 | — | Cache budget 0, 1 MB, huge; prune with pinned nodes | Eviction by budget; no pinned eviction; run correct without cache | E | auto: TP 8.4 | P1 |
| EXE-06 | — | Cache across projects: Save As then run | All `Cached` | E | auto: TP 8.1 | P1 |
| EXE-07 | — | Engine version bump | Snapshots dropped, run recomputes, no error | E | auto: TP 8.2 | P1 |
| EXE-08 | — | Layer input is a raster / table / mixed; input layer deleted; layer replaced by the node tree's own output (feedback) | Defined message ("a layer cannot use its own result as input") | E | to automate | P0 |
| EXE-09 | — | Node params JSON with missing keys, extra keys, wrong types, null, huge strings | Readable parameter error naming the setting | E | partial: TP 6.17; to automate wrong types | P0 |
| EXE-10 | — | `preview_output` on a node with unmet inputs / after cancel | Message; no leftover temp tables | E | to automate | P1 |
| EXE-11 | — | Run with 0-row input at every tool (see TOOL-G03) | Empty output with correct fields; no error | E | to automate | P0 |
| EXE-12 | — | Run, disk full/readonly project dir during snapshot | "Could not store the result of <node>" error; layer intact | E | to automate (readonly) | P2 |
| EXE-13 | — | Result CRS flows through chain Project→Buffer→Clip | Final CRS equals expected; changing the Project target changes downstream | E | partial: TP 2.14, 7 Project | P0 |

## 3.4 Attribute history ("git for tables", `node_*`, `drop_node_diff`)

Existing: TP 8.1–8.13. New:

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| HIS-01 | — | Diff nodes of different CRS; with one side 0 rows; identical tables; no key available | CRS mismatch skips geometry diff with notice; empty = all added; identical = zero chips; no key → content diff | E | partial: TP 8.6; to automate empty/identical | P1 |
| HIS-02 | 1 M rows | Diff by key; cancel; open another diff | Cancel stops < 2 s; old diff tables dropped | E (perf) | auto: TP 8.9, 8.10 | P1 |
| HIS-03 | — | Diff duplicates in key; NULL keys; key of mixed type | Message "key is not unique: 12 duplicates" | E | to automate | P1 |
| HIS-04 | — | Diff while project is Saved/closed | Diff tables dropped; UI shows empty state | E | auto: TP 8.8 | P2 |

## 3.5 Progress and cancel (`progress`, `cancel_task`)

Existing: TP 13.1–13.13. New:

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| PRG-01 | — | `progress` polled during 3 tasks; one finishes between polls | Finished task disappears, others unaffected | E | partial: TP 13.1 | P1 |
| PRG-02 | — | `cancel_task` with unknown id / already-finished id / id of a non-cancellable task | `Ok` no-op or "task is not cancellable" | E | partial: TP 13.3 | P1 |
| PRG-03 | — | Cancel twice | Idempotent | E | to automate | P2 |
| PRG-04 | — | Cancel every long operation: import, export, tile build, model step (each tool category), TIGER, TNM, 3DEP, layout render/export, diff, calculate field, select by attributes | Every one stops < 2 s (or documented larger bound) and leaves no partial output; status shows "Cancelled" not an error | E | partial: TP 13.6–13.12; gaps: calculate field, select, export, import (ignored) | P0 |
| PRG-05 | — | Progress UI with 10 tasks; very long task title; task failing | Status bar collapses to "10 tasks"; message wraps | 2E | partial: TP 13.13 (M-25 e2e) | P2 |
| PRG-06 | — | App quit while tasks run | Prompt "tasks are running"; quitting doesn't corrupt project DB | M | manual | P0 |
| PRG-07 | — | Progress never regresses / never > 100 % in UI | Clamped | V | partial: TP 13.2, 13.5 | P2 |

## 3.6 Concurrency (cross-cutting)

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| CON-01 | — | Two imports at once | Both finish; unique ids; no deadlock | E | to automate | P0 |
| CON-02 | — | Two downloads at once (mock) | Independent progress and cancel | E | to automate (needs HTTP-MOCK) | P1 |
| CON-03 | Layer A | Run tree on A while running tree on B | Both finish; DuckDB connection use serialised without a visible freeze > 1 s on UI calls | E | to automate | P0 |
| CON-04 | — | Edit table / delete rows / rename layer while the layer's tree runs | Run completes or is cancelled with message; no stale overwrite | E | partial: TP 3.5 | P0 |
| CON-05 | — | Save project while a model runs / tile build runs / download runs | Save waits or says "finish running tasks"; DB consistent after reopen | E | to automate | P0 |
| CON-06 | — | Open project while tasks run | Tasks cancelled first, no cross-project layers | E | to automate | P0 |
| CON-07 | — | 50 concurrent tile requests + 5 IPC reads | UI IPC latency < 500 ms | E | to automate | P1 |
| CON-08 | — | Raster tools from 8 threads at once (GDAL thread-safety; M4 gate) | No SIGSEGV | E | pending feature (M4 on hold: P17 stress test) | P1 |
| CON-09 | — | Layout export while model runs and map tiles build | All complete; export content matches data at start | E | to automate | P1 |
