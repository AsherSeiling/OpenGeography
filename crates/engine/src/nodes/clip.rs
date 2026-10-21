use super::*;

pub struct Clip;

impl Tool for Clip {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "clip",
            name: "Clip",
            category: "Overlay",
            role: Role::Tool,
            description: "Cut input features to the area of the clip polygons. Keeps input attributes only.",
            inputs: vec![
                Port { id: "in", label: "Input", kind: PortKind::Vector },
                Port { id: "clip", label: "Clip features", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (a, c) = (&inputs["in"], &inputs["clip"]);
        let crs = need_crs(a)?.to_string();
        let cg = ctx.geom_in(c, "c", &crs)?;
        let dim = ctx.dimension(a)?;
        let sql = format!(
            "WITH a AS (SELECT row_number() OVER () AS __rid, * FROM {at}),
             c AS (SELECT {cg} AS g FROM {ct} c),
             j AS (SELECT a.__rid, ST_Intersection(a.geom, c.g) AS g FROM a JOIN c ON ST_Intersects(a.geom, c.g)),
             u AS (SELECT __rid, CASE WHEN count(*) = 1 THEN any_value(g) ELSE ST_Union_Agg(g) END AS g FROM j GROUP BY __rid)
             SELECT a.* EXCLUDE (__rid) REPLACE (ST_CollectionExtract(u.g, {t}) AS geom)
             FROM a JOIN u ON a.__rid = u.__rid
             WHERE NOT ST_IsEmpty(ST_CollectionExtract(u.g, {t}))",
            at = ident(&a.table),
            ct = ident(&c.table),
            t = dim + 1
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
