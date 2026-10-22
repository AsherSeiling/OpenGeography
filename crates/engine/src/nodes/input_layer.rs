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
            params: vec![
                Param::new("layer", "Layer", ParamKind::Layer),
                Param::new("fids", "Only these features", ParamKind::Text).hidden(),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, _inputs: &Inputs, p: &Params) -> Result<Output> {
        let id = p.req_str("layer", "Layer")?;
        let info = ctx.layers.get(&id).ok_or_else(|| anyhow!("Layer no longer exists"))?;
        let hidden = if info.has_geometry() { "__fid, __g, __x0, __y0, __sz" } else { "__fid" };
        // Respects the layer's definition query; `fids` limits to a selection.
        let only = match p.0.get("fids").and_then(|v| v.as_array()) {
            Some(ids) => {
                let ids: Vec<String> = ids.iter().filter_map(|v| v.as_i64()).map(|i| i.to_string()).collect();
                if ids.is_empty() { " AND FALSE".to_string() } else { format!(" AND __fid IN ({})", ids.join(",")) }
            }
            None => String::new(),
        };
        Ok(Output::Select {
            sql: format!("SELECT * EXCLUDE ({hidden}) FROM {} WHERE TRUE{}{only}", ident(&info.table), info.def_and()),
            crs: info.crs.clone(),
            view: true,
        })
    }
}
