use super::OutFields;
use super::*;

/// SQL for the parts of `left` not covered by any `right` geometry. Both are
/// relations with `__rid` and `geom`; `t` is the geometry type to keep.
pub(super) fn minus_sql(left: &str, right: &str, t: i32) -> String {
    format!(
        "SELECT l.* EXCLUDE (__rid) REPLACE (
             CASE WHEN cut.g IS NULL THEN l.geom ELSE ST_CollectionExtract(ST_Difference(l.geom, cut.g), {t}) END AS geom)
         FROM {left} l LEFT JOIN (
             SELECT l.__rid, ST_Union_Agg(r.geom) AS g FROM {left} l JOIN {right} r ON ST_Intersects(l.geom, r.geom) GROUP BY l.__rid
         ) cut ON l.__rid = cut.__rid
         WHERE cut.g IS NULL OR NOT ST_IsEmpty(ST_CollectionExtract(ST_Difference(l.geom, cut.g), {t}))"
    )
}

/// The two inputs as `a` / `b2` CTEs (b reprojected to a's CRS, colliding
/// field names renamed to `name_1`), plus a's CRS and both geometry types.
pub(super) fn overlay_ctes(ctx: &Ctx, inputs: &Inputs) -> Result<(String, String, i32, i32)> {
    let (a, b) = (&inputs["a"], &inputs["b"]);
    let crs = need_crs(a)?.to_string();
    let bg = ctx.geom_in(b, "b", &crs)?;
    let b_cols = join_columns(&ctx.columns(a)?, &ctx.columns(b)?, "b");
    let b_sel = if b_cols.is_empty() { String::new() } else { format!("{b_cols}, ") };
    let ctes = format!(
        "a AS (SELECT row_number() OVER () AS __rid, * FROM {at}),
         b2 AS (SELECT row_number() OVER () AS __rid, {b_sel}{bg} AS geom FROM {bt} b)",
        at = ident(&a.table),
        bt = ident(&b.table)
    );
    Ok((ctes, crs, ctx.dimension(a)? + 1, ctx.dimension(b)? + 1))
}

pub struct Union;

impl Tool for Union {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "union",
            name: "Union",
            category: "Overlay",
            role: Role::Tool,
            description: "Overlay two layers, keeping every part of both: where they overlap (attributes from both) and where only one covers (the other's fields are empty). QGIS: Union, native:union.",
            inputs: vec![
                Port { id: "a", label: "Input", kind: PortKind::Vector },
                Port { id: "b", label: "Overlay features", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&["a", "b"])
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (ctes, crs, ta, tb) = overlay_ctes(ctx, inputs)?;
        let both = ta.min(tb);
        let sql = format!(
            "WITH {ctes},
             overlap AS (SELECT a.* EXCLUDE (geom, __rid), b2.* EXCLUDE (geom, __rid),
                             ST_CollectionExtract(ST_Intersection(a.geom, b2.geom), {both}) AS geom
                      FROM a JOIN b2 ON ST_Intersects(a.geom, b2.geom)),
             a_only AS ({a_only}),
             b_only AS ({b_only})
             SELECT * FROM overlap WHERE NOT ST_IsEmpty(geom)
             UNION ALL BY NAME SELECT * FROM a_only
             UNION ALL BY NAME SELECT * FROM b_only",
            a_only = minus_sql("a", "b2", ta),
            b_only = minus_sql("b2", "a", tb),
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
