use super::*;

pub struct Near;

impl Tool for Near {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "near",
            name: "Near",
            category: "Proximity",
            role: Role::Tool,
            description: "Geodesic distance from each input feature to the closest near feature (NEAR_DIST), plus that feature's row (NEAR_ROW) (QGIS: Distance to nearest hub).",
            inputs: vec![
                Port { id: "in", label: "Input", kind: PortKind::Vector },
                Port { id: "near", label: "Near features", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("radius", "Search radius", ParamKind::Number).default(10)
                    .help("Features farther than this get no match (keeps it fast)"),
                Param::new("units", "Units", ParamKind::Enum).default("km").options(LINEAR_UNITS),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["in"], &inputs["near"]);
        let crs = need_crs(a)?.to_string();
        let utm = ctx.utm(a)?;
        let units = p.str("units").unwrap_or_else(|| "km".into());
        let r = to_meters(p.f64("radius").unwrap_or(10.0), &units)?;
        let per_unit = to_meters(1.0, &units)?;
        let ag = format!("ST_Transform(a.geom, {}, {}, true)", lit(&crs), lit(&utm));
        let bg = format!("ST_Transform({}, {}, {}, true)", ctx.geom_in(b, "b", &crs)?, lit(&crs), lit(&utm));
        let sql = format!(
            "WITH a AS (SELECT row_number() OVER () AS __rid, *, {ag} AS __m, {aeqd} AS __aeqd FROM {at} a),
             b AS (SELECT row_number() OVER () AS __nid, {bg} AS __m, {bgeom} AS __g FROM {bt} b),
             cand AS (SELECT a.__rid, b.__nid,
                          ST_Distance(ST_Transform(a.geom, {c}, a.__aeqd, true), ST_Transform(b.__g, {c}, a.__aeqd, true)) AS gd
                      FROM a JOIN b ON ST_DWithin(a.__m, b.__m, {r} * 1.05)),
             n AS (SELECT __rid, min(gd) AS d, arg_min(__nid, gd) AS nid FROM cand WHERE gd <= {r} GROUP BY __rid)
             SELECT a.* EXCLUDE (__rid, __m, __aeqd, geom), n.d / {per_unit} AS NEAR_DIST, n.nid AS NEAR_ROW, a.geom
             FROM a LEFT JOIN n ON a.__rid = n.__rid",
            at = ident(&a.table),
            bt = ident(&b.table),
            c = lit(&crs::resolve(&crs)),
            aeqd = aeqd_for("a.geom", &crs),
            bgeom = ctx.geom_in(b, "b", &crs)?
        );
        Ok(Output::Select { sql, crs: Some(crs), view: false })
    }
}
