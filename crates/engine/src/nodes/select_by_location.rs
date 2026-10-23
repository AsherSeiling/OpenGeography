use super::*;

pub struct SelectByLocation;

impl Tool for SelectByLocation {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "select_by_location",
            name: "Select by Location",
            category: "Extract",
            role: Role::Tool,
            description: "Keep input features based on where they are relative to another layer (shapes are not cut).",
            inputs: vec![
                Port { id: "in", label: "Input", kind: PortKind::Vector },
                Port { id: "by", label: "Selecting features", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("relation", "Relationship", ParamKind::Enum).default("intersects").options(&[
                    ("intersects", "Intersect"),
                    ("within", "Are within"),
                    ("contains", "Contain"),
                    ("touches", "Touch the boundary of"),
                    ("crosses", "Cross"),
                    ("distance", "Are within a distance of"),
                ]),
                Param::new("distance", "Distance", ParamKind::Number).default(1000),
                Param::new("units", "Units", ParamKind::Enum).default("m").options(LINEAR_UNITS),
                Param::new("invert", "Keep features that do NOT match", ParamKind::Bool).default(false),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["in"], &inputs["by"]);
        let crs = need_crs(a)?.to_string();
        let bg = ctx.geom_in(b, "b", &crs)?;
        let rel = p.str("relation").unwrap_or_else(|| "intersects".into());
        let d = to_meters(p.f64("distance").unwrap_or(0.0), &p.str("units").unwrap_or_else(|| "m".into()))?;
        let not = if p.bool("invert") { "NOT " } else { "" };
        let sql = format!(
            "WITH b AS (SELECT {bg} AS g FROM {bt} b)
             SELECT a.* FROM {at} a WHERE {not}{test}",
            at = ident(&a.table),
            bt = ident(&b.table),
            test = location_test(&rel, &crs, d)
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}

/// The spatial test shared by the Select by Location node and the map's
/// Select By Location: an `EXISTS (…)` over CTE `b` (column `g`, already in
/// `crs`) for the row aliased `a` (column `geom`). Distances are geodesic,
/// measured in an azimuthal equidistant projection centered on each feature.
pub fn location_test(relation: &str, crs: &str, distance_m: f64) -> String {
    let (geom_a, geom_b, pred) = if relation == "distance" {
        let c = lit(&crs::resolve(crs));
        let aeqd = aeqd_for("a.geom", crs);
        (
            format!("ST_Transform(a.geom, {c}, {aeqd}, true)"),
            format!("ST_Transform(b.g, {c}, {aeqd}, true)"),
            format!("ST_DWithin(ga, gb, {distance_m})"),
        )
    } else {
        let f = match relation {
            "within" => "ST_Within",
            "contains" => "ST_Contains",
            "touches" => "ST_Touches",
            "crosses" => "ST_Crosses",
            _ => "ST_Intersects",
        };
        ("a.geom".into(), "b.g".into(), format!("{f}(ga, gb)"))
    };
    format!("EXISTS (SELECT 1 FROM b, LATERAL (SELECT {geom_a} AS ga, {geom_b} AS gb) WHERE {pred})")
}
