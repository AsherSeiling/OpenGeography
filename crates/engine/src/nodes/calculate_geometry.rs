use super::*;

pub struct CalculateGeometry;

const AREA_UNITS: &[(&str, &str)] = &[
    ("sqm", "Square meters"),
    ("sqkm", "Square kilometers"),
    ("ha", "Hectares"),
    ("acres", "Acres"),
    ("sqmi", "Square miles"),
    ("sqft", "Square feet"),
];

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
                Param::new("property", "Property", ParamKind::Enum).default("area").options(&[
                    ("area", "Area (geodesic)"),
                    ("length", "Length (geodesic)"),
                    ("perimeter", "Perimeter (geodesic)"),
                    ("x", "Centroid longitude"),
                    ("y", "Centroid latitude"),
                ]),
                Param::new("area_units", "Area units", ParamKind::Enum).default("sqkm").options(AREA_UNITS),
                Param::new("length_units", "Length units", ParamKind::Enum).default("km").options(LINEAR_UNITS),
            ],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?;
        let f = p.req_str("field", "Field name")?;
        // Geodesic measures on the WGS 84 ellipsoid. With geometry_always_xy
        // set (see db.rs), the spheroid functions take lon/lat order.
        let ll = format!("ST_Transform(geom, {}, 'EPSG:4326', true)", lit(crs));
        let geo = ll.clone();
        let expr = match p.str("property").as_deref().unwrap_or("area") {
            "area" => {
                let div = match p.str("area_units").as_deref().unwrap_or("sqkm") {
                    "sqm" => 1.0,
                    "sqkm" => 1e6,
                    "ha" => 1e4,
                    "acres" => 4046.8564224,
                    "sqmi" => 2_589_988.110336,
                    "sqft" => 0.09290304,
                    u => bail!("Unknown area unit '{u}'"),
                };
                format!("ST_Area_Spheroid({geo}) / {div}")
            }
            prop @ ("length" | "perimeter") => {
                let div = to_meters(1.0, &p.str("length_units").unwrap_or_else(|| "km".into()))?;
                let func = if prop == "length" { "ST_Length_Spheroid" } else { "ST_Perimeter_Spheroid" };
                format!("{func}({geo}) / {div}")
            }
            "x" => format!("ST_X(ST_Centroid({ll}))"),
            "y" => format!("ST_Y(ST_Centroid({ll}))"),
            other => bail!("Unknown property '{other}'"),
        };
        Ok(Output::Select {
            sql: format!("SELECT * EXCLUDE (geom), {expr} AS {}, geom FROM {}", ident(&f), ident(&i.table)),
            crs: i.crs.clone(),
            view: false,
        })
    }
}
