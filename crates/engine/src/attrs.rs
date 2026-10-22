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

fn sql_type(ty: &str) -> Result<&'static str> {
    FIELD_TYPES
        .iter()
        .find(|(k, v)| k.eq_ignore_ascii_case(ty) || v.eq_ignore_ascii_case(ty))
        .map(|(_, v)| *v)
        .ok_or_else(|| anyhow!("Unknown field type '{ty}'"))
}

fn editable(info: &LayerInfo) -> Result<()> {
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
fn tables(info: &LayerInfo) -> Vec<String> {
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

/// Calculate a geodesic geometry property into a field (created if needed).
pub fn calculate_geometry(conn: &Connection, info: &mut LayerInfo, column: &str, property: &str, area_units: &str, length_units: &str, fids: Option<&[i64]>) -> Result<i64> {
    let crs = info.crs.clone().ok_or_else(|| anyhow!("The layer has no coordinate system. Use Define Projection first."))?;
    let expr = measure::geometry_expr(&crs, property, area_units, length_units)?;
    calculate(conn, info, column, &expr, Some("double"), fids)
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
