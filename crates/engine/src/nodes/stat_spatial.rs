//! Statistics › Spatial (package ST6): the spatial weights manager, global Moran's I with a Moran scatter plot, and local Moran (LISA) with a
//! permutation test, a visible seed and optional Holm / Benjamini-Hochberg adjustment. The maths is in `stats/weights.rs`.
//! Checked against hand-worked examples and independent formulas (`tests/stat_spatial.rs`); see the method notes in each result.

use super::stat_geo::*;
use super::stat_support::*;
use super::*;
use crate::stats::multiple::{adjust, adjust_label};
use crate::stats::weights::{self, Rng};
use crate::stats::{charts, ColumnKind as K, Dropped};
use serde_json::{json, Value};

const C_WEIGHTS: &str = "Anselin, L. (1988). Spatial Econometrics: Methods and Models. Kluwer (spatial weights); Karney, C. F. F. (2013). Algorithms for geodesics. Journal of Geodesy 87, 43-55 (geodesic distances, geographiclib-rs, MIT).";
const C_MORAN: &str = "Moran, P. A. P. (1950). Notes on continuous stochastic phenomena. Biometrika 37, 17-23; Cliff, A. D. and Ord, J. K. (1981). Spatial Processes: Models and Applications. Pion (variance under randomisation and normality).";
const C_LISA: &str = "Anselin, L. (1995). Local indicators of spatial association - LISA. Geographical Analysis 27(2), 93-115 (local Moran, conditional permutation).";
const C_HOLM: &str = "Holm, S. (1979). Scandinavian Journal of Statistics 6, 65-70.";
const C_BH: &str = "Benjamini, Y. and Hochberg, Y. (1995). Journal of the Royal Statistical Society B 57, 289-300.";

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(WeightsTool), Box::new(MoranTool), Box::new(LisaTool)]
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "stat_weights" => &["spatial weights", "neighbours", "contiguity", "queen", "rook", "k nearest", "distance band", "islands", "weights matrix"],
        "stat_moran" => &["moran", "spatial autocorrelation", "clustering of values", "moran scatter plot", "global moran"],
        "stat_lisa" => &["lisa", "local moran", "hot spots", "cluster and outlier", "anselin", "high-high", "low-low", "spatial outliers"],
        _ => &[],
    }
}

struct Prepared {
    co: Coords,
    /// Table rows used (a value and a geometry).
    used: Vec<usize>,
    vals: Vec<f64>,
    total: usize,
    dropped: Vec<Dropped>,
}

/// Load one numeric field and the centroids; rows need both.
fn prepare(ctx: &Ctx, rel: &Rel, field: &str) -> Result<Prepared> {
    let co = coords(ctx, rel)?;
    let d = load(ctx, rel, &[(field, true)])?;
    let mut used = vec![];
    let mut vals = vec![];
    let mut no_geom = 0u64;
    for r in 0..d.rows {
        match (d.cols[0].num[r], co.xy.get(r).copied().flatten()) {
            (Some(v), Some(_)) => {
                used.push(r);
                vals.push(v);
            }
            (Some(_), None) => no_geom += 1,
            _ => {}
        }
    }
    let mut dropped = drop_reasons(&d.cols[0], field);
    if no_geom > 0 {
        dropped.push(Dropped { reason: "no geometry (rows with a value)".into(), count: no_geom });
    }
    if used.len() > weights::MAX_FEATURES * 10 {
        bail!("{} features is more than the spatial statistics hold in memory; use a subset.", used.len());
    }
    Ok(Prepared { co, used, vals, total: d.rows, dropped })
}

fn seed_of(p: &Params) -> Result<u64> {
    let s = p.f64("seed").unwrap_or(12345.0);
    if !(s.is_finite() && s >= 0.0 && s.fract() == 0.0) {
        bail!("'Random seed' must be a whole number, 0 or more");
    }
    Ok(s as u64)
}

fn perms_of(p: &Params) -> Result<usize> {
    let n = p.f64("permutations").unwrap_or(999.0);
    if !(n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= 99_999.0) {
        bail!("'Permutations' must be a whole number from 0 to 99999");
    }
    Ok(n as usize)
}

fn unprojected_note(co: &Coords, r: &mut crate::stats::StatResult) {
    if co.geographic {
        r.notes.push("The layer is in degrees: neighbour distances are geodesic (WGS 84, Karney 2013) between feature centroids; contiguity is topological.".into());
    }
}

// ------------------------------------------------------------------ weights

struct WeightsTool;

impl Tool for WeightsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_weights",
            name: "Spatial Weights",
            category: "Statistics › Spatial",
            role: Role::Tool,
            description: "Builds and checks the neighbour structure the spatial statistics use: queen or rook contiguity (polygons), k nearest neighbours, or a distance band, with geodesic distances for layers in degrees. Reports how many neighbours each feature has, lists islands (features with none), and adds the neighbour count to a copy of the layer so islands can be mapped.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: weights_params(),
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = inputs.get("in").ok_or_else(|| anyhow!("Spatial Weights needs an input layer"))?;
        let co = coords(ctx, rel)?;
        let used: Vec<usize> = (0..co.xy.len()).filter(|i| co.xy[*i].is_some()).collect();
        if used.len() < 3 {
            bail!("At least 3 features with a geometry are needed.");
        }
        let w = make_weights(ctx, rel, p, &used, &co)?;
        let counts: Vec<usize> = w.nbrs.iter().map(|v| v.len()).collect();
        let n = used.len() as f64;
        let mean = counts.iter().sum::<usize>() as f64 / n;
        let isl = w.islands();
        let mut r = result(format!("Spatial weights: {} for {} features", w.kind, used.len()), &[C_WEIGHTS]);
        r.tables.push(table(
            "Weights summary",
            vec![col("weights", K::Text), col("features", K::Integer), col("links", K::Integer), col("mean_neighbours", K::Number), col("min_neighbours", K::Integer), col("max_neighbours", K::Integer), col("islands", K::Integer), col("symmetric", K::Bool)],
            vec![vec![json!(w.kind), json!(used.len()), json!(w.links()), num(mean), json!(counts.iter().min().unwrap()), json!(counts.iter().max().unwrap()), json!(isl.len()), json!(w.is_symmetric())]],
        ));
        let mut hist: std::collections::BTreeMap<usize, usize> = Default::default();
        for c in &counts {
            *hist.entry(*c).or_default() += 1;
        }
        r.tables.push(table("Neighbour counts", vec![col("neighbours", K::Integer), col("features", K::Integer)], hist.iter().map(|(k, v)| vec![json!(k), json!(v)]).collect()));
        let fl: Vec<f64> = counts.iter().map(|c| *c as f64).collect();
        if let Ok(b) = charts::histogram(&vec![-1; fl.len()], &fl, None, "Number of neighbours", 20_000) {
            r.charts.push(b.chart);
        }
        r.n_used = Some(used.len() as u64);
        if used.len() < co.xy.len() {
            r.dropped.push(Dropped { reason: "no geometry".into(), count: (co.xy.len() - used.len()) as u64 });
        }
        if let Some(n) = island_note(&w) {
            r.notes.push(n);
        }
        if !w.is_symmetric() {
            r.notes.push("The links are not symmetric (A can list B as a neighbour without B listing A), as is usual for k nearest neighbours. Statistics that need symmetric weights say so.".into());
        }
        r.notes.push("Weights are row-standardised by the statistics that use them (each feature's neighbours share a total weight of 1).".into());
        unprojected_note(&co, &mut r);
        r.notes.push(SPATIAL_NOTE.to_string());
        let mut vals: Vec<Vec<Option<f64>>> = vec![vec![None; co.xy.len()]];
        for (i, row) in used.iter().enumerate() {
            vals[0][*row] = Some(counts[i] as f64);
        }
        let sql = join_columns_sql(&rel.table, &["n_neighbours"], &vals);
        ctx.set_stat(r);
        Ok(Output::Select { sql, crs: rel.crs.clone(), view: false })
    }
}

// -------------------------------------------------------------------- Moran

struct MoranTool;

impl Tool for MoranTool {
    fn spec(&self) -> ToolSpec {
        let mut params = vec![
            Param::new("field", "Value field", ParamKind::Field).field_of("in"),
            Param::new("permutations", "Permutations (0 = none)", ParamKind::Number).default(999).help("Random reshufflings of the values for a pseudo p-value."),
            Param::new("seed", "Random seed", ParamKind::Number).default(12345).help("Shown in the result; the same seed gives the same pseudo p-value."),
        ];
        params.extend(weights_params());
        ToolSpec {
            id: "stat_moran",
            name: "Global Moran's I",
            category: "Statistics › Spatial",
            role: Role::Tool,
            description: "Do similar values sit near each other? Moran's I with row-standardised weights of your choice: the statistic, its expectation, the z-score and p-value under randomisation (and under normality), and an optional permutation pseudo p-value with a visible seed. Draws the Moran scatter plot (value against the average of its neighbours; the slope is I). The output is a copy of the layer with the standardised value and its spatial lag added.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params,
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = inputs.get("in").ok_or_else(|| anyhow!("Moran's I needs an input layer"))?;
        let field = p.req_str("field", "Value field")?;
        let (perms, seed) = (perms_of(p)?, seed_of(p)?);
        let d = prepare(ctx, rel, &field)?;
        if d.vals.len() < 4 {
            bail!("Moran's I needs at least 4 features with a value and a geometry ({} found).", d.vals.len());
        }
        let w = make_weights(ctx, rel, p, &d.used, &d.co)?;
        let ws = w.row_standardised();
        let m = weights::moran(&d.vals, &ws).map_err(|e| anyhow!(e))?;
        let mut pseudo = None;
        if perms > 0 {
            let n = d.vals.len();
            let mean = d.vals.iter().sum::<f64>() / n as f64;
            let z: Vec<f64> = d.vals.iter().map(|v| v - mean).collect();
            let m2: f64 = z.iter().map(|v| v * v).sum();
            let s0: f64 = ws.iter().flatten().map(|(_, v)| v).sum();
            let stat = |zz: &[f64]| -> f64 { n as f64 / s0 * ws.iter().enumerate().map(|(i, r)| r.iter().map(|(j, v)| v * zz[i] * zz[*j]).sum::<f64>()).sum::<f64>() / m2 };
            let mut rng = Rng::new(seed);
            let mut perm = z.clone();
            let (mut hi, mut lo) = (0usize, 0usize);
            let task = crate::progress::Task::start("Moran's I: permutations").cancellable();
            for k in 0..perms {
                task.check()?;
                if k % 50 == 0 {
                    task.set(k as f64 / perms as f64, None::<String>);
                }
                rng.shuffle(&mut perm);
                let ip = stat(&perm);
                if ip >= m.i {
                    hi += 1;
                }
                if ip <= m.i {
                    lo += 1;
                }
            }
            pseudo = Some((2.0 * ((hi + 1).min(lo + 1)) as f64 / (perms as f64 + 1.0)).min(1.0));
        }
        let mut r = result(format!("Global Moran's I of {field} with row-standardised {} weights", w.kind), &[C_MORAN, C_WEIGHTS]);
        r.tables.push(table(
            "Global Moran's I",
            vec![col("n", K::Integer), col("morans_i", K::Number), col("expected_i", K::Number), col("variance", K::Number), col("z", K::Number), col("p", K::Number), col("z_normality", K::Number), col("p_normality", K::Number), col("pseudo_p", K::Number), col("permutations", K::Integer), col("weights", K::Text)],
            vec![vec![json!(m.n), num(m.i), num(m.expected), num(m.var_randomisation), num(m.z), num(m.p), num(m.z_normality), num(m.p_normality), opt(pseudo), json!(perms), json!(w.kind)]],
        ));
        r.n_used = Some(d.vals.len() as u64);
        r.dropped = d.dropped.clone();
        if perms > 0 {
            r.seed = Some(seed);
        }
        if let Some(c) = moran_scatter(&d.vals, &ws, &field) {
            r.charts.push(c);
        }
        let verdict = if m.p < 0.05 { if m.i > m.expected { "Similar values cluster together (positive spatial autocorrelation)." } else { "Neighbouring values are more different than chance (negative spatial autocorrelation)." } } else { "No significant spatial autocorrelation at the 0.05 level." };
        r.notes.push(format!("{verdict} I near +1: high values near high and low near low; near -1: a checkerboard; the expected value under no pattern is -1/(n-1) = {:.4}.", m.expected));
        r.notes.push("p is two-sided, from the z-score under randomisation (Cliff and Ord 1981); p_normality assumes the values are normal. The pseudo p-value is (the number of random reshufflings at least as extreme, plus 1) / (permutations + 1), doubled for two sides.".into());
        if let Some(n) = island_note(&w) {
            r.notes.push(n);
        }
        unprojected_note(&d.co, &mut r);
        r.notes.push(SPATIAL_NOTE.to_string());
        let mean = d.vals.iter().sum::<f64>() / d.vals.len() as f64;
        let sd = (d.vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / d.vals.len() as f64).sqrt();
        let zs: Vec<f64> = d.vals.iter().map(|v| (v - mean) / sd).collect();
        let lag = weights::lag(&ws, &zs);
        let mut vals: Vec<Vec<Option<f64>>> = vec![vec![None; d.total]; 2];
        for (i, row) in d.used.iter().enumerate() {
            vals[0][*row] = Some(zs[i]);
            vals[1][*row] = Some(lag[i]);
        }
        let sql = join_columns_sql(&rel.table, &["z_value", "lag_z"], &vals);
        ctx.set_stat(r);
        Ok(Output::Select { sql, crs: rel.crs.clone(), view: false })
    }
}

// --------------------------------------------------------------------- LISA

struct LisaTool;

impl Tool for LisaTool {
    fn spec(&self) -> ToolSpec {
        let mut params = vec![
            Param::new("field", "Value field", ParamKind::Field).field_of("in"),
            Param::new("permutations", "Permutations", ParamKind::Number).default(999),
            Param::new("seed", "Random seed", ParamKind::Number).default(12345).help("Shown in the result."),
            Param::new("alpha", "Significance level", ParamKind::Number).default(0.05),
            Param::new("adjust", "Adjust p-values for the number of features", ParamKind::Enum).default("none").options(&[("none", "No adjustment"), ("holm", "Holm"), ("bh", "Benjamini-Hochberg (false discovery rate)")]).help("A local test is run for every feature, so some will look significant by chance; adjustment controls that."),
        ];
        params.extend(weights_params());
        ToolSpec {
            id: "stat_lisa",
            name: "Local Moran's I (Cluster and Outlier)",
            category: "Statistics › Spatial",
            role: Role::Tool,
            description: "Finds where clusters and spatial outliers are: for each feature, Local Moran's I (Anselin 1995) with a conditional permutation p-value (the feature's own value is held fixed). Classifies significant features as High-High or Low-Low clusters, or Low-High and High-Low outliers. The output is a copy of the layer with local I, standardised value, lag, p-value, adjusted p-value and cluster code (1 High-High, 2 Low-High, 3 Low-Low, 4 High-Low, 0 not significant).",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params,
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = inputs.get("in").ok_or_else(|| anyhow!("Local Moran needs an input layer"))?;
        let field = p.req_str("field", "Value field")?;
        let (perms, seed) = (perms_of(p)?, seed_of(p)?);
        let alpha = p.f64("alpha").unwrap_or(0.05);
        if !(alpha > 0.0 && alpha < 1.0) {
            bail!("'Significance level' must be between 0 and 1 (0.05 is usual)");
        }
        let how = p.str("adjust").unwrap_or_else(|| "none".into());
        let d = prepare(ctx, rel, &field)?;
        if d.vals.len() < 4 {
            bail!("Local Moran needs at least 4 features with a value and a geometry ({} found).", d.vals.len());
        }
        let w = make_weights(ctx, rel, p, &d.used, &d.co)?;
        let ws = w.row_standardised();
        let task = crate::progress::Task::start("Local Moran: permutations").cancellable();
        let l = weights::local_moran(&d.vals, &ws, perms, seed, &|| crate::progress::run_cancelled()).map_err(|e| anyhow!(e))?;
        drop(task);
        let ps: Vec<f64> = l.iter().map(|x| x.p).collect();
        let ok: Vec<usize> = (0..ps.len()).filter(|i| ps[*i].is_finite()).collect();
        let adj_ok = adjust(&ok.iter().map(|i| ps[*i]).collect::<Vec<_>>(), &how);
        let mut padj = vec![f64::NAN; ps.len()];
        for (k, i) in ok.iter().enumerate() {
            padj[*i] = adj_ok[k];
        }
        let code = |i: usize| -> f64 {
            let q = l[i].quadrant;
            if perms == 0 || !padj[i].is_finite() || padj[i] > alpha {
                0.0
            } else {
                q as f64
            }
        };
        let names = ["Not significant", "High-High", "Low-High", "Low-Low", "High-Low"];
        let mut counts = [0usize; 5];
        for i in 0..l.len() {
            counts[code(i) as usize] += 1;
        }
        let mut cites = vec![C_LISA, C_WEIGHTS];
        if let Some(_) = adjust_label(&how) {
            cites.push(if how == "holm" { C_HOLM } else { C_BH });
        }
        let mut r = result(format!("Local Moran's I (LISA) of {field}, {} permutations, {} weights, alpha {alpha}{}", perms, w.kind, adjust_label(&how).map_or(String::new(), |a| format!(", p-values adjusted by {a}"))), &cites);
        r.tables.push(table("Clusters and outliers", vec![col("cluster", K::Text), col("code", K::Integer), col("features", K::Integer)], (0..5).map(|k| vec![json!(names[k]), json!(k), json!(counts[k])]).collect()));
        r.n_used = Some(d.vals.len() as u64);
        r.dropped = d.dropped.clone();
        r.seed = Some(seed);
        if perms == 0 {
            r.notes.push("With 0 permutations there is no significance test, so no feature is classified; the local I values are still in the output layer.".into());
        }
        if perms > 0 && perms + 1 < (1.0 / alpha).ceil() as usize * 2 {
            r.notes.push(format!("{perms} permutations cannot resolve p-values near {alpha}; use at least {}.", (2.0 / alpha).ceil() as usize));
        }
        if let Some(c) = moran_scatter(&d.vals, &ws, &field) {
            r.charts.push(c);
        }
        r.notes.push("Quadrants refer to the feature's value and the average of its neighbours, relative to the overall mean: High-High and Low-Low are clusters, Low-High and High-Low are spatial outliers. Pseudo p-values are two-sided: (the permuted values at least as extreme + 1) / (permutations + 1), doubled.".into());
        if how == "none" {
            r.notes.push(format!("No multiple-testing adjustment was applied: with {} features, about {:.0} would look significant at {alpha} by chance alone. Consider Holm or Benjamini-Hochberg.", l.len(), alpha * l.len() as f64));
        }
        if let Some(n) = island_note(&w) {
            r.notes.push(n);
        }
        unprojected_note(&d.co, &mut r);
        r.notes.push(SPATIAL_NOTE.to_string());
        let mut vals: Vec<Vec<Option<f64>>> = vec![vec![None; d.total]; 6];
        for (i, row) in d.used.iter().enumerate() {
            vals[0][*row] = Some(l[i].i);
            vals[1][*row] = Some(l[i].z);
            vals[2][*row] = Some(l[i].lag_z);
            vals[3][*row] = Some(l[i].p);
            vals[4][*row] = Some(padj[i]);
            vals[5][*row] = Some(code(i));
        }
        let sql = join_columns_sql(&rel.table, &["local_i", "z_value", "lag_z", "p_value", "p_adjusted", "cluster_code"], &vals);
        ctx.set_stat(r);
        let _ = Value::Null;
        Ok(Output::Select { sql, crs: rel.crs.clone(), view: false })
    }
}
