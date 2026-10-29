use super::*;

pub struct Simplify;

impl Tool for Simplify {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "simplify",
            name: "Simplify",
            category: "Vector › Generalize",
            role: Role::Tool,
            description: "Remove small bends from lines and polygon outlines. Tolerance is a real ground distance.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("tolerance", "Tolerance", ParamKind::Number).default(50),
                Param::new("units", "Units", ParamKind::Enum).default("m").options(LINEAR_UNITS),
                Param::new("topology", "Keep shapes valid", ParamKind::Bool).default(true)
                    .help("Slower, but never creates self-crossing polygons"),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?.to_string();
        let utm = ctx.utm(i)?;
        let tol = to_meters(p.f64("tolerance").unwrap_or(0.0), &p.str("units").unwrap_or_else(|| "m".into()))?;
        let f = if p.bool("topology") { "ST_SimplifyPreserveTopology" } else { "ST_Simplify" };
        let g = format!("ST_Transform({f}(ST_Transform(geom, {c}, {u}, true), {tol}), {u}, {c}, true)", c = lit(&crs), u = lit(&utm));
        Ok(Output::Select { sql: format!("SELECT * REPLACE ({g} AS geom) FROM {}", ident(&i.table)), crs: Some(crs), view: false })
    }
}
