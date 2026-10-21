use super::*;

pub struct InputLayer;

impl Tool for InputLayer {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "input_layer",
            name: "Layer",
            category: "Data",
            role: Role::Data,
            description: "Use a layer from the Contents pane as input.",
            inputs: vec![],
            output: Some(PortKind::Table),
            params: vec![Param::new("layer", "Layer", ParamKind::Layer)],
        }
    }

    fn build(&self, ctx: &Ctx, _inputs: &Inputs, p: &Params) -> Result<Output> {
        let id = p.req_str("layer", "Layer")?;
        let info = ctx.layers.get(&id).ok_or_else(|| anyhow!("Layer no longer exists"))?;
        let hidden = if info.has_geometry() { "__fid, __g, __x0, __y0, __sz" } else { "__fid" };
        Ok(Output::Select {
            sql: format!("SELECT * EXCLUDE ({hidden}) FROM {}", ident(&info.table)),
            crs: info.crs.clone(),
            view: true,
        })
    }
}
