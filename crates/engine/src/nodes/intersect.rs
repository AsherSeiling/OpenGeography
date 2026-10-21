use super::*;

pub struct Intersect;

impl Tool for Intersect {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "intersect",
            name: "Intersect",
            category: "Overlay",
            role: Role::Tool,
            description: "Geometric intersection of two layers. Output keeps attributes from both.",
            inputs: vec![
                Port { id: "a", label: "Input", kind: PortKind::Vector },
                Port { id: "b", label: "Intersect features", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["a"], &inputs["b"]);
        let crs = need_crs(a)?.to_string();
        let bg = ctx.geom_in(b, "b", &crs)?;
        let a_cols = ctx.columns(a)?;
        let b_cols = join_columns(&a_cols, &ctx.columns(b)?, "b2");
        let dim = ctx.dimension(a)?.min(ctx.dimension(b)?);
        let b_sel = if b_cols.is_empty() { String::new() } else { format!(", {b_cols}") };
        let sql = format!(
            "WITH b2 AS (SELECT * REPLACE ({bg} AS geom) FROM {bt} b),
             j AS (SELECT a.* EXCLUDE (geom){b_sel}, ST_CollectionExtract(ST_Intersection(a.geom, b2.geom), {t}) AS geom
                   FROM {at} a JOIN b2 ON ST_Intersects(a.geom, b2.geom))
             SELECT * FROM j WHERE NOT ST_IsEmpty(geom)",
            at = ident(&a.table),
            bt = ident(&b.table),
            t = dim + 1
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
