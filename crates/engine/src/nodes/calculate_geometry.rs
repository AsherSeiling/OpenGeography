use super::{NewField, OutFields};
use super::*;

pub struct CalculateGeometry;


impl Tool for CalculateGeometry {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "calculate_geometry",
            name: "Calculate Geometry",
            category: "Tables",
            role: Role::Tool,
            description: "Add geodesic area, length or perimeter, or centroid coordinates, as a field.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("field", "Field name", ParamKind::Text).default("AREA"),
                Param::new("property", "Property", ParamKind::Enum).default("area").options(crate::measure::PROPERTIES),
                Param::new("area_units", "Area units", ParamKind::Enum).default("sqkm").options(crate::measure::AREA_UNITS),
                Param::new("length_units", "Length units", ParamKind::Enum).default("km").options(LINEAR_UNITS),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&["in"]).add(NewField::new("", "DOUBLE").named_by("field"))
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?;
        let f = p.req_str("field", "Field name")?;
        let expr = crate::measure::geometry_expr(
            crs,
            p.str("property").as_deref().unwrap_or("area"),
            p.str("area_units").as_deref().unwrap_or("sqkm"),
            p.str("length_units").as_deref().unwrap_or("km"),
        )?;
        Ok(Output::Select {
            sql: format!("SELECT * EXCLUDE (geom), {expr} AS {}, geom FROM {}", ident(&f), ident(&i.table)),
            crs: i.crs.clone(),
            view: false,
        })
    }
}
