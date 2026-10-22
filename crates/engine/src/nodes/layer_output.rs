use super::*;

/// The fixed Output node of a layer's node tree. Its input becomes what the
/// layer shows on the map and in the table; the original data is untouched.
pub struct LayerOutput;

impl Tool for LayerOutput {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "layer_output",
            name: "Output",
            category: "Output",
            role: Role::Output,
            description: "The layer shows whatever is connected here. The original data is not changed.",
            inputs: vec![Port { id: "in", label: "Result", kind: PortKind::Any }],
            output: None,
            params: vec![Param::new("layer", "Layer", ParamKind::Layer).hidden()],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        if let Some(path) = &i.raster {
            return Ok(Output::ResultRaster { owner: p.req_str("layer", "Layer")?, path: path.clone() });
        }
        Ok(Output::Result {
            owner: p.req_str("layer", "Layer")?,
            sql: format!("SELECT * FROM {}", ident(&i.table)),
            crs: i.crs.clone(),
        })
    }
}
