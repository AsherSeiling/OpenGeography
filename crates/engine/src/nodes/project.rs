use super::*;
use crate::crs;

pub struct Project;

impl Tool for Project {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "project",
            name: "Project",
            category: "Coordinate Systems",
            role: Role::Tool,
            description: "Reproject features to another coordinate system.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![Param::new("crs", "Output coordinate system", ParamKind::Crs)],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let target = p.req_str("crs", "Output coordinate system")?;
        crs::validate(ctx.conn, &target)?;
        let g = ctx.geom_in(i, "i", &target)?;
        Ok(Output::Select {
            sql: format!("SELECT i.* REPLACE ({g} AS geom) FROM {} i", ident(&i.table)),
            crs: Some(target),
            view: false,
        })
    }
}
