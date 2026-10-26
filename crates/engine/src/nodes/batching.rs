//! Making huge single queries stoppable by Cancel (B-T4). DuckDB can only be
//! interrupted between its own work units, and two things made those units
//! very long (docs/DECISIONS.md "Slow cancel"):
//!  * Geodesic Buffer builds a PROJ projection per distinct centre. A row group
//!    holding features from all over the layer sets up hundreds of them, and
//!    all threads do it at once behind a lock (~12 s before the first
//!    interrupt check). Sorting the layer by that centre first gives each row
//!    group a handful of centres: the query is ~2× faster and stops in ~0.1 s.
//!  * Dissolve is one `ST_Union_Agg` finalize call. It is replaced by partial
//!    unions per spatial tile (one statement per tile), then pairwise unions of
//!    neighbouring tiles level by level (each a short query).
//! Tools return an `Output::Script`; the executor checks for Cancel between
//! statements. Results equal the single-query versions (tests/batching.rs).

use super::*;

/// Layers above this many features use the sorted geodesic Buffer path.
pub const BIG_BUFFER: i64 = 50_000;
/// Target features per spatial tile of a partitioned union.
const FEATURES_PER_TILE: i64 = 4_000;

pub fn row_count(conn: &Connection, table: &str) -> Result<i64> {
    Ok(conn.query_row(&format!("SELECT count(*) FROM {}", ident(table)), [], |r| r.get(0))?)
}

/// Tile grid for a partitioned union: SQL for the x and y cell of a geometry
/// (by the centre of its bounding box), the grid size, and the cells in use.
/// A partition only: any split gives the same union.
pub struct Tiles {
    pub x: String,
    pub y: String,
    pub grid: i64,
    pub cells: Vec<(i64, i64)>,
}

pub fn tiles(conn: &Connection, table: &str, rows: i64) -> Result<Tiles> {
    let t = ident(table);
    let (x0, y0, x1, y1): (Option<f64>, Option<f64>, Option<f64>, Option<f64>) = conn.query_row(
        &format!("SELECT min(ST_XMin(geom)), min(ST_YMin(geom)), max(ST_XMax(geom)), max(ST_YMax(geom)) FROM {t}"),
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let grid = (((rows as f64) / FEATURES_PER_TILE as f64).sqrt().ceil() as i64).max(1);
    let (x0, y0) = (x0.unwrap_or(0.0), y0.unwrap_or(0.0));
    let w = (x1.unwrap_or(1.0) - x0).max(1e-9);
    let h = (y1.unwrap_or(1.0) - y0).max(1e-9);
    let cell = |c: &str, o: f64, s: f64| format!("coalesce(least(floor(({c} - {o:?}) / {s:?} * {grid}), {})::BIGINT, 0)", grid - 1);
    let x = cell("(ST_XMin(geom) + ST_XMax(geom)) / 2", x0, w);
    let y = cell("(ST_YMin(geom) + ST_YMax(geom)) / 2", y0, h);
    let mut stmt = conn.prepare(&format!("SELECT DISTINCT {x}, {y} FROM {t} ORDER BY 1, 2"))?;
    let cells = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
    Ok(Tiles { x, y, grid, cells })
}

/// Statements that dissolve `"{out}_t"` (columns `__tx`, `__ty`, the optional
/// `field`, `geom`; fed by the caller) into `"{out}"` with columns
/// `[field,] [COUNT,] geom`: partial unions per tile, then pairwise unions of
/// neighbouring tiles (x, then y, until one cell is left).
pub fn union_tiles(tiles: &Tiles, field: Option<&str>, with_count: bool) -> Vec<String> {
    let f = field.map(ident);
    let lead = f.as_ref().map(|f| format!("{f}, ")).unwrap_or_default();
    let mut s = Vec::new();
    for (n, (x, y)) in tiles.cells.iter().enumerate() {
        let sel = format!(
            "SELECT {x} AS tx, {y} AS ty, {lead}count(*)::BIGINT AS cnt, ST_Union_Agg(geom) AS geom FROM \"{{out}}_t\" WHERE __tx = {x} AND __ty = {y}{}",
            f.as_ref().map(|f| format!(" GROUP BY {f}")).unwrap_or_default()
        );
        s.push(if n == 0 { format!("CREATE OR REPLACE TABLE \"{{out}}_l0\" AS {sel}") } else { format!("INSERT INTO \"{{out}}_l0\" {sel}") });
    }
    let mut level = 0;
    let mut span = 1;
    while span < tiles.grid {
        for (dim, other) in [("tx", "ty"), ("ty", "tx")] {
            let (cur, next) = (format!("\"{{out}}_l{level}\""), format!("\"{{out}}_l{}\"", level + 1));
            let same_group = f.as_ref().map(|f| format!(" AND a.{f} IS NOT DISTINCT FROM b.{f}")).unwrap_or_default();
            let lead_merged = f.as_ref().map(|f| format!(", coalesce(a.{f}, b.{f}) AS {f}")).unwrap_or_default();
            s.push(format!(
                "CREATE OR REPLACE TABLE {next} AS SELECT coalesce(a.{dim}, b.{dim} - 1) // 2 AS {dim}, coalesce(a.{other}, b.{other}) AS {other}{lead_merged}, \
                 coalesce(a.cnt, 0) + coalesce(b.cnt, 0) AS cnt, \
                 CASE WHEN a.geom IS NULL THEN b.geom WHEN b.geom IS NULL THEN a.geom ELSE ST_Union(a.geom, b.geom) END AS geom \
                 FROM (SELECT * FROM {cur} WHERE {dim} % 2 = 0) a FULL JOIN (SELECT * FROM {cur} WHERE {dim} % 2 = 1) b \
                 ON a.{dim} + 1 = b.{dim} AND a.{other} = b.{other}{same_group}"
            ));
            level += 1;
        }
        span *= 2;
    }
    let count = if with_count { "cnt::BIGINT AS COUNT, " } else { "" };
    s.push(format!("CREATE OR REPLACE TABLE \"{{out}}\" AS SELECT {lead}{count}geom FROM \"{{out}}_l{level}\""));
    s.push("DROP TABLE \"{out}_t\"".into());
    for n in 0..=level {
        s.push(format!("DROP TABLE \"{{out}}_l{n}\""));
    }
    s
}
