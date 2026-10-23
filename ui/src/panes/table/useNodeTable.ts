// Node mode of the table pane: loads a node's snapshot table, and (optionally)
// a diff against another node. The diff table is dropped whenever the pane
// closes it, switches nodes or changes the compare target.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, errorText, type DiffKind, type DiffRow, type DiffSummary, type Field, type NodeTableInfo, type RowsPage } from "../../ipc/api";
import { nodeSignature, useModel, useRun, type Tree } from "../../store/model";
import { useApp } from "../../store/store";
import { diffTotal, isDiffGone, isNotStored, nodeTable, pickerEntries, useTableTarget, type PickerEntry } from "./history";

export type NodePage = RowsPage & { diff?: DiffRow[] };
export type KindFilter = "all" | DiffKind;

export interface NodeTable {
  /** A node is selected (node mode). The picker fields below are valid either way. */
  active: boolean;
  tree: Tree | undefined;
  nodeId: string | null;
  label: string;
  entries: PickerEntry[];
  table: string | null;
  compareId: string | null;
  compareLabel: string | null;
  /** The table can't be read: never run, or evicted. */
  missing: { nodeId: string; label: string; why: string } | null;
  info: NodeTableInfo | null;
  diff: DiffSummary | null;
  comparing: boolean;
  diffError: string | null;
  kind: KindFilter;
  setKind: (k: KindFilter) => void;
  key: string | null;
  setKey: (k: string | null) => void;
  /** Re-run the comparison (e.g. after "no longer available"). */
  recompare: () => void;
  /** Columns the grid shows: the diff's field order when comparing. */
  fields: Field[];
  total: number;
  fetchPage: (offset: number, limit: number, sort: { field: string; desc: boolean } | null, filter: string) => Promise<NodePage>;
  /** Handle a failed page read; returns true if it was a recoverable "stored/diff" loss. */
  onFetchError: (e: unknown) => boolean;
  /** Key for the grid's query cache. */
  queryKey: string;
}

export function useNodeTable(owner: string | undefined): NodeTable {
  const rawTarget = useTableTarget((s) => s.target);
  const target = rawTarget && rawTarget.tree === owner ? rawTarget : null;
  const openLayer = useModel((s) => s.layer);
  const nodes = useModel((s) => s.nodes);
  const edges = useModel((s) => s.edges);
  const trees = useModel((s) => s.trees);
  const results = useRun((s) => s.results);
  const specs = useRun((s) => s.specs);
  const tree = useMemo<Tree | undefined>(() => (!owner ? undefined : owner === openLayer ? { nodes, edges } : trees[owner]), [owner, openLayer, nodes, edges, trees]);
  const node = tree?.nodes.find((n) => n.id === target?.nodeId);

  // The target is gone (node deleted, tree removed): fall back to the layer's table.
  useEffect(() => {
    if (target && tree && !node) useTableTarget.getState().set(null);
  }, [target, node, tree]);

  // Nodes of the open tree edited since they last ran: their table is out of date.
  const ranSig = useRun((s) => s.ranSig);
  const layers = useApp((s) => s.layers);
  const changed = useMemo<ReadonlySet<string>>(() => {
    const ids = new Set<string>();
    if (target?.tree === openLayer) {
      const memo = new Map<string, string>();
      for (const n of nodes) if (ranSig[n.id] && nodeSignature(n.id, memo) !== ranSig[n.id]) ids.add(n.id);
    }
    return ids;
  }, [target?.tree, openLayer, nodes, edges, layers, ranSig]);
  const entries = useMemo(() => (tree ? pickerEntries(tree, results, specs, changed) : []), [tree, results, specs, changed]);
  const table = node ? nodeTable(node, results[node.id]) : null;
  const compareNode = tree?.nodes.find((n) => n.id === target?.compareId);
  const compareTable = compareNode ? nodeTable(compareNode, results[compareNode.id]) : null;
  const labelOf = (id: string | null | undefined) => entries.find((e) => e.id === id)?.label ?? id ?? "";

  const [info, setInfo] = useState<NodeTableInfo | null>(null);
  const [compareInfo, setCompareInfo] = useState<NodeTableInfo | null>(null);
  const [lost, setLost] = useState<{ nodeId: string; why: string } | null>(null);
  // A diff is only valid for the inputs it was made from; between a change of inputs and the effect below
  // replacing it, the old one (already dropped in the engine) must not be read.
  const [made, setMade] = useState<{ sig: string; s: DiffSummary } | null>(null);
  const [comparing, setComparing] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [kind, setKind] = useState<KindFilter>("all");
  const [key, setKey] = useState<string | null>(null);
  const [nonce, setNonce] = useState(0);
  const retried = useRef(false);

  const nodeId = target?.nodeId ?? null;
  const sig = JSON.stringify([compareTable, table, key, nonce]);
  const diff = made && made.sig === sig ? made.s : null;
  const compareId = target?.compareId ?? null;
  // New node or compare target: start from a clean state.
  useEffect(() => {
    setKind("all");
    setKey(null);
    setLost(null);
  }, [nodeId, compareId, table, compareTable]);

  useEffect(() => {
    setInfo(null);
    if (!nodeId || !table) return;
    let live = true;
    api
      .nodeTableInfo(table)
      .then((i) => live && setInfo(i))
      .catch((e) => live && isNotStored(errorText(e)) && setLost({ nodeId, why: "is not stored" }));
    return () => {
      live = false;
    };
  }, [nodeId, table, nonce]);

  useEffect(() => {
    setCompareInfo(null);
    if (!compareTable) return;
    let live = true;
    api.nodeTableInfo(compareTable).then((i) => live && setCompareInfo(i)).catch(() => {});
    return () => {
      live = false;
    };
  }, [compareTable, nonce]);

  // The diff lives only while this effect does: closing it, switching nodes or
  // changing the compare target drops the engine's diff table.
  useEffect(() => {
    setMade(null);
    setDiffError(null);
    if (!compareId || !table || !compareTable) {
      setComparing(false);
      return;
    }
    let live = true;
    let madeId: string | null = null;
    setComparing(true);
    api
      .nodeDiff(compareTable, table, key)
      .then((s) => {
        if (!live) {
          api.dropNodeDiff(s.diffId).catch(() => {});
          return;
        }
        madeId = s.diffId;
        setMade({ sig, s });
      })
      .catch((e) => {
        if (!live) return;
        const msg = errorText(e).split("\n")[0];
        if (isNotStored(msg)) setLost({ nodeId: compareId, why: "is not stored" });
        else setDiffError(msg);
      })
      .finally(() => live && setComparing(false));
    return () => {
      live = false;
      if (madeId) api.dropNodeDiff(madeId).catch(() => {});
    };
  }, [compareId, table, compareTable, key, nonce]);

  const diffId = diff?.diffId;
  const typeOf = useMemo(() => {
    const m = new Map<string, string>();
    for (const f of [...(compareInfo?.fields ?? []), ...(info?.fields ?? [])]) m.set(f.name.toLowerCase(), f.type);
    return m;
  }, [info, compareInfo]);
  const fields = useMemo<Field[]>(() => (diff ? diff.fields.map((name) => ({ name, type: name === "geom" ? "GEOMETRY" : (typeOf.get(name.toLowerCase()) ?? "VARCHAR") })) : (info?.fields ?? [])), [diff, info, typeOf]);

  const fetchPage: NodeTable["fetchPage"] = useCallback(
    async (offset, limit, sort, filter) => {
      if (diffId && diff) {
        const rows = await api.nodeDiffRows(diffId, kind, offset, limit);
        retried.current = false;
        return {
          total: diffTotal(diff, kind),
          fids: rows.map((_, i) => offset + i),
          rows: rows.map((r) => r.values.map((v) => (r.kind === "removed" ? v.a : v.b))),
          diff: rows,
        };
      }
      return api.nodeRows(table!, { offset, limit, sort: sort?.field, desc: sort?.desc, filter: filter || null });
    },
    [diffId, diff, kind, table],
  );

  const onFetchError = useCallback(
    (e: unknown) => {
      const msg = errorText(e).split("\n")[0];
      if (isNotStored(msg)) {
        setLost({ nodeId: nodeId!, why: "is not stored" });
        return true;
      }
      if (isDiffGone(msg)) {
        // Project reopened or saved for the first time: compare again, once.
        if (!retried.current) {
          retried.current = true;
          setNonce((n) => n + 1);
        } else setDiffError("The comparison was closed when the project was reopened or saved.");
        return true;
      }
      return false;
    },
    [nodeId],
  );

  if (!target || !tree || !node) {
    return {
      active: false, tree, nodeId: null, label: "", entries, table: null, compareId: null, compareLabel: null, missing: null, info: null, diff: null,
      comparing: false, diffError: null, kind, setKind, key, setKey, recompare: () => {}, fields: [], total: 0, fetchPage, onFetchError, queryKey: "",
    };
  }
  const neverRun = !table;
  const missing = lost ?? (neverRun ? { nodeId: node.id, why: "has not been run yet" } : null);
  return {
    active: true,
    tree,
    nodeId: node.id,
    label: labelOf(node.id),
    entries,
    table,
    compareId,
    compareLabel: compareId ? labelOf(compareId) : null,
    missing: missing ? { nodeId: missing.nodeId, label: labelOf(missing.nodeId), why: missing.why } : compareId && !compareTable ? { nodeId: compareId, label: labelOf(compareId), why: "has not been run yet" } : null,
    info,
    diff,
    comparing,
    diffError,
    kind,
    setKind,
    key,
    setKey,
    recompare: () => {
      retried.current = false;
      setLost(null);
      setNonce((n) => n + 1);
    },
    fields,
    total: diff ? diffTotal(diff, kind) : (info?.rows ?? 0),
    fetchPage,
    onFetchError,
    queryKey: JSON.stringify([table, compareTable, diffId ?? null, kind, nonce]),
  };
}
