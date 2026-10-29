use super::*;

pub struct FeatureToPoint;

impl Tool for FeatureToPoint {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "feature_to_point",
            name: "Feature To Point",
            category: "Vector › Geometry",
            role: Role::Tool,
            description: "One point per feature: its centroid, or a point guaranteed to be inside the shape.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![Param::new("method", "Point", ParamKind::Enum).default("centroid").options(&[
                ("centroid", "Centroid"),
                ("inside", "Inside the feature"),
            ])],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let f = if p.str("method").as_deref() == Some("inside") { "ST_PointOnSurface" } else { "ST_Centroid" };
        Ok(Output::Select { sql: format!("SELECT * REPLACE ({f}(geom) AS geom) FROM {}", ident(&i.table)), crs: i.crs.clone(), view: false })
    }
}
