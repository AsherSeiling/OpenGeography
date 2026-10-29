use super::*;

pub struct Filter;

impl Tool for Filter {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "filter",
            name: "Definition Query",
            category: "Vector › Select and Clean",
            role: Role::Tool,
            description: "Keep only features matching a query (build it point-and-click, in Python, or SQL).",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: vec![Param::new("where", "Query", ParamKind::Query)
                .field_of("in")
                .help("e.g. POP100 > 1000 AND COUNTY = '109'")],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let w = p.req_str("where", "Query")?;
        Ok(Output::Select { sql: format!("SELECT * FROM {} WHERE ({w})", ident(&i.table)), crs: i.crs.clone(), view: false })
    }
}
