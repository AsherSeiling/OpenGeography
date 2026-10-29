use super::*;

pub struct Erase;

impl Tool for Erase {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "erase",
            name: "Erase",
            category: "Vector › Overlay",
            role: Role::Tool,
            description: "Remove the parts of input features that fall inside the erase polygons.",
            inputs: vec![
                Port { id: "in", label: "Input", kind: PortKind::Vector },
                Port { id: "erase", label: "Erase features", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (a, e) = (&inputs["in"], &inputs["erase"]);
        let crs = need_crs(a)?.to_string();
        let eg = ctx.geom_in(e, "e", &crs)?;
        let dim = ctx.dimension(a)?;
        // Only features that touch an erase polygon are cut; the rest pass through.
        let sql = format!(
            "WITH a AS (SELECT row_number() OVER () AS __rid, * FROM {at}),
             e AS (SELECT {eg} AS g FROM {et} e),
             cut AS (SELECT a.__rid, ST_Union_Agg(e.g) AS g FROM a JOIN e ON ST_Intersects(a.geom, e.g) GROUP BY a.__rid)
             SELECT a.* EXCLUDE (__rid) REPLACE (
                 CASE WHEN cut.g IS NULL THEN a.geom ELSE ST_CollectionExtract(ST_Difference(a.geom, cut.g), {t}) END AS geom)
             FROM a LEFT JOIN cut ON a.__rid = cut.__rid
             WHERE cut.g IS NULL OR NOT ST_IsEmpty(ST_CollectionExtract(ST_Difference(a.geom, cut.g), {t}))",
            at = ident(&a.table),
            et = ident(&e.table),
            t = dim + 1
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
