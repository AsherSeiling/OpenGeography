use super::*;

pub struct Explode;

impl Tool for Explode {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "explode",
            name: "Multipart To Singlepart",
            category: "Vector › Geometry",
            role: Role::Tool,
            description: "Split multipart features (e.g. a county with islands) into one feature per part.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        Ok(Output::Select {
            sql: format!(
                "SELECT * EXCLUDE (geom, __part), __part.geom AS geom FROM (SELECT *, UNNEST(ST_Dump(geom)) AS __part FROM {})",
                ident(&i.table)
            ),
            crs: i.crs.clone(),
            view: false,
        })
    }
}
