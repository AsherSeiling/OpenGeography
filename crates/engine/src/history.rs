//! Attribute history: read a model node's snapshot table and compare two of
//! them (docs/ATTRIBUTE_HISTORY_PLAN.md D5–D7).
//!
//! Snapshots are the `__node_<hash>` tables (or, for Layer Input, views) of the
//! model cache. A diff is tool-agnostic: rows are matched by a key field
//! (chosen by the user, or detected), and with no usable key the tables are
//! compared as multisets of row hashes. The result lands in a temporary
//! `__diff_*` table that is never indexed in `__node_cache` and is dropped when
//! a project is opened or started.

use crate::db::{ident, lit};
use crate::progress::TaskHandle;
use crate::table::{RowsPage, RowsQuery};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const DIFF_PREFIX: &str = "__diff_";

#[derive(Serialize, Debug, Clone)]
pub struct FieldDesc {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NodeTableInfo {
    pub fields: Vec<FieldDesc>,
    pub rows: i64,
    pub crs: Option<String>,
    pub has_geom: bool,
    /// `table` or `view` (Layer Input snapshots are views).
    pub kind: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Retyped {
    pub name: String,
    pub from: String,
    pub to: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub diff_id: String,
    pub key_used: Option<String>,
    /// `user`, `auto` or `none` (content diff).
    pub key_source: String,
    pub note: Option<String>,
    /// Field order of the rows `node_diff_rows` returns.
    pub fields: Vec<String>,
    pub fields_added: Vec<String>,
    pub fields_removed: Vec<String>,
    pub fields_retyped: Vec<Retyped>,
    pub rows_a: i64,
    pub rows_b: i64,
    pub added: i64,
    pub removed: i64,
    pub changed: i64,
    pub unchanged: i64,
    pub geometry_compared: bool,
}

#[derive(Serialize, Debug)]
pub struct DiffValue {
    pub field: String,
    pub a: Option<String>,
    pub b: Option<String>,
    pub changed: bool,
}

#[derive(Serialize, Debug)]
pub struct DiffRow {
    /// `added`, `removed` or `changed`.
    pub kind: String,
    pub key: Option<String>,
    pub values: Vec<DiffValue>,
}

/// What `node_diff_rows` needs to read a diff table back.
#[derive(Debug, Clone)]
pub struct DiffMeta {
    pub table: String,
    pub fields: Vec<String>,
}

fn check_node_table(t: &str) -> Result<()> {
    let ok = t.starts_with("__node_") && t != "__node_cache" && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok { Ok(()) } else { bail!("Not a model snapshot: {t}") }
}

fn describe(conn: &Connection, table: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(&format!("DESCRIBE SELECT * FROM {}", ident(table)))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    rows.collect::<Result<_, _>>().map_err(|_| anyhow!("Snapshot {table} is not stored (run the model to this node)"))
}

fn is_geom(name: &str, ty: &str) -> bool {
    name == "geom" && ty.to_uppercase().starts_with("GEOMETRY")
}

fn table_kind(conn: &Connection, table: &str) -> Result<String> {
    let ty: String = conn
        .query_row("SELECT table_type FROM information_schema.tables WHERE table_name = ?", [table], |r| r.get(0))
        .map_err(|_| anyhow!("Snapshot {table} is not stored (run the model to this node)"))?;
    Ok(if ty.to_uppercase().contains("VIEW") { "view" } else { "table" }.into())
}

fn indexed_crs(conn: &Connection, table: &str) -> Option<String> {
    conn.query_row("SELECT crs FROM __node_cache WHERE table_name = ?", [table], |r| r.get::<_, Option<String>>(0)).ok().flatten()
}

fn count(conn: &Connection, table: &str) -> Result<i64> {
    Ok(conn.query_row(&format!("SELECT count(*) FROM {}", ident(table)), [], |r| r.get(0))?)
}

pub fn table_info(conn: &Connection, table: &str) -> Result<NodeTableInfo> {
    check_node_table(table)?;
    let cols = describe(conn, table)?;
    let has_geom = cols.iter().any(|(n, t)| is_geom(n, t));
    Ok(NodeTableInfo {
        fields: cols.into_iter().filter(|(n, t)| !is_geom(n, t)).map(|(name, ty)| FieldDesc { name, ty }).collect(),
        rows: count(conn, table)?,
        crs: indexed_crs(conn, table),
        has_geom,
        kind: table_kind(conn, table)?,
    })
}

/// A page of a snapshot. Tables page by `rowid` (stable for a CREATE TABLE AS
/// result). Views (Layer Input) have no row id: unsorted, they page in
/// DuckDB's insertion order (`preserve_insertion_order` is on by default, so a
/// plain scan is deterministic); sorted, a `row_number()` breaks ties. The
/// page's `fids` are row positions in the snapshot, not layer fids.
pub fn node_rows(conn: &Connection, table: &str, q: &RowsQuery) -> Result<RowsPage> {
    check_node_table(table)?;
    let cols: Vec<String> = describe(conn, table)?.into_iter().filter(|(n, t)| !is_geom(n, t)).map(|(n, _)| n).collect();
    let is_view = table_kind(conn, table)? == "view";
    let t = ident(table);
    let w = match q.filter.as_ref().filter(|f| !f.trim().is_empty()) {
        Some(f) => format!("WHERE ({f})"),
        None => String::new(),
    };
    let sort = match &q.sort {
        Some(s) if cols.contains(s) => Some(format!("{} {} NULLS LAST", ident(s), if q.desc { "DESC" } else { "ASC" })),
        Some(s) => bail!("Unknown sort field '{s}'"),
        None => None,
    };
    let select_cols: Vec<String> = cols.iter().map(|c| format!("CAST({} AS VARCHAR)", ident(c))).collect();
    let limit = q.limit.clamp(1, 5000);
    let (source, pos, order) = match (is_view, &sort) {
        (false, _) => (format!("(SELECT rowid AS __pos, * FROM {t})"), "__pos", format!("ORDER BY {}__pos", sort.as_ref().map(|s| format!("{s}, ")).unwrap_or_default())),
        (true, s) => (
            format!("(SELECT row_number() OVER () - 1 AS __pos, * FROM {t})"),
            "__pos",
            format!("ORDER BY {}__pos", s.as_ref().map(|s| format!("{s}, ")).unwrap_or_default()),
        ),
    };
    let total: i64 = conn.query_row(&format!("SELECT count(*) FROM {source} {w}"), [], |r| r.get(0))?;
    let select = if select_cols.is_empty() { pos.to_string() } else { format!("{pos}, {}", select_cols.join(", ")) };
    let mut stmt = conn.prepare(&format!("SELECT {select} FROM {source} {w} {order} LIMIT {limit} OFFSET {}", q.offset))?;
    let n = cols.len();
    let (mut fids, mut rows) = (Vec::new(), Vec::new());
    let mut it = stmt.query([])?;
    while let Some(r) = it.next()? {
        fids.push(r.get::<_, i64>(0)?);
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            row.push(r.get::<_, Option<String>>(i + 1)?);
        }
        rows.push(row);
    }
    Ok(RowsPage { total, fids, rows })
}

/// Run `f`, interrupting the DuckDB connection while the task is cancelled
/// (same watcher as `Engine::run_graph`: DuckDB clears the flag at each query
/// start, so a single interrupt can miss).
pub fn cancellable<T>(conn: &Connection, handle: TaskHandle, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let interrupt = conn.interrupt_handle();
    let finished = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|s| {
        s.spawn(|| {
            while !finished.load(std::sync::atomic::Ordering::Relaxed) {
                if handle.check().is_err() {
                    interrupt.interrupt();
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        });
        let out = f();
        finished.store(true, std::sync::atomic::Ordering::Relaxed);
        out
    })
}

/// Drop every temporary diff table (project opened or started).
pub fn drop_all_diffs(conn: &Connection) {
    let Ok(mut stmt) = conn.prepare("SELECT table_name FROM information_schema.tables WHERE table_name LIKE '\\_\\_diff\\_%' ESCAPE '\\'") else { return };
    let names: Vec<String> = match stmt.query_map([], |r| r.get(0)).and_then(|it| it.collect::<Result<_, _>>()) {
        Ok(n) => n,
        Err(_) => return,
    };
    for n in names {
        let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&n)));
    }
}

pub fn drop_diff(conn: &Connection, table: &str) {
    let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(table)));
}

/// One compared field. `a`/`b` are the column names in each table (`None` when
/// the field exists on one side only).
struct Field {
    name: String,
    a: Option<(String, String)>,
    b: Option<(String, String)>,
}

impl Field {
    fn common(&self) -> bool {
        self.a.is_some() && self.b.is_some()
    }
    fn same_type(&self) -> bool {
        match (&self.a, &self.b) {
            (Some(a), Some(b)) => a.1 == b.1,
            _ => true,
        }
    }
}

/// Duplicate-or-empty values of a column: `count(*) - count(DISTINCT col)`
/// (0 means the column is a usable key: unique and non-null).
fn dup_count(conn: &Connection, table: &str, col: &str) -> Result<i64> {
    Ok(conn.query_row(&format!("SELECT count(*) - count(DISTINCT {}) FROM {}", ident(col), ident(table)), [], |r| r.get::<_, i64>(0))?)
}

fn sql_a(f: &Field) -> String {
    f.a.as_ref().map(|(n, _)| format!("a.{}", ident(n))).unwrap_or_else(|| "NULL".into())
}

fn sql_b(f: &Field) -> String {
    f.b.as_ref().map(|(n, _)| format!("b.{}", ident(n))).unwrap_or_else(|| "NULL".into())
}

fn differs(f: &Field) -> String {
    if f.same_type() {
        format!("({} IS DISTINCT FROM {})", sql_a(f), sql_b(f))
    } else {
        format!("(CAST({} AS VARCHAR) IS DISTINCT FROM CAST({} AS VARCHAR))", sql_a(f), sql_b(f))
    }
}

fn diff_id(a: &str, b: &str, key: Option<&str>) -> String {
    let short = |t: &str| t.trim_start_matches("__node_").to_string();
    let k = match key {
        Some(k) => {
            let h = hex::encode(&Sha256::digest(k.as_bytes())[..4]);
            let clean: String = k.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).take(24).collect();
            format!("{clean}_{h}")
        }
        None => "content".into(),
    };
    format!("{DIFF_PREFIX}{}_{}_{k}", short(a), short(b))
}

pub fn node_diff(conn: &Connection, a: &str, b: &str, user_key: Option<&str>, handle: TaskHandle, progress: &dyn Fn(f64, &str)) -> Result<(DiffSummary, DiffMeta)> {
    check_node_table(a)?;
    check_node_table(b)?;
    let (cols_a, cols_b) = (describe(conn, a)?, describe(conn, b)?);
    let (geom_a, geom_b) = (cols_a.iter().any(|(n, t)| is_geom(n, t)), cols_b.iter().any(|(n, t)| is_geom(n, t)));
    let plain = |cols: Vec<(String, String)>| -> Vec<(String, String)> { cols.into_iter().filter(|(n, t)| !is_geom(n, t)).collect() };
    let (cols_a, cols_b) = (plain(cols_a), plain(cols_b));
    let lower = |s: &str| s.to_lowercase();

    // Schema diff: names compare case-insensitively.
    let bmap: HashMap<String, (String, String)> = cols_b.iter().map(|c| (lower(&c.0), c.clone())).collect();
    let amap: HashMap<String, (String, String)> = cols_a.iter().map(|c| (lower(&c.0), c.clone())).collect();
    let mut fields: Vec<Field> = cols_a.iter().map(|c| Field { name: c.0.clone(), a: Some(c.clone()), b: bmap.get(&lower(&c.0)).cloned() }).collect();
    fields.extend(cols_b.iter().filter(|c| !amap.contains_key(&lower(&c.0))).map(|c| Field { name: c.0.clone(), a: None, b: Some(c.clone()) }));
    let fields_added: Vec<String> = fields.iter().filter(|f| f.a.is_none()).map(|f| f.name.clone()).collect();
    let fields_removed: Vec<String> = fields.iter().filter(|f| f.b.is_none()).map(|f| f.name.clone()).collect();
    let fields_retyped: Vec<Retyped> = fields.iter().filter(|f| f.common() && !f.same_type()).map(|f| Retyped { name: f.name.clone(), from: f.a.as_ref().unwrap().1.clone(), to: f.b.as_ref().unwrap().1.clone() }).collect();

    // Geometry is compared only between two geometry tables in one CRS.
    let (crs_a, crs_b) = (indexed_crs(conn, a), indexed_crs(conn, b));
    let mut notes: Vec<String> = Vec::new();
    let geometry_compared = if !(geom_a && geom_b) {
        if geom_a != geom_b {
            notes.push("Geometry not compared: only one of the tables has geometry.".into());
        }
        false
    } else if crs_a != crs_b {
        let show = |c: &Option<String>| c.clone().unwrap_or_else(|| "undefined".into());
        notes.push(format!("Geometry not compared: coordinate systems differ ({} → {}).", show(&crs_a), show(&crs_b)));
        false
    } else {
        true
    };

    handle.check()?;
    let rows_a = count(conn, a)?;
    let rows_b = count(conn, b)?;
    progress(0.1, "Choosing the matching key");

    // Key choice (D5): the user's field, else an auto-detected unique one.
    let find = |k: &str| fields.iter().position(|f| f.common() && lower(&f.name) == lower(k));
    let unique_in_both = |f: &Field| -> Result<(i64, i64)> { Ok((dup_count(conn, a, &f.a.as_ref().unwrap().0)?, dup_count(conn, b, &f.b.as_ref().unwrap().0)?)) };
    let mut key: Option<(usize, &str)> = None;
    match user_key.filter(|k| !k.trim().is_empty()) {
        Some(k) => {
            let Some(i) = find(k) else { bail!("Key field '{k}' must exist in both tables") };
            let (da, db) = unique_in_both(&fields[i])?;
            if da == 0 && db == 0 {
                key = Some((i, "user"));
            } else {
                for (side, d) in [("A", da), ("B", db)].into_iter().filter(|(_, d)| *d > 0) {
                    notes.push(format!("Key '{}' isn't unique in {side} ({d} duplicate or empty values).", fields[i].name));
                }
            }
        }
        None => {
            let preferred = |n: &str| lower(n).ends_with("id");
            let mut cands: Vec<usize> = (0..fields.len()).filter(|&i| fields[i].common()).collect();
            cands.sort_by_key(|&i| !preferred(&fields[i].name)); // stable: keeps field order within each group
            for i in cands.into_iter().take(5) {
                handle.check()?;
                if unique_in_both(&fields[i])? == (0, 0) {
                    key = Some((i, "auto"));
                    break;
                }
            }
        }
    }
    progress(0.3, "Comparing rows");

    let id = diff_id(a, b, key.map(|(i, _)| fields[i].name.as_str()));
    let (at, bt) = (ident(a), ident(b));
    let n = fields.len();
    let geom_field = geometry_compared;
    let tbl = ident(&id);
    let mut out_names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
    if geom_field {
        out_names.push("geom".into());
    }

    // Final projection shared by both modes: every column cast to VARCHAR.
    let final_cols = |src: &str| -> String {
        let mut cols = Vec::new();
        for i in 0..out_names.len() {
            let (ga, gb) = (format!("a_{i}"), format!("b_{i}"));
            if geom_field && i == n {
                cols.push(format!("left(ST_AsText({src}.{ga}), 80) AS {ga}, left(ST_AsText({src}.{gb}), 80) AS {gb}"));
            } else {
                cols.push(format!("CAST({src}.{ga} AS VARCHAR) AS {ga}, CAST({src}.{gb} AS VARCHAR) AS {gb}"));
            }
            cols.push(format!("{src}.c_{i} AS c_{i}"));
        }
        cols.join(", ")
    };

    let (key_used, key_source, sql) = match key {
        Some((ki, source)) => {
            let kf = &fields[ki];
            let on = if kf.same_type() {
                format!("{} = {}", sql_a(kf), sql_b(kf))
            } else {
                format!("CAST({} AS VARCHAR) = CAST({} AS VARCHAR)", sql_a(kf), sql_b(kf))
            };
            let mut inner = vec![format!("{} AS __ak", sql_a(kf)), format!("{} AS __bk", sql_b(kf))];
            let mut flags = Vec::new();
            for (i, f) in fields.iter().enumerate() {
                inner.push(format!("{} AS a_{i}, {} AS b_{i}", sql_a(f), sql_b(f)));
                inner.push(if f.common() { format!("{} AS c_{i}", differs(f)) } else { format!("FALSE AS c_{i}") });
                if f.common() {
                    flags.push(format!("c_{i}"));
                }
            }
            if geom_field {
                inner.push(format!("a.geom AS a_{n}, b.geom AS b_{n}"));
                inner.push(format!("(ST_AsWKB(a.geom) IS DISTINCT FROM ST_AsWKB(b.geom)) AS c_{n}"));
                flags.push(format!("c_{n}"));
            }
            let any = if flags.is_empty() { "FALSE".to_string() } else { flags.join(" OR ") };
            let sql = format!(
                "CREATE OR REPLACE TABLE {tbl} AS SELECT __kind, CAST(coalesce(CAST(__ak AS VARCHAR), CAST(__bk AS VARCHAR)) AS VARCHAR) AS __key, {fin} \
                 FROM (SELECT CASE WHEN __ak IS NULL THEN 'added' WHEN __bk IS NULL THEN 'removed' WHEN ({any}) THEN 'changed' ELSE 'unchanged' END AS __kind, * \
                       FROM (SELECT {inner} FROM {at} a FULL OUTER JOIN {bt} b ON {on})) j WHERE __kind <> 'unchanged'",
                fin = final_cols("j"),
                inner = inner.join(", ")
            );
            (Some(kf.name.clone()), source.to_string(), sql)
        }
        None => {
            notes.push("No matching key — showing rows that exist in only one table. Pick a key field to see changed rows.".into());
            // Row hash over the common columns (NULL gets its own sentinel).
            let side = |alias: &str, is_a: bool| -> String {
                let mut parts: Vec<String> = fields
                    .iter()
                    .filter(|f| f.common())
                    .map(|f| {
                        let c = if is_a { &f.a } else { &f.b };
                        format!("coalesce(CAST({alias}.{} AS VARCHAR), chr(1) || 'NULL' || chr(1))", ident(&c.as_ref().unwrap().0))
                    })
                    .collect();
                if geometry_compared {
                    parts.push(format!("coalesce(CAST(md5(ST_AsWKB({alias}.geom)::BLOB) AS VARCHAR), 'NULL')"));
                }
                if parts.is_empty() { "'row'".to_string() } else { format!("md5(concat_ws(chr(31), {}))", parts.join(", ")) }
            };
            let mut sel_a = vec![format!("{} AS __h", side("a", true))];
            let mut sel_b = vec![format!("{} AS __h", side("b", false))];
            let mut final_parts = Vec::new();
            for (i, f) in fields.iter().enumerate() {
                if f.a.is_some() {
                    sel_a.push(format!("{} AS a_{i}", sql_a(f)));
                }
                if f.b.is_some() {
                    sel_b.push(format!("{} AS b_{i}", sql_b(f)));
                }
                final_parts.push(format!(
                    "CAST({} AS VARCHAR) AS a_{i}, CAST({} AS VARCHAR) AS b_{i}, FALSE AS c_{i}",
                    if f.a.is_some() { format!("x.a_{i}") } else { "NULL".into() },
                    if f.b.is_some() { format!("y.b_{i}") } else { "NULL".into() }
                ));
            }
            if geom_field {
                sel_a.push(format!("a.geom AS a_{n}"));
                sel_b.push(format!("b.geom AS b_{n}"));
                final_parts.push(format!("left(ST_AsText(x.a_{n}), 80) AS a_{n}, left(ST_AsText(y.b_{n}), 80) AS b_{n}, FALSE AS c_{n}"));
            }
            let sql = format!(
                "CREATE OR REPLACE TABLE {tbl} AS \
                 WITH x1 AS (SELECT {sa} FROM {at} a), x AS (SELECT *, row_number() OVER (PARTITION BY __h) AS __n FROM x1), \
                      y1 AS (SELECT {sb} FROM {bt} b), y AS (SELECT *, row_number() OVER (PARTITION BY __h) AS __n FROM y1) \
                 SELECT CASE WHEN x.__h IS NULL THEN 'added' ELSE 'removed' END AS __kind, CAST(NULL AS VARCHAR) AS __key, {fin} \
                 FROM x FULL OUTER JOIN y ON x.__h = y.__h AND x.__n = y.__n WHERE x.__h IS NULL OR y.__h IS NULL",
                sa = sel_a.join(", "),
                sb = sel_b.join(", "),
                fin = final_parts.join(", ")
            );
            (None, "none".to_string(), sql)
        }
    };

    handle.check()?;
    // One transaction: a cancelled comparison leaves no half-built table.
    crate::atomically(conn, || {
        conn.execute_batch(&sql)?;
        Ok(())
    })?;
    progress(0.9, "Counting differences");
    let (mut added, mut removed, mut changed) = (0i64, 0i64, 0i64);
    {
        let mut stmt = conn.prepare(&format!("SELECT __kind, count(*) FROM {tbl} GROUP BY 1"))?;
        let mut it = stmt.query([])?;
        while let Some(r) = it.next()? {
            let (k, c): (String, i64) = (r.get(0)?, r.get(1)?);
            match k.as_str() {
                "added" => added = c,
                "removed" => removed = c,
                _ => changed = c,
            }
        }
    }
    let summary = DiffSummary {
        diff_id: id.clone(),
        key_used,
        key_source,
        note: if notes.is_empty() { None } else { Some(notes.join(" ")) },
        fields: out_names.clone(),
        fields_added,
        fields_removed,
        fields_retyped,
        rows_a,
        rows_b,
        added,
        removed,
        changed,
        unchanged: rows_a - removed - changed,
        geometry_compared,
    };
    Ok((summary, DiffMeta { table: id, fields: out_names }))
}

pub fn diff_rows(conn: &Connection, meta: &DiffMeta, kind: &str, offset: usize, limit: usize) -> Result<Vec<DiffRow>> {
    let w = match kind {
        "" | "all" => String::new(),
        "added" | "removed" | "changed" => format!("WHERE __kind = {}", lit(kind)),
        k => bail!("Unknown diff kind '{k}'"),
    };
    let n = meta.fields.len();
    let cols: Vec<String> = (0..n).map(|i| format!("a_{i}, b_{i}, c_{i}")).collect();
    let sql = format!("SELECT __kind, __key{}{} FROM {} {w} ORDER BY rowid LIMIT {} OFFSET {offset}", if n > 0 { ", " } else { "" }, cols.join(", "), ident(&meta.table), limit.clamp(1, 5000));
    let mut stmt = conn.prepare(&sql).map_err(|_| anyhow!("This comparison is no longer available; run it again"))?;
    let mut it = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(r) = it.next()? {
        let kind: String = r.get(0)?;
        let mut values = Vec::with_capacity(n);
        for (i, f) in meta.fields.iter().enumerate() {
            let c: bool = r.get::<_, Option<bool>>(2 + i * 3 + 2)?.unwrap_or(false);
            values.push(DiffValue { field: f.clone(), a: r.get(2 + i * 3)?, b: r.get(2 + i * 3 + 1)?, changed: kind == "changed" && c });
        }
        out.push(DiffRow { kind, key: r.get(1)?, values });
    }
    Ok(out)
}
