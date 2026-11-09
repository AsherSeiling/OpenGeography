//! Attribute editing: cell edits, add/rename/delete fields, calculate fields
//! (including geometry), delete rows. Edits apply to a layer's own data.
//!
//! A layer's overview tables (used for zoomed-out tiles) carry copies of its
//! attributes, so every edit is mirrored there too.

use crate::db::{ident, lit};
use crate::layers::{self, Field, LayerInfo, HIDDEN_COLS};
use crate::measure;
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;

/// Field types offered in the UI → DuckDB types.
pub const FIELD_TYPES: &[(&str, &str)] = &[
    ("text", "VARCHAR"),
    ("integer", "BIGINT"),
    ("double", "DOUBLE"),
    ("date", "DATE"),
    ("boolean", "BOOLEAN"),
];

pub fn sql_type(ty: &str) -> Result<&'static str> {
    FIELD_TYPES
        .iter()
        .find(|(k, v)| k.eq_ignore_ascii_case(ty) || v.eq_ignore_ascii_case(ty))
        .map(|(_, v)| *v)
        .ok_or_else(|| anyhow!("Unknown field type '{ty}'"))
}

fn editable(info: &LayerInfo) -> Result<()> {
    if info.editing {
        bail!("\"{}\" is being edited. Save or discard the shape edits first, then change its attributes or fields.", info.name);
    }
    if info.result_of.is_some() {
        bail!("This is a node-tree result, which is rebuilt on every run. Edit the original layer, or Save as new layer first.");
    }
    Ok(())
}

fn check_name(info: &LayerInfo, name: &str, must_exist: bool) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Field name is empty");
    }
    if name.eq_ignore_ascii_case("geom") || HIDDEN_COLS.contains(&name) {
        bail!("'{name}' is reserved");
    }
    let exists = info.fields.iter().any(|f| f.name.eq_ignore_ascii_case(name));
    match (must_exist, exists) {
        (true, false) => bail!("Field '{name}' not found"),
        (false, true) => bail!("A field named '{name}' already exists"),
        _ => Ok(()),
    }
}

fn field<'a>(info: &'a LayerInfo, name: &str) -> Result<&'a Field> {
    info.fields.iter().find(|f| f.name == name).ok_or_else(|| anyhow!("Field '{name}' not found"))
}

/// The base table plus its overview tables.
pub(crate) fn tables(info: &LayerInfo) -> Vec<String> {
    let mut t = vec![info.table.clone()];
    t.extend(info.overviews.iter().map(|z| layers::overview_table(&info.table, *z)));
    t
}

/// Run a schema change. DuckDB can't ALTER a table that has an index, so the
/// spatial indexes are dropped first and rebuilt afterwards.
fn alter(conn: &Connection, info: &LayerInfo, change: impl Fn(&str) -> String) -> Result<()> {
    let indexed = info.has_geometry();
    for t in tables(info) {
        let idx = ident(&format!("{t}_rtree"));
        if indexed {
            conn.execute_batch(&format!("DROP INDEX IF EXISTS {idx}"))?;
        }
        let res = conn.execute_batch(&change(&t));
        if indexed {
            conn.execute_batch(&format!("CREATE INDEX {idx} ON {} USING RTREE (__g)", ident(&t)))?;
        }
        res?;
    }
    Ok(())
}

/// Copy one column's values from the base table into the overview tables.
fn sync_overviews(conn: &Connection, info: &LayerInfo, column: &str) -> Result<()> {
    for z in &info.overviews {
        let ov = layers::overview_table(&info.table, *z);
        conn.execute_batch(&format!(
            "UPDATE {o} SET {c} = b.{c} FROM {b} b WHERE {o}.__fid = b.__fid",
            o = ident(&ov),
            b = ident(&info.table),
            c = ident(column)
        ))?;
    }
    Ok(())
}

/// After an edit: refresh field list and counts, and bump the version so
/// cached tiles and table pages are refreshed.
fn touch(conn: &Connection, info: &mut LayerInfo) -> Result<()> {
    layers::refresh_meta(conn, info)?;
    info.version += 1;
    Ok(())
}

pub fn add_field(conn: &Connection, info: &mut LayerInfo, name: &str, ty: &str) -> Result<()> {
    editable(info)?;
    check_name(info, name, false)?;
    let t = sql_type(ty)?;
    alter(conn, info, |table| format!("ALTER TABLE {} ADD COLUMN {} {t}", ident(table), ident(name.trim())))?;
    touch(conn, info)
}

pub fn delete_field(conn: &Connection, info: &mut LayerInfo, name: &str) -> Result<()> {
    editable(info)?;
    check_name(info, name, true)?;
    alter(conn, info, |table| format!("ALTER TABLE {} DROP COLUMN {}", ident(table), ident(name)))?;
    touch(conn, info)
}

pub fn rename_field(conn: &Connection, info: &mut LayerInfo, old: &str, new: &str) -> Result<()> {
    editable(info)?;
    check_name(info, old, true)?;
    check_name(info, new, false)?;
    alter(conn, info, |table| format!("ALTER TABLE {} RENAME COLUMN {} TO {}", ident(table), ident(old), ident(new.trim())))?;
    touch(conn, info)
}

/// Set one cell. `value` is text as typed; it is converted to the field's
/// type (an error explains values that don't fit). `None` sets NULL.
pub fn update_cell(conn: &Connection, info: &mut LayerInfo, fid: i64, column: &str, value: Option<&str>) -> Result<()> {
    editable(info)?;
    let f = field(info, column)?;
    let v = match value {
        None => "NULL".to_string(),
        Some(s) if s.trim().is_empty() && !f.ty.eq_ignore_ascii_case("VARCHAR") => "NULL".to_string(),
        Some(s) => {
            let cast = format!("CAST({} AS {})", lit(s.trim()), f.ty);
            conn.query_row(&format!("SELECT {cast}"), [], |_| Ok(()))
                .map_err(|_| anyhow!("'{s}' is not a valid {} value for {column}", friendly_type(&f.ty)))?;
            cast
        }
    };
    for t in tables(info) {
        conn.execute_batch(&format!("UPDATE {} SET {} = {v} WHERE __fid = {fid}", ident(&t), ident(column)))?;
    }
    touch(conn, info)
}

fn friendly_type(t: &str) -> &'static str {
    let t = t.to_uppercase();
    if t.contains("INT") {
        "whole number"
    } else if t.contains("DOUBLE") || t.contains("FLOAT") || t.contains("DECIMAL") || t.contains("REAL") {
        "number"
    } else if t.contains("DATE") || t.contains("TIME") {
        "date"
    } else if t.contains("BOOL") {
        "true/false"
    } else {
        "text"
    }
}

/// Calculate a field from an SQL expression, creating the field if needed
/// (its type is inferred from the expression, or `new_type` if given).
/// When `fids` is set, only those rows change (e.g. the selection).
pub fn calculate(conn: &Connection, info: &mut LayerInfo, column: &str, expr: &str, new_type: Option<&str>, fids: Option<&[i64]>) -> Result<i64> {
    editable(info)?;
    let column = column.trim();
    let t = ident(&info.table);
    // Validate the expression before changing anything.
    let inferred: String = conn
        .query_row(&format!("SELECT typeof(({expr})) FROM {t} LIMIT 1"), [], |r| r.get(0))
        .or_else(|e| match e {
            duckdb::Error::QueryReturnedNoRows => Ok("VARCHAR".to_string()),
            e => Err(e),
        })
        .map_err(|e| anyhow!("{}", e.to_string().split("\n\nLINE").next().unwrap_or("Invalid expression")))?;
    if !info.fields.iter().any(|f| f.name == column) {
        let ty = match new_type {
            Some(ty) => sql_type(ty)?.to_string(),
            None if inferred == "NULL" => "VARCHAR".to_string(),
            None => inferred,
        };
        check_name(info, column, false)?;
        alter(conn, info, |table| format!("ALTER TABLE {} ADD COLUMN {} {ty}", ident(table), ident(column)))?;
        layers::refresh_meta(conn, info)?;
    }
    let ty = field(info, column)?.ty.clone();
    let filter = match fids {
        Some([]) => return Ok(0),
        Some(ids) => format!("WHERE __fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")),
        None => String::new(),
    };
    let n = conn
        .execute(&format!("UPDATE {t} SET {c} = CAST(({expr}) AS {ty}) {filter}", c = ident(column)), [])
        .map_err(|e| anyhow!("{}", e.to_string().split("\n\nLINE").next().unwrap_or("Calculation failed")))?;
    sync_overviews(conn, info, column)?;
    touch(conn, info)?;
    Ok(n as i64)
}

/// Calculate a field with a Python script (`def calculate(row): return value`) over the attributes of every feature (or of `fids`). The
/// script is approved first (`pyscript::is_approved`) and runs before anything is changed, so a failure leaves the table as it was. Values
/// must fit the field (a new field's type comes from `new_type`, or from what the script returns): a row that raises or returns something that
/// doesn't fit is a failed row. By default any failed row stops the whole calculation; with `skip_failed` those rows keep their old value and
/// are reported in the returned note. `None` writes a real NULL. Returns the number of rows written and that note.
pub fn calculate_python(conn: &Connection, info: &mut LayerInfo, column: &str, script: &str, new_type: Option<&str>, fids: Option<&[i64]>, skip_failed: bool) -> Result<(i64, Option<String>)> {
    use crate::pyscript::{self, Mode, OutType};
    editable(info)?;
    let column = column.trim();
    let existing = info.fields.iter().find(|f| f.name == column).map(|f| f.ty.clone());
    let want = match &existing {
        Some(ty) => match ty.to_ascii_uppercase().as_str() {
            "BIGINT" | "INTEGER" | "SMALLINT" | "TINYINT" | "HUGEINT" => OutType::Integer,
            "DOUBLE" | "FLOAT" | "REAL" => OutType::Double,
            t if t.starts_with("DECIMAL") => OutType::Double,
            "VARCHAR" => OutType::Text,
            "BOOLEAN" => OutType::Boolean,
            "DATE" => OutType::Date,
            "TIMESTAMP" => OutType::Timestamp,
            other => bail!("The field '{column}' is {other}, which a Python script can't write. Choose another field or a new one."),
        },
        None => match new_type.map(|t| sql_type(t)).transpose()? {
            Some("BIGINT") => OutType::Integer,
            Some("DOUBLE") => OutType::Double,
            Some("VARCHAR") => OutType::Text,
            Some("BOOLEAN") => OutType::Boolean,
            Some("DATE") => OutType::Date,
            Some("TIMESTAMP") => OutType::Timestamp,
            Some(other) => bail!("A Python script can't fill a {other} field"),
            None => OutType::Auto,
        },
    };
    if matches!(fids, Some([])) {
        return Ok((0, None));
    }
    if existing.is_none() {
        check_name(info, column, false)?;
    }
    let ev = pyscript::evaluate_layer(conn, info, script, Mode::Field, want, skip_failed, fids, true)?;
    let sql_ty = match (&existing, ev.out_type) {
        (Some(t), _) => t.clone(),
        (None, OutType::Integer) => "BIGINT".into(),
        (None, OutType::Double) => "DOUBLE".into(),
        (None, OutType::Text | OutType::Auto) => "VARCHAR".into(),
        (None, OutType::Boolean) => "BOOLEAN".into(),
        (None, OutType::Date) => "DATE".into(),
        (None, OutType::Timestamp) => "TIMESTAMP".into(),
    };
    let result = (|| -> Result<(i64, Option<String>)> {
        if existing.is_none() {
            alter(conn, info, |table| format!("ALTER TABLE {} ADD COLUMN {} {sql_ty}", ident(table), ident(column)))?;
            layers::refresh_meta(conn, info)?;
        }
        let skip_failed_rows = if ev.failed_rns.is_empty() { String::new() } else { format!(" WHERE s.__rn NOT IN ({})", ev.failed_rns.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")) };
        let n = conn.execute(
            &format!(
                "UPDATE {t} SET {c} = CAST(m.v AS {sql_ty}) FROM (SELECT s.__fid AS fid, {value} AS v FROM {snap} s LEFT JOIN {stage} o USING (__rn){skip_failed_rows}) m WHERE {t}.__fid = m.fid",
                t = ident(&info.table),
                c = ident(column),
                value = ev.out_type.value_sql("o"),
                snap = ident(&ev.snapshot),
                stage = ident(&ev.staging),
            ),
            [],
        )?;
        sync_overviews(conn, info, column)?;
        touch(conn, info)?;
        let note = (ev.failures.count > 0).then(|| format!("{}. Those rows keep their old value.", ev.failures.report(ev.total)));
        Ok((n as i64, note))
    })();
    let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}; DROP TABLE IF EXISTS {}", ident(&ev.snapshot), ident(&ev.staging)));
    result
}

/// Calculate a geodesic geometry property into a field (created if needed).
pub fn calculate_geometry(conn: &Connection, info: &mut LayerInfo, column: &str, property: &str, area_units: &str, length_units: &str, fids: Option<&[i64]>) -> Result<i64> {
    let crs = info.crs.clone().ok_or_else(|| anyhow!("The layer has no coordinate system. Use Define Projection first."))?;
    let expr = measure::geometry_expr(&crs, property, area_units, length_units)?;
    calculate(conn, info, column, &expr, Some("double"), fids)
}

fn table_only(info: &LayerInfo, what: &str) -> Result<()> {
    if info.has_geometry() || info.raster.is_some() {
        bail!("{what} works on tables. A map layer's features have shapes, which a blank row doesn't; add features by drawing, importing or copying them.");
    }
    editable(info)
}

/// One row of a table as text, for undoing a delete.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct RowData {
    pub fid: i64,
    pub values: Vec<Option<String>>,
}

/// A new blank row at the end of a table (every field empty). Returns its row id.
pub fn add_row(conn: &Connection, info: &mut LayerInfo) -> Result<i64> {
    table_only(info, "Adding a row")?;
    let fid: i64 = conn.query_row(&format!("SELECT coalesce(max(__fid), 0) + 1 FROM {}", ident(&info.table)), [], |r| r.get(0))?;
    conn.execute_batch(&format!("INSERT INTO {} (__fid) VALUES ({fid})", ident(&info.table)))?;
    touch(conn, info)?;
    Ok(fid)
}

/// The rows with these ids, every field as text in the order of `info.fields` (NULL stays None).
pub fn rows_by_fid(conn: &Connection, info: &LayerInfo, fids: &[i64]) -> Result<Vec<RowData>> {
    if fids.is_empty() {
        return Ok(vec![]);
    }
    let cols = info.fields.iter().map(|f| format!("CAST({} AS VARCHAR)", ident(&f.name))).collect::<Vec<_>>();
    let list = fids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let sql = format!("SELECT __fid{}{} FROM {} WHERE __fid IN ({list}) ORDER BY __fid", if cols.is_empty() { "" } else { ", " }, cols.join(", "), ident(&info.table));
    let mut stmt = conn.prepare(&sql)?;
    let n = info.fields.len();
    let rows = stmt.query_map([], |r| {
        let mut values = Vec::with_capacity(n);
        for i in 0..n {
            values.push(r.get::<_, Option<String>>(i + 1)?);
        }
        Ok(RowData { fid: r.get(0)?, values })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Put rows back (undoing a delete) with their old ids. A row id that is in use is refused.
pub fn restore_rows(conn: &Connection, info: &mut LayerInfo, rows: &[RowData]) -> Result<i64> {
    table_only(info, "Restoring rows")?;
    for r in rows {
        if r.values.len() != info.fields.len() {
            bail!("The saved rows no longer match the table's fields.");
        }
        let taken: i64 = conn.query_row(&format!("SELECT count(*) FROM {} WHERE __fid = {}", ident(&info.table), r.fid), [], |x| x.get(0))?;
        if taken > 0 {
            bail!("Row {} is already in use, so the deleted row can't be put back.", r.fid);
        }
    }
    for r in rows {
        let vals = info.fields.iter().zip(&r.values).map(|(f, v)| match v {
            None => "NULL".to_string(),
            Some(s) => format!("CAST({} AS {})", lit(s), f.ty),
        });
        let names = info.fields.iter().map(|f| ident(&f.name)).collect::<Vec<_>>();
        conn.execute_batch(&format!(
            "INSERT INTO {} (__fid{}{}) VALUES ({}{}{})",
            ident(&info.table),
            if names.is_empty() { "" } else { ", " },
            names.join(", "),
            r.fid,
            if names.is_empty() { "" } else { ", " },
            vals.collect::<Vec<_>>().join(", ")
        ))?;
    }
    touch(conn, info)?;
    Ok(rows.len() as i64)
}

pub fn delete_rows(conn: &Connection, info: &mut LayerInfo, fids: &[i64]) -> Result<i64> {
    editable(info)?;
    if fids.is_empty() {
        return Ok(0);
    }
    let list = fids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let mut n = 0;
    for (i, t) in tables(info).iter().enumerate() {
        let c = conn.execute(&format!("DELETE FROM {} WHERE __fid IN ({list})", ident(t)), [])?;
        if i == 0 {
            n = c as i64;
        }
    }
    touch(conn, info)?;
    Ok(n)
}
