//! Statistics › Spatial regression (package ST6): spatial lag and spatial error models (maximum likelihood), GWR and MGWR. The maths is in
//! `stats/spatial_reg.rs` and `stats/mgwr.rs`; checked against brute-force likelihoods, OLS equivalence and simulated data with known
//! parameters (`tests/stat_spatial_reg.rs`, `tests/stat_mgwr.rs`). Each result states which kind of check backs it. The output of each tool is a
//! copy of the input layer with the model's fitted values, residuals and (GWR) local coefficients added; the source layer is not changed.

use super::stat_geo::*;
use super::stat_regress::{build_design, moran_of, Design};
use super::stat_support::*;
use super::*;
use crate::stats::describe::quantile7;
use crate::stats::mgwr::mgwr;
use crate::stats::regression::ols;
use crate::stats::spatial_reg::*;
use crate::stats::weights::moran;
use crate::stats::{ColumnKind as K, Dropped};
use nalgebra::DMatrix;
use serde_json::{json, Value};

const C_ML: &str = "Ord, J. K. (1975). Estimation methods for models of spatial interaction. Journal of the American Statistical Association 70, 120-126 (log-determinant from eigenvalues); Anselin, L. (1988). Spatial Econometrics: Methods and Models. Kluwer (maximum likelihood, asymptotic standard errors).";
const C_GWR: &str = "Brunsdon, C., Fotheringham, A. S. and Charlton, M. E. (1996). Geographically weighted regression: a method for exploring spatial nonstationarity. Geographical Analysis 28, 281-298; Fotheringham, A. S., Brunsdon, C. and Charlton, M. (2002). Geographically Weighted Regression. Wiley.";
const C_AICC: &str = "Hurvich, C. M., Simonoff, J. S. and Tsai, C.-L. (1998). Smoothing parameter selection in nonparametric regression using an improved Akaike information criterion. Journal of the Royal Statistical Society B 60, 271-293.";
const C_MGWR: &str = "Fotheringham, A. S., Yang, W. and Kang, W. (2017). Multiscale geographically weighted regression (MGWR). Annals of the American Association of Geographers 107(6), 1247-1265.";
const C_MULTI: &str = "Wheeler, D. and Tiefelsdorf, M. (2005). Multicollinearity and correlation among local regression coefficients in geographically weighted regression. Journal of Geographical Systems 7, 161-187; da Silva, A. R. and Fotheringham, A. S. (2016). The multiple testing issue in geographically weighted regression. Geographical Analysis 48, 233-247.";
const NOTE_CHECKS: &str = "Verification: on Anselin's Columbus data this implementation reproduces the published OLS coefficients and GeoDa's Moran's I for queen weights (0.500). The lag and error estimates match an independent computation of the same likelihood (numpy, full determinant) to 5 decimals, and simulated data with known parameters. The values usually quoted from spdep for Columbus (rho 0.43102, lambda 0.56179) come from the neighbour file in Anselin's book, not from the polygons' queen contiguity, so they are not the same weights and differ from this result. GWR and MGWR are checked against OLS equivalence and simulations only; no GWmodel or mgwr output was available.";

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(LagTool), Box::new(ErrorTool), Box::new(GwrTool(false)), Box::new(GwrTool(true))]
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "stat_spatial_lag" => &["spatial lag", "sar", "spatial autoregressive", "spillover", "spatial econometrics", "rho"],
        "stat_spatial_error" => &["spatial error", "sem", "spatially correlated errors", "spatial econometrics", "lambda"],
        "stat_gwr" => &["geographically weighted regression", "gwr", "local regression", "spatially varying coefficients", "non-stationarity", "bandwidth"],
        "stat_mgwr" => &["mgwr", "multiscale gwr", "multiscale geographically weighted regression", "covariate-specific bandwidth", "spatial scale"],
        _ => &[],
    }
}

/// The design restricted to rows that have a geometry.
fn with_geometry(ctx: &Ctx, rel: &Rel, d: Design) -> Result<(Design, Coords)> {
    let co = coords(ctx, rel)?;
    let keep: Vec<usize> = (0..d.used.len()).filter(|i| co.xy[d.used[*i]].is_some()).collect();
    let mut dropped = d.dropped.clone();
    if keep.len() < d.used.len() {
        dropped.push(Dropped { reason: "no geometry (rows with every value)".into(), count: (d.used.len() - keep.len()) as u64 });
    }
    let pick = |v: &Vec<f64>| keep.iter().map(|i| v[*i]).collect::<Vec<f64>>();
    let nd = Design { y: pick(&d.y), names: d.names.clone(), cols: d.cols.iter().map(pick).collect(), used: keep.iter().map(|i| d.used[*i]).collect(), total: d.total, dropped, notes: d.notes.clone(), levels: d.levels.clone() };
    Ok((nd, co))
}

fn fields(p: &Params, key: &str) -> Vec<String> {
    split_fields(&p.str(key).unwrap_or_default())
}

fn common_params(categorical: bool) -> Vec<Param> {
    let mut v = vec![
        Param::new("dependent", "Dependent field", ParamKind::Field).field_of("in"),
        Param::new("predictors", "Numeric predictors", ParamKind::Text).help("Field names separated by commas."),
    ];
    if categorical {
        v.push(Param::new("categorical", "Categorical predictors", ParamKind::Text).optional().help("Field names separated by commas; each becomes 0/1 columns with the first value as the reference."));
    }
    v
}

fn weights_note(p: &Params) -> Param {
    let _ = p;
    Param::new("residual_moran", "Test residuals for spatial autocorrelation", ParamKind::Enum).default("yes").options(&[("yes", "Yes"), ("no", "No")])
}

fn islands_hint(w: &crate::stats::weights::Weights) -> Option<String> {
    if w.islands().is_empty() {
        None
    } else {
        island_note(w)
    }
}

// ---------------------------------------------------------- lag and error

struct LagTool;
struct ErrorTool;

fn ml_spec(id: &'static str, name: &'static str, description: &'static str) -> ToolSpec {
    let mut params = common_params(true);
    params.push(Param::new("conf", "Confidence level", ParamKind::Number).default(0.95));
    params.push(weights_note(&Params(&Default::default())));
    let mut w = weights_params();
    w[0] = Param::new("weights", "Neighbours", ParamKind::Enum).default("queen").options(&[("queen", "Queen contiguity (polygons sharing a point)"), ("rook", "Rook contiguity (polygons sharing an edge)"), ("knn", "k nearest neighbours"), ("band", "Distance band")]).help("Every feature needs at least one neighbour: use a band wide enough to reach all, or k nearest neighbours. Distances are geodesic (metres) for layers in degrees.");
    params.extend(w);
    ToolSpec { id, name, category: "Statistics › Spatial regression", role: Role::Tool, description, inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }], output: Some(PortKind::Table), params }
}

impl Tool for LagTool {
    fn spec(&self) -> ToolSpec {
        ml_spec("stat_spatial_lag", "Spatial Lag Regression", "Regression in which each feature's value also depends on its neighbours' values: y = rho W y + X beta + e, fitted by maximum likelihood (Ord 1975; Anselin 1988) with row-standardised weights. Reports the coefficients with asymptotic standard errors, the spatial parameter rho with its test, a likelihood-ratio test against ordinary least squares, AIC, and Moran's I of the residuals. In a lag model a coefficient is not the whole effect of a predictor: its change also spills over to neighbours. The output is a copy of the layer with fitted values and residuals. Limited to 2,500 features.")
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        ml_build(ctx, inputs, p, true)
    }
}

impl Tool for ErrorTool {
    fn spec(&self) -> ToolSpec {
        ml_spec("stat_spatial_error", "Spatial Error Regression", "Regression whose errors are spatially correlated: y = X beta + u, u = lambda W u + e, fitted by maximum likelihood (Ord 1975; Anselin 1988) with row-standardised weights. Coefficients keep their ordinary meaning, with standard errors that allow for the correlated errors. Reports the spatial error parameter lambda with its test, a likelihood-ratio test against ordinary least squares, AIC, and Moran's I of the residuals. The output is a copy of the layer with fitted values and residuals. Limited to 2,500 features.")
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        ml_build(ctx, inputs, p, false)
    }
}

fn ml_build(ctx: &Ctx, inputs: &Inputs, p: &Params, lag_model: bool) -> Result<Output> {
    let rel = inputs.get("in").ok_or_else(|| anyhow!("The spatial regression needs an input layer"))?;
    let y = p.req_str("dependent", "Dependent field")?;
    let (preds, cats) = (fields(p, "predictors"), fields(p, "categorical"));
    let conf = p.f64("conf").unwrap_or(0.95);
    if !(conf > 0.5 && conf < 1.0) {
        bail!("'Confidence level' must be between 0.5 and 1");
    }
    let d0 = build_design(ctx, rel, &y, None, &preds, &cats)?;
    let (d, co) = with_geometry(ctx, rel, d0)?;
    let n = d.used.len();
    if n > MAX_ML {
        bail!("Maximum likelihood spatial regression is limited to {MAX_ML} features ({n} given); use a subset or Geographically Weighted Regression.");
    }
    let w = make_weights(ctx, rel, p, &d.used, &co)?;
    let ws = w.row_standardised();
    let task = crate::progress::Task::start("Spatial regression: eigenvalues of the weights").cancellable();
    let eigs = eigenvalues(&ws, w.is_symmetric());
    task.check()?;
    drop(task);
    let m = if lag_model { sar(&d.cols, &d.y, &d.names, &ws, &eigs) } else { sem(&d.cols, &d.y, &d.names, &ws, &eigs) }.map_err(|e| anyhow!(e))?;
    let o = ols(&d.cols, &d.y, &d.names, conf).map_err(|e| anyhow!(e))?;
    let zc = crate::stats::inference::qnorm(1.0 - (1.0 - conf) / 2.0);
    let sp_name = if lag_model { "rho" } else { "lambda" };
    let mut r = result(
        if lag_model { format!("Spatial lag model (SAR) of {y} by maximum likelihood, row-standardised {} weights", w.kind) } else { format!("Spatial error model (SEM) of {y} by maximum likelihood, row-standardised {} weights", w.kind) },
        &[C_ML],
    );
    let mut rows = vec![];
    for j in 0..m.beta.len() {
        rows.push(vec![json!(if j == 0 { "(Intercept)".to_string() } else { d.names[j - 1].clone() }), num(m.beta[j]), num(m.se[j]), num(m.z[j]), num(m.p[j]), num(m.beta[j] - zc * m.se[j]), num(m.beta[j] + zc * m.se[j]), num(o.beta[j])]);
    }
    let mut cols = vec![col("term", K::Text)];
    for c in ["estimate", "std_error", "z", "p", "ci_low", "ci_high", "ols_estimate"] {
        cols.push(col(c, K::Number));
    }
    r.tables.push(table("Coefficients", cols, rows));
    r.tables.push(table(
        "Spatial parameter",
        vec![col("parameter", K::Text), col("estimate", K::Number), col("std_error", K::Number), col("z", K::Number), col("p", K::Number), col("lower_bound", K::Number), col("upper_bound", K::Number)],
        vec![vec![json!(sp_name), num(m.rho), num(m.rho_se), num(m.rho_z), num(m.rho_p), num(m.bounds.0), num(m.bounds.1)]],
    ));
    r.tables.push(table(
        "Model fit",
        vec![col("n", K::Integer), col("log_likelihood", K::Number), col("log_likelihood_ols", K::Number), col("lr_chi_square", K::Number), col("lr_p", K::Number), col("aic", K::Number), col("aic_ols", K::Number), col("sigma_squared", K::Number)],
        vec![vec![json!(m.n), num(m.loglik), num(m.loglik_ols), num(m.lr), num(m.lr_p), num(m.aic), num(-2.0 * m.loglik_ols + 2.0 * (o.k as f64 + 1.0)), num(m.sigma2)]],
    ));
    r.n_used = Some(n as u64);
    r.dropped = d.dropped.clone();
    r.notes = d.notes.clone();
    if m.lr_p < 0.05 {
        r.notes.push(format!("The likelihood-ratio test prefers the spatial model to ordinary least squares (chi-square = {:.3}, p = {:.4}); AIC {:.2} against {:.2} for OLS.", m.lr, m.lr_p, m.aic, -2.0 * m.loglik_ols + 2.0 * (o.k as f64 + 1.0)));
    } else {
        r.notes.push(format!("The likelihood-ratio test finds no gain over ordinary least squares (chi-square = {:.3}, p = {:.4}).", m.lr, m.lr_p));
    }
    if lag_model {
        r.notes.push("Spatial lag: the coefficients are not marginal effects. A change in a predictor at one feature also changes its neighbours' values through rho, so the total effect of a predictor is its coefficient times 1/(1 - rho) on average (direct and indirect impacts are not separated here).".into());
    } else {
        r.notes.push("Spatial error: the coefficients keep their ordinary meaning; the spatial term absorbs correlated error from omitted variables that vary smoothly over space.".into());
    }
    r.notes.push(format!("The standard errors are asymptotic (Anselin 1988) and the intervals are {} % normal intervals; p-values are from the normal distribution, so they are approximate in small samples. {NOTE_CHECKS}", (conf * 1000.0).round() / 10.0));
    if let Some(n) = islands_hint(&w) {
        r.notes.push(n);
    }
    // The error structure that remains: Moran's I of the innovations.
    if p.str("residual_moran").as_deref() != Some("no") && n >= 10 {
        match moran(&m.innovations, &ws) {
            Ok(mi) => {
                r.tables.push(table("Residual spatial autocorrelation", vec![col("morans_i", K::Number), col("expected", K::Number), col("z", K::Number), col("p", K::Number), col("n", K::Integer), col("weights", K::Text)], vec![vec![num(mi.i), num(mi.expected), num(mi.z), num(mi.p), json!(mi.n), json!(w.kind)]]));
                r.notes.push(format!("Moran's I of the model innovations (the part the model treats as independent): I = {:.3}, p = {:.4}{}.", mi.i, mi.p, if mi.p < 0.05 { "; spatial dependence remains, so consider other weights or predictors" } else { "; no remaining spatial autocorrelation detected" }));
            }
            Err(e) => r.notes.push(format!("Moran's I of the innovations could not be computed: {e}")),
        }
    }
    if co.geographic {
        r.notes.push("The layer is in degrees: neighbour distances are geodesic (WGS 84) between feature centroids; contiguity is topological.".into());
    }
    r.notes.push(SPATIAL_NOTE.to_string());
    let mut vals: Vec<Vec<Option<f64>>> = vec![vec![None; d.total]; 3];
    for (i, row) in d.used.iter().enumerate() {
        vals[0][*row] = Some(m.fitted[i]);
        vals[1][*row] = Some(m.resid[i]);
        vals[2][*row] = Some(m.innovations[i]);
    }
    let sql = join_columns_sql(&rel.table, &["fitted", "residual", "innovation"], &vals);
    ctx.set_stat(r);
    let _ = moran_of;
    Ok(Output::Select { sql, crs: rel.crs.clone(), view: false })
}

// ---------------------------------------------------------------- GWR, MGWR

struct GwrTool(bool);

impl Tool for GwrTool {
    fn spec(&self) -> ToolSpec {
        let multi = self.0;
        let mut params = common_params(false);
        params.push(Param::new("kernel", "Kernel", ParamKind::Enum).default("bisquare").options(&[("bisquare", "Bisquare (zero beyond the bandwidth)"), ("gaussian", "Gaussian"), ("exponential", "Exponential")]));
        params.push(Param::new("bandwidth_type", "Bandwidth", ParamKind::Enum).default("adaptive").options(&[("adaptive", "Adaptive (number of nearest neighbours)"), ("fixed", "Fixed (distance)")]));
        if !multi {
            params.push(Param::new("bandwidth", "Bandwidth (empty: choose automatically)", ParamKind::Number).optional().help("Neighbours for an adaptive bandwidth, or a distance (map units, or metres for degrees) for a fixed one."));
        }
        params.push(Param::new("criterion", "Bandwidth selection", ParamKind::Enum).default("aicc").options(&[("aicc", "Corrected AIC (AICc)"), ("cv", "Cross-validation")]));
        params.push(weights_note(&Params(&Default::default())));
        params.extend(weights_params());
        ToolSpec {
            id: if multi { "stat_mgwr" } else { "stat_gwr" },
            name: if multi { "Multiscale GWR (MGWR)" } else { "Geographically Weighted Regression (GWR)" },
            category: "Statistics › Spatial regression",
            role: Role::Tool,
            description: if multi {
                "Geographically weighted regression in which every predictor gets its own bandwidth (Fotheringham, Yang and Kang 2017), so a relationship that is the same everywhere is separated from one that changes over short distances. Bandwidths are chosen term by term by backfitting. Reports each term's bandwidth and effective number of parameters, local coefficient summaries, AICc against the global model, and Moran's I of the residuals. The output is a copy of the layer with local coefficients, standard errors, t-values, fitted values and residuals. Limited to 600 features."
            } else {
                "Fits a separate regression at every location, weighting nearby features more (Brunsdon, Fotheringham and Charlton 1996), so coefficients can vary over the map. The bandwidth is chosen automatically by AICc or cross-validation unless you give it. Reports the global model against GWR (AICc, residual sum of squares, effective parameters), a summary of each local coefficient, and Moran's I of the residuals. The output is a copy of the layer with local coefficients, standard errors, t-values, local R-squared, fitted values and residuals. Limited to 4,000 features."
            },
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params,
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let multi = self.0;
        let rel = inputs.get("in").ok_or_else(|| anyhow!("GWR needs an input layer"))?;
        let y = p.req_str("dependent", "Dependent field")?;
        let preds = fields(p, "predictors");
        if preds.is_empty() {
            bail!("Choose at least one numeric predictor");
        }
        let d0 = build_design(ctx, rel, &y, None, &preds, &[])?;
        let (d, co) = with_geometry(ctx, rel, d0)?;
        let n = d.used.len();
        let k = preds.len() + 1;
        if n < k + 10 {
            bail!("{n} features is too few for {k} coefficients at every location; GWR needs many more features than coefficients.");
        }
        let kernel = Kernel::parse(p.str("kernel").as_deref());
        let adaptive = p.str("bandwidth_type").as_deref() != Some("fixed");
        let crit = if p.str("criterion").as_deref() == Some("cv") { Criterion::Cv } else { Criterion::Aicc };
        let xy: Vec<(f64, f64)> = d.used.iter().map(|r| co.xy[*r].unwrap()).collect();
        let cancel = || crate::progress::run_cancelled();
        let dist = Dist::new(&xy, co.geographic, if multi { MAX_MGWR } else { MAX_GWR }, &cancel).map_err(|e| anyhow!(e))?;
        let x = DMatrix::from_fn(n, k, |i, j| if j == 0 { 1.0 } else { d.cols[j - 1][i] });
        let mut names = vec!["Intercept".to_string()];
        names.extend(preds.iter().cloned());
        let o = ols(&d.cols, &d.y, &d.names, 0.95).map_err(|e| anyhow!(e))?;
        let nf = n as f64;
        let ols_aicc = nf * (o.rss / nf).ln() + nf * (2.0 * std::f64::consts::PI).ln() + nf * (nf + k as f64) / (nf - 2.0 - k as f64);
        let tss = o.tss;
        let task = crate::progress::Task::start(if multi { "MGWR: choosing bandwidths and backfitting" } else { "GWR: selecting the bandwidth and fitting" }).cancellable();
        let mut r;
        let (beta, se, fitted, resid, local_r2, aicc, rss, enp_total);
        let mut summary_bw = vec![];
        if multi {
            let m = mgwr(&x, &d.y, &dist, adaptive, kernel, crit, &cancel).map_err(|e| anyhow!(e))?;
            task.check()?;
            r = result(format!("Multiscale GWR (MGWR) of {y} on {} predictors, {} kernel with {} bandwidths chosen term by term (criterion {})", preds.len(), kernel.name(), if adaptive { "adaptive" } else { "fixed" }, if crit == Criterion::Aicc { "AICc" } else { "cross-validation" }), &[C_MGWR, C_GWR, C_AICC, C_MULTI]);
            let mut rows = vec![];
            for j in 0..k {
                rows.push(vec![json!(names[j]), num(m.bandwidths[j]), num(m.enp[j]), num(o.beta[j])]);
                summary_bw.push(m.bandwidths[j]);
            }
            r.tables.push(table("Bandwidths", vec![col("term", K::Text), col(if adaptive { "bandwidth_neighbours" } else { "bandwidth_distance" }, K::Number), col("effective_parameters", K::Number), col("ols_estimate", K::Number)], rows));
            r.notes.push("A small bandwidth means the relationship changes over short distances; a bandwidth near the number of features (or a very large distance) means it is close to the same everywhere. Bandwidths are chosen term by term on partial residuals; inference uses the hat matrices of the converged backfit (n - 2 tr S + tr S'S degrees of freedom).".into());
            beta = m.beta;
            se = m.se;
            fitted = m.fitted;
            resid = m.resid;
            local_r2 = vec![f64::NAN; n];
            aicc = m.aicc;
            rss = m.rss;
            enp_total = m.tr_s;
        } else {
            let bw = match p.f64("bandwidth") {
                Some(b) => {
                    if adaptive && !(b.fract() == 0.0 && b >= (k + 2) as f64 && b <= nf) {
                        bail!("An adaptive bandwidth is a whole number of neighbours from {} to {n}", k + 2);
                    }
                    if !adaptive && !(b > 0.0) {
                        bail!("A fixed bandwidth must be a positive distance");
                    }
                    b
                }
                None => gwr_select(&x, &d.y, &dist, adaptive, kernel, crit).map_err(|e| anyhow!(e))?,
            };
            task.check()?;
            let g = gwr_fit(&x, &d.y, &dist, bw, adaptive, kernel, true).map_err(|e| anyhow!(e))?;
            r = result(format!("Geographically weighted regression of {y} on {} predictors, {} kernel, {} bandwidth {}{}", preds.len(), kernel.name(), if adaptive { "adaptive" } else { "fixed" }, if adaptive { format!("{bw:.0} neighbours") } else { format!("{bw:.4}") }, if p.f64("bandwidth").is_none() { format!(" chosen by {}", if crit == Criterion::Aicc { "AICc" } else { "cross-validation" }) } else { " given by the user".to_string() }), &[C_GWR, C_AICC, C_MULTI]);
            r.tables.push(table("Bandwidth", vec![col("bandwidth", K::Number), col("type", K::Text), col("kernel", K::Text), col("effective_parameters", K::Number), col("residual_df", K::Number), col("sigma_squared", K::Number)], vec![vec![num(bw), json!(if adaptive { "adaptive (neighbours)" } else { "fixed (distance)" }), json!(kernel.name()), num(g.enp), num(g.delta1), num(g.sigma2)]]));
            summary_bw.push(bw);
            if adaptive && bw >= nf - 1.0 {
                r.notes.push("The selected bandwidth uses (almost) every feature: the data do not show the relationships changing over space, and the global regression is as good.".into());
            }
            beta = g.beta;
            se = g.se;
            fitted = g.fitted;
            resid = g.resid;
            local_r2 = g.local_r2;
            aicc = g.aicc;
            rss = g.rss;
            enp_total = g.enp;
        }
        drop(task);
        // Global against local.
        r.tables.insert(
            0,
            table(
                "Global OLS and GWR",
                vec![col("model", K::Text), col("n", K::Integer), col("effective_parameters", K::Number), col("rss", K::Number), col("r_squared", K::Number), col("aicc", K::Number)],
                vec![vec![json!("Global OLS"), json!(n), num(k as f64), num(o.rss), num(o.r2), num(ols_aicc)], vec![json!(if multi { "MGWR" } else { "GWR" }), json!(n), num(enp_total), num(rss), num(1.0 - rss / tss), num(aicc)]],
            ),
        );
        let verdict = if aicc < ols_aicc - 3.0 { format!("AICc favours the local model ({aicc:.2} against {ols_aicc:.2}): the relationships vary over space.") } else if aicc < ols_aicc { format!("AICc is lower for the local model ({aicc:.2} against {ols_aicc:.2}) but by less than 3, which is not a clear gain.") } else { format!("AICc does not favour the local model ({aicc:.2} against {ols_aicc:.2}); the global regression is adequate.") };
        r.notes.push(verdict);
        // Coefficient summaries.
        let mut rows = vec![];
        for j in 0..k {
            let mut v: Vec<f64> = beta.iter().map(|b| b[j]).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let sig = (0..n).filter(|i| (beta[*i][j] / se[*i][j]).abs() > 1.96).count() as f64 / nf;
            let mean_se = se.iter().map(|s| s[j]).sum::<f64>() / nf;
            rows.push(vec![json!(names[j]), num(v[0]), num(quantile7(&v, 0.25)), num(quantile7(&v, 0.5)), num(quantile7(&v, 0.75)), num(v[n - 1]), num(v.iter().sum::<f64>() / nf), num(mean_se), num(sig), num(o.beta[j])]);
        }
        r.tables.push(table(
            "Local coefficients",
            vec![col("term", K::Text), col("min", K::Number), col("q1", K::Number), col("median", K::Number), col("q3", K::Number), col("max", K::Number), col("mean", K::Number), col("mean_std_error", K::Number), col("share_abs_t_above_1_96", K::Number), col("ols_estimate", K::Number)],
            rows,
        ));
        r.n_used = Some(n as u64);
        r.dropped = d.dropped.clone();
        r.notes.extend(d.notes.clone());
        r.notes.push("Local t-values are computed at every location from the same data, so many will look significant by chance, and neighbouring local coefficients are correlated (Wheeler and Tiefelsdorf 2005; da Silva and Fotheringham 2016): read patterns of the coefficients, not individual t-values. Local standard errors use sigma-squared = RSS / (n - 2 tr S + tr S'S).".into());
        r.notes.push(NOTE_CHECKS.to_string());
        if co.geographic {
            r.notes.push("The layer is in degrees: distances are geodesic (WGS 84) between feature centroids.".into());
        }
        let _ = (&dist, &d.names);
        let dd = Design { y: d.y.clone(), names: d.names.clone(), cols: d.cols.clone(), used: d.used.clone(), total: d.total, dropped: vec![], notes: vec![], levels: vec![] };
        moran_of(ctx, rel, p, &dd, &resid, &mut r)?;
        // Output layer.
        let mut cols_out: Vec<String> = vec![];
        for nm in &names {
            for pre in ["b_", "se_", "t_"] {
                cols_out.push(format!("{pre}{nm}"));
            }
        }
        cols_out.extend(["local_r2".to_string(), "gwr_fitted".to_string(), "gwr_residual".to_string()]);
        let mut vals: Vec<Vec<Option<f64>>> = vec![vec![None; d.total]; cols_out.len()];
        for (i, row) in d.used.iter().enumerate() {
            for j in 0..k {
                vals[3 * j][*row] = Some(beta[i][j]);
                vals[3 * j + 1][*row] = Some(se[i][j]);
                vals[3 * j + 2][*row] = Some(beta[i][j] / se[i][j]);
            }
            vals[3 * k][*row] = Some(local_r2[i]);
            vals[3 * k + 1][*row] = Some(fitted[i]);
            vals[3 * k + 2][*row] = Some(resid[i]);
        }
        let refs: Vec<&str> = cols_out.iter().map(|s| s.as_str()).collect();
        let sql = join_columns_sql(&rel.table, &refs, &vals);
        let _ = Value::Null;
        ctx.set_stat(r);
        Ok(Output::Select { sql, crs: rel.crs.clone(), view: false })
    }
}
