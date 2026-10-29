//! Spatial statistics as model nodes (phase H3): Moran's I and Hot Spot Analysis (Getis-Ord Gi*). Formulas in `spatial_stats.rs`;
//! the equivalent ArcGIS tools are Spatial Autocorrelation (Global Moran's I) and Hot Spot Analysis (Getis-Ord Gi*).

use super::*;
use crate::spatial_stats as ss;

/// Rows with a geometry and a number in `field`: (row number, value, x, y) from the feature's centroid. The row number is DuckDB's scan order, which is the table's insertion order (the same in `HotSpots` when it joins the results back).
pub(super) fn read_points(ctx: &Ctx, i: &Rel, field: &str) -> Result<Vec<(i64, f64, f64, f64)>> {
    let crs = need_crs(i)?;
    if crate::crs::is_geographic(crs) {
        bail!("Spatial statistics measure distances between features, so the layer must be in a projected coordinate system (metres or feet), not degrees. Add a Project step first.");
    }
    if !i.has_geom {
        bail!("This step needs a layer with geometry");
    }
    let f = ident(field);
    let mut stmt = ctx.conn.prepare(&format!(
        "SELECT rn, TRY_CAST({f}::VARCHAR AS DOUBLE) AS v, ST_X(ST_Centroid(geom)) AS x, ST_Y(ST_Centroid(geom)) AS y FROM (SELECT row_number() OVER () AS rn, * FROM {}) WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)",
        ident(&i.table)
    ))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<f64>>(1)?, r.get::<_, f64>(2)?, r.get::<_, f64>(3)?)))?;
    let mut out = vec![];
    for r in rows {
        let (id, v, x, y) = r?;
        if let Some(v) = v.filter(|v| v.is_finite()) {
            if x.is_finite() && y.is_finite() {
                out.push((id, v, x, y));
            }
        }
    }
    if out.len() > ss::MAX_FEATURES {
        bail!("That is {} features; the limit is {}. Use a subset or aggregate first.", out.len(), ss::MAX_FEATURES);
    }
    Ok(out)
}

fn neighbours_param(p: &Params) -> Result<usize> {
    let k = p.f64("k").unwrap_or(8.0);
    if !(k.is_finite() && k >= 1.0 && k.fract() == 0.0) {
        bail!("The number of neighbours must be a whole number, 1 or more (8 is usual)");
    }
    Ok(k as usize)
}

pub struct MoransI;
impl Tool for MoransI {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "morans_i", name: "Spatial Autocorrelation (Moran's I)", category: "Analysis", role: Role::Tool,
            description: "Do similar values cluster? Moran's I is near +1 when high values sit near high and low near low, near 0 for a random pattern, near -1 for a checkerboard. Neighbours are each feature's k nearest (by centroid); the layer must be in a projected coordinate system. Outputs a one-row table with I, the expected value, z-score and p-value (randomisation). Features without a number are left out and counted.",
            inputs: vec![Port { id: "in", label: "Features", kind: PortKind::Vector }], output: Some(PortKind::Table),
            params: vec![
                Param::new("field", "Value field", ParamKind::Field).field_of("in"),
                Param::new("k", "Neighbours (k)", ParamKind::Number).default(8),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let field = p.req_str("field", "Value field")?;
        let k = neighbours_param(p)?;
        let pts = read_points(ctx, i, &field)?;
        let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&i.table)), [], |r| r.get(0))?;
        let xy: Vec<(f64, f64)> = pts.iter().map(|p| (p.2, p.3)).collect();
        let vals: Vec<f64> = pts.iter().map(|p| p.1).collect();
        let nb = ss::knn(&xy, k.min(pts.len().saturating_sub(1)).max(1), &|| crate::progress::run_cancelled())?;
        let m = ss::morans_i(&vals, &nb)?;
        let verdict = if m.p < 0.05 { if m.i > m.expected { "clustered" } else { "dispersed" } } else { "random" };
        Ok(Output::Select {
            sql: format!(
                "SELECT {n} AS features_used, {left} AS features_left_out, {k} AS neighbours, {i} AS morans_i, {e} AS expected_i, {v} AS variance, {z} AS z_score, {p} AS p_value, '{verdict}' AS pattern",
                n = m.n, left = total - m.n as i64, k = k, i = m.i, e = m.expected, v = m.variance, z = m.z, p = m.p
            ),
            crs: None,
            view: false,
        })
    }
}

pub struct HotSpots;
impl Tool for HotSpots {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "hot_spots", name: "Hot Spot Analysis (Getis-Ord Gi*)", category: "Analysis", role: Role::Tool,
            description: "Find statistically significant clusters of high values (hot spots) and low values (cold spots). Each feature is compared with itself and its k nearest neighbours. Adds gi_z (z-score), gi_p (p-value) and hotspot (3, 2, 1 = hot at 99 %, 95 %, 90 %; -1, -2, -3 = cold; 0 = not significant). The layer must be in a projected coordinate system. Features without a number get empty results.",
            inputs: vec![Port { id: "in", label: "Features", kind: PortKind::Vector }], output: Some(PortKind::Vector),
            params: vec![
                Param::new("field", "Value field", ParamKind::Field).field_of("in"),
                Param::new("k", "Neighbours (k)", ParamKind::Number).default(8),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let field = p.req_str("field", "Value field")?;
        let k = neighbours_param(p)?;
        let pts = read_points(ctx, i, &field)?;
        let xy: Vec<(f64, f64)> = pts.iter().map(|p| (p.2, p.3)).collect();
        let vals: Vec<f64> = pts.iter().map(|p| p.1).collect();
        let nb = ss::knn(&xy, k.min(pts.len().saturating_sub(1)).max(1), &|| crate::progress::run_cancelled())?;
        let gi = ss::getis_ord_gi_star(&vals, &nb)?;
        let rows: Vec<String> = pts.iter().zip(&gi).map(|(p, g)| format!("({}, {}, {}, {})", p.0, g.z, g.p, g.class)).collect();
        Ok(Output::Select {
            sql: format!(
                "SELECT t.* EXCLUDE (rn), g.gi_z, g.gi_p, g.hotspot FROM (SELECT row_number() OVER () AS rn, * FROM {}) t LEFT JOIN (SELECT * FROM (VALUES {}) v(rid, gi_z, gi_p, hotspot)) g ON t.rn = g.rid",
                ident(&i.table), rows.join(", ")
            ),
            crs: i.crs.clone(),
            view: false,
        })
    }
}

pub struct Kriging;
impl Tool for Kriging {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "kriging", name: "Kriging (Ordinary)", category: "Raster", role: Role::Tool,
            description: "Make a surface from points with a number field, plus how uncertain it is. The spatial pattern of the values is measured (a variogram) and fitted with a spherical, exponential or Gaussian model, or all three with the best kept (Auto); leave nugget, sill and range empty to fit them, or fill all three to use your own. Band 1 is the prediction, band 2 the kriging variance (higher = less certain). The layer must be in a projected coordinate system; cell size and range are in its units. The fitted model is stored in the raster's VARIOGRAM metadata. Points at the same place are averaged.",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("field", "Value field", ParamKind::Field).field_of("in"),
                Param::new("model", "Variogram model", ParamKind::Enum).default("auto").options(&[("auto", "Auto (best fit)"), ("spherical", "Spherical"), ("exponential", "Exponential"), ("gaussian", "Gaussian")]),
                Param::new("cell", "Cell size", ParamKind::Number).default(100),
                Param::new("neighbours", "Neighbours used per cell", ParamKind::Number).default(12),
                Param::new("nugget", "Nugget (empty: fit)", ParamKind::Number).optional(),
                Param::new("sill", "Partial sill (empty: fit)", ParamKind::Number).optional(),
                Param::new("range", "Range (empty: fit)", ParamKind::Number).optional(),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let field = p.req_str("field", "Value field")?;
        let pts: Vec<(f64, f64, f64)> = read_points(ctx, i, &field)?.into_iter().map(|r| (r.2, r.3, r.1)).collect();
        let model = crate::kriging::Model::parse(p.str("model").as_deref().unwrap_or("auto"))?;
        let fixed = match (p.f64("nugget"), p.f64("sill"), p.f64("range")) {
            (None, None, None) => None,
            (Some(n), Some(s), Some(r)) => Some((n, s, r)),
            _ => bail!("Give all of nugget, sill and range to use your own variogram, or leave all three empty to fit one"),
        };
        let (path, _v) = crate::kriging::run(&pts, model, fixed, p.f64("cell").unwrap_or(0.0), p.f64("neighbours").unwrap_or(12.0) as usize, need_crs(i)?)?;
        Ok(Output::Raster { path: path.display().to_string(), crs: i.crs.clone() })
    }
}
