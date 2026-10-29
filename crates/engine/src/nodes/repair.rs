use super::*;

pub struct Repair;

impl Tool for Repair {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "repair",
            name: "Repair Geometry",
            category: "Vector › Select and Clean",
            role: Role::Tool,
            description: "Fix invalid shapes (self-crossing polygons, etc.) and drop empty ones.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let t = ctx.dimension(i)? + 1;
        Ok(Output::Select {
            sql: format!(
                "SELECT * REPLACE (CASE WHEN ST_IsValid(geom) THEN geom ELSE ST_CollectionExtract(ST_MakeValid(geom), {t}) END AS geom)
                 FROM {} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)",
                ident(&i.table)
            ),
            crs: i.crs.clone(),
            view: false,
        })
    }
}
