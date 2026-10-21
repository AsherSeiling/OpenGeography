use super::*;

pub struct Filter;

impl Tool for Filter {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "filter",
            name: "Select (Filter)",
            category: "Extract",
            role: Role::Tool,
            description: "Keep only features matching an attribute expression.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: vec![Param::new("where", "Expression", ParamKind::Expression)
                .field_of("in")
                .help("SQL expression, e.g. POP2020 > 1000 AND STATE = 'TX'")],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let w = p.req_str("where", "Expression")?;
        Ok(Output::Select { sql: format!("SELECT * FROM {} WHERE ({w})", ident(&i.table)), crs: i.crs.clone(), view: false })
    }
}
