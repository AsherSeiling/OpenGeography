use super::OutFields;
use super::*;

pub struct SpatialJoin;

impl Tool for SpatialJoin {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "spatial_join",
            name: "Spatial Join",
            category: "Overlay",
            role: Role::Tool,
            description: "Attach attributes of join features to target features based on location.",
            inputs: vec![
                Port { id: "target", label: "Target features", kind: PortKind::Vector },
                Port { id: "join", label: "Join features", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("predicate", "Match option", ParamKind::Enum).default("intersects").options(&[
                    ("intersects", "Intersects"),
                    ("within", "Target within join feature"),
                    ("contains", "Target contains join feature"),
                ]),
                Param::new("keep_all", "Keep all target features", ParamKind::Bool).default(true),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&["target", "join"])
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["target"], &inputs["join"]);
        let crs = need_crs(a)?.to_string();
        let bg = ctx.geom_in(b, "b", &crs)?;
        let pred = match p.str("predicate").as_deref().unwrap_or("intersects") {
            "within" => "ST_Within(a.geom, b2.geom)",
            "contains" => "ST_Contains(a.geom, b2.geom)",
            _ => "ST_Intersects(a.geom, b2.geom)",
        };
        let a_cols = ctx.columns(a)?;
        let b_cols = join_columns(&a_cols, &ctx.columns(b)?, "b2");
        let b_sel = if b_cols.is_empty() { String::new() } else { format!(", {b_cols}") };
        let join = if p.bool("keep_all") { "LEFT JOIN" } else { "JOIN" };
        let sql = format!(
            "WITH b2 AS (SELECT * REPLACE ({bg} AS geom) FROM {bt} b)
             SELECT a.*{b_sel} FROM {at} a {join} b2 ON {pred}",
            at = ident(&a.table),
            bt = ident(&b.table)
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
