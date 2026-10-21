use super::*;

pub struct AddToMap;

impl Tool for AddToMap {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "add_to_map",
            name: "Add to Map",
            category: "Output",
            role: Role::Output,
            description: "Add the result to the map as a layer. Re-running updates the same layer.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: None,
            params: vec![Param::new("name", "Layer name", ParamKind::Text).default("Result")],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        Ok(Output::Layer {
            name: p.str("name").unwrap_or_else(|| "Result".into()),
            sql: format!("SELECT * FROM {}", ident(&i.table)),
            crs: i.crs.clone(),
        })
    }
}
