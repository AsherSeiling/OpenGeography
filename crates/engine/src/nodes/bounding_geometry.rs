use super::*;

pub struct BoundingGeometry;

impl Tool for BoundingGeometry {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "bounding_geometry",
            name: "Minimum Bounding Geometry",
            category: "Generalize",
            role: Role::Tool,
            description: "Convex hull or bounding rectangle around each feature, each group, or everything.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("shape", "Shape", ParamKind::Enum).default("hull").options(&[
                    ("hull", "Convex hull"),
                    ("envelope", "Rectangle (envelope)"),
                ]),
                Param::new("group", "Group", ParamKind::Enum).default("each").options(&[
                    ("each", "Each feature"),
                    ("all", "All features"),
                    ("field", "By field"),
                ]),
                Param::new("field", "Group field", ParamKind::Field).field_of("in").optional(),
            ],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let t = ident(&i.table);
        let shape = |g: &str| if p.str("shape").as_deref() == Some("envelope") { format!("ST_Envelope({g})") } else { format!("ST_ConvexHull({g})") };
        let sql = match p.str("group").as_deref().unwrap_or("each") {
            "all" => format!("SELECT count(*) AS COUNT, {} AS geom FROM {t}", shape("ST_Collect(list(geom))")),
            "field" => {
                let f = ident(&p.req_str("field", "Group field")?);
                format!("SELECT {f}, count(*) AS COUNT, {} AS geom FROM {t} GROUP BY {f}", shape("ST_Collect(list(geom))"))
            }
            _ => format!("SELECT * REPLACE ({} AS geom) FROM {t}", shape("geom")),
        };
        Ok(Output::Select { sql, crs: i.crs.clone(), view: false })
    }
}
