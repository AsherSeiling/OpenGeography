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
        let target = crs::resolve(&p.req_str("crs", "Output coordinate system")?);
        crs::validate(ctx.conn, &target)?;
        let g = ctx.geom_in(i, "i", &target)?;
        // Features PROJ cannot show in the target CRS (a pole in a conic, a point beyond a projection's domain) are not kept as
        // broken shapes: they are left out and counted, so the result says how many went (RD-6, bug C-2).
        let (total, lost): (i64, i64) = ctx.conn.query_row(
            &format!("SELECT count(*), count(*) FILTER (WHERE geom IS NOT NULL AND ({g}) IS NULL) FROM (SELECT geom FROM {}) i", ident(&i.table)),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap_or((0, 0));
        let _ = total;
        if lost > 0 {
            ctx.add_note(format!("{lost} feature{} could not be projected into this coordinate system (outside what it can show) and {} left out.", if lost == 1 { "" } else { "s" }, if lost == 1 { "was" } else { "were" }));
        }
        Ok(Output::Select {
            sql: format!("SELECT i.* REPLACE ({g} AS geom) FROM {} i WHERE i.geom IS NULL OR ({g}) IS NOT NULL", ident(&i.table)),
            crs: Some(target),
            view: false,
        })
    }
}
