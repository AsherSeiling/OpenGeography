//! Statistics › Regression (package ST5): ordinary least squares with numeric and categorical terms, and logistic regression. The maths is in
//! `stats/regression.rs` (QR, checked against R on Anscombe, mtcars and the NIST Wampler data). Each tool reports coefficients with intervals,
//! fit statistics, VIF, a Breusch-Pagan test, residual charts and Moran's I of the residuals when the rows are places; the node's output is the
//! input layer with the fitted values and residuals added, as a new layer (the source is never changed).

use super::stat_geo::*;
use super::stat_support::*;
use super::*;
use crate::stats::inference::{pnorm, shapiro_wilk};
use crate::stats::regression::{self as reg, auc};
use crate::stats::weights::moran;
use crate::stats::{charts, ColumnKind as K, Dropped};
use serde_json::{json, Value};

const C_OLS: &str = "Golub, G. H. and Van Loan, C. F. (2013). Matrix Computations, 4th ed. Johns Hopkins (QR least squares); Draper, N. R. and Smith, H. (1998). Applied Regression Analysis, 3rd ed. Wiley.";
const C_VIF: &str = "Belsley, D. A., Kuh, E. and Welsch, R. E. (1980). Regression Diagnostics. Wiley (variance inflation).";
const C_BP: &str = "Breusch, T. S. and Pagan, A. R. (1979). Econometrica 47, 1287-1294; Koenker, R. (1981). Journal of Econometrics 17, 107-112 (studentised form).";
const C_COOK: &str = "Cook, R. D. (1977). Detection of influential observations in linear regression. Technometrics 19, 15-18.";
const C_MORAN: &str = "Moran, P. A. P. (1950). Notes on continuous stochastic phenomena. Biometrika 37, 17-23; Cliff, A. D. and Ord, J. K. (1981). Spatial Processes. Pion (variance under randomisation); applied to residuals as in Anselin (1988), Spatial Econometrics, Kluwer.";
const C_LOGIT: &str = "McCullagh, P. and Nelder, J. A. (1989). Generalized Linear Models, 2nd ed. Chapman and Hall (iteratively reweighted least squares); Nelder, J. A. and Wedderburn, R. W. M. (1972). J. Royal Statistical Society A 135, 370-384.";
const C_MCF: &str = "McFadden, D. (1974). Conditional logit analysis of qualitative choice behavior. In Zarembka (ed.), Frontiers in Econometrics, Academic Press (pseudo R-squared).";
const C_AUC: &str = "Hanley, J. A. and McNeil, B. J. (1982). The meaning and use of the area under a receiver operating characteristic curve. Radiology 143, 29-36.";

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(OlsTool), Box::new(LogitTool)]
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "stat_ols" => &["regression", "linear regression", "ols", "least squares", "r squared", "predict", "residuals", "coefficients", "multiple regression", "vif", "multicollinearity"],
        "stat_logistic" => &["logistic regression", "logit", "binary outcome", "odds ratio", "probability model", "classification", "presence absence"],
        _ => &[],
    }
}

pub(super) struct Design {
    pub(super) y: Vec<f64>,
    pub(super) names: Vec<String>,
    pub(super) cols: Vec<Vec<f64>>,
    /// Table row of each used observation.
    pub(super) used: Vec<usize>,
    pub(super) total: usize,
    pub(super) dropped: Vec<Dropped>,
    pub(super) notes: Vec<String>,
    /// Names of the numeric predictors and of the dummy columns of each categorical term, for the report.
    pub(super) levels: Vec<(String, Vec<String>)>,
}

fn same_value(txt: &str, success: &str) -> bool {
    let t = txt.trim();
    t == success || matches!((t.parse::<f64>(), success.parse::<f64>()), (Ok(a), Ok(b)) if a == b)
}

/// Load the outcome, numeric predictors and categorical terms; a row is used when every one of them has a value.
pub(super) fn build_design(ctx: &Ctx, rel: &Rel, y: &str, success: Option<&str>, preds: &[String], cats: &[String]) -> Result<Design> {
    let mut all: Vec<&str> = vec![y];
    all.extend(preds.iter().map(|s| s.as_str()));
    all.extend(cats.iter().map(|s| s.as_str()));
    for (i, a) in all.iter().enumerate() {
        if all[..i].contains(a) {
            bail!("'{a}' is listed more than once (the outcome, the predictors and the categorical terms must all be different fields)");
        }
    }
    if preds.is_empty() && cats.is_empty() {
        bail!("Choose at least one predictor (a numeric field, or a categorical field)");
    }
    let mut wanted: Vec<(&str, bool)> = vec![(y, success.is_none())];
    wanted.extend(preds.iter().map(|s| (s.as_str(), true)));
    wanted.extend(cats.iter().map(|s| (s.as_str(), false)));
    let d = load(ctx, rel, &wanted)?;
    let np = preds.len();
    let mut used = vec![];
    let mut counts = vec![0u64; wanted.len()];
    for r in 0..d.rows {
        let ok = |c: usize| -> bool {
            if c == 0 && success.is_some() {
                d.cols[0].txt[r].is_some()
            } else if c <= np {
                d.cols[c].num[r].is_some()
            } else {
                d.cols[c].txt[r].is_some()
            }
        };
        match (0..wanted.len()).find(|c| !ok(*c)) {
            Some(c) => counts[c] += 1,
            None => used.push(r),
        }
    }
    let dropped: Vec<Dropped> = counts.iter().enumerate().filter(|(_, c)| **c > 0).map(|(i, c)| Dropped { reason: format!("{}: empty, missing or not a usable value", wanted[i].0), count: *c }).collect();
    let yv: Vec<f64> = used
        .iter()
        .map(|r| match success {
            Some(s) => {
                if same_value(d.cols[0].txt[*r].as_ref().unwrap(), s) {
                    1.0
                } else {
                    0.0
                }
            }
            None => d.cols[0].num[*r].unwrap(),
        })
        .collect();
    let mut names = vec![];
    let mut cols = vec![];
    let mut levels = vec![];
    for (i, p) in preds.iter().enumerate() {
        names.push(p.clone());
        cols.push(used.iter().map(|r| d.cols[1 + i].num[*r].unwrap()).collect::<Vec<f64>>());
    }
    let mut notes = vec![];
    for (i, c) in cats.iter().enumerate() {
        let col = &d.cols[1 + np + i];
        let mut lv: Vec<String> = used.iter().map(|r| col.txt[*r].clone().unwrap()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        sort_levels(&mut lv);
        if lv.len() < 2 {
            bail!("The categorical field '{c}' has only one value among the rows used, so it cannot enter the model");
        }
        if lv.len() > 50 {
            bail!("The categorical field '{c}' has {} values; at most 50 are allowed. Group the values first.", lv.len());
        }
        let mut dummies = vec![];
        for l in &lv[1..] {
            let name = format!("{c}[{l}]");
            cols.push(used.iter().map(|r| if col.txt[*r].as_deref() == Some(l.as_str()) { 1.0 } else { 0.0 }).collect::<Vec<f64>>());
            names.push(name.clone());
            dummies.push(name);
        }
        notes.push(format!("'{c}' is coded with dummy (0/1) columns; the reference level is {} (the first in order), so each coefficient is the difference from it.", lv[0]));
        levels.push((c.clone(), dummies));
    }
    if used.len() > 2_000_000 {
        bail!("{} rows is more than the regression holds in memory (2,000,000); use a sample.", used.len());
    }
    Ok(Design { y: yv, names, cols, used, total: d.rows, dropped, notes, levels })
}

fn coef_cols_ols() -> Vec<crate::stats::StatColumn> {
    let mut c = vec![col("term", K::Text)];
    for n in ["estimate", "std_error", "t", "p", "ci_low", "ci_high", "vif"] {
        c.push(col(n, K::Number));
    }
    c
}

pub(super) fn moran_of(ctx: &Ctx, rel: &Rel, p: &Params, d: &Design, resid: &[f64], r: &mut crate::stats::StatResult) -> Result<()> {
    if !(rel.has_geom && rel.crs.is_some()) || p.str("residual_moran").as_deref() == Some("no") {
        return Ok(());
    }
    let co = coords(ctx, rel)?;
    let keep: Vec<usize> = (0..d.used.len()).filter(|i| co.xy[d.used[*i]].is_some()).collect();
    let rows: Vec<usize> = keep.iter().map(|i| d.used[*i]).collect();
    if rows.len() < 10 {
        r.notes.push("Moran's I of the residuals was skipped: fewer than 10 rows have a geometry.".into());
        return Ok(());
    }
    if rows.len() < d.used.len() {
        r.notes.push(format!("Moran's I of the residuals leaves out {} row(s) without a geometry.", d.used.len() - rows.len()));
    }
    let w = make_weights(ctx, rel, p, &rows, &co)?;
    let res: Vec<f64> = keep.iter().map(|i| resid[*i]).collect();
    if let Some(n) = island_note(&w) {
        r.notes.push(n);
    }
    match moran(&res, &w.row_standardised()) {
        Ok(m) => {
            r.tables.push(table("Residual spatial autocorrelation", vec![col("morans_i", K::Number), col("expected", K::Number), col("z", K::Number), col("p", K::Number), col("n", K::Integer), col("weights", K::Text)], vec![vec![num(m.i), num(m.expected), num(m.z), num(m.p), json!(m.n), json!(w.kind)]]));
            if m.p < 0.05 {
                r.notes.push(format!("The residuals are spatially autocorrelated (Moran's I = {:.3}, p = {:.4}, {}). Nearby features share error, so the standard errors and p-values above are too optimistic; consider a spatial lag or spatial error model, or Geographically Weighted Regression.", m.i, m.p, w.kind));
            } else {
                r.notes.push(format!("No spatial autocorrelation of the residuals was found (Moran's I = {:.3}, p = {:.4}, {}); this does not prove independence.", m.i, m.p, w.kind));
            }
            if let Some(c) = moran_scatter(&res, &w.row_standardised(), "residuals") {
                r.charts.push(c);
            }
        }
        Err(e) => r.notes.push(format!("Moran's I of the residuals could not be computed: {e}")),
    }
    r.citations.push(C_MORAN.into());
    r.notes.push(SPATIAL_NOTE.to_string());
    Ok(())
}

// ----------------------------------------------------------------------- OLS

struct OlsTool;

impl Tool for OlsTool {
    fn spec(&self) -> ToolSpec {
        let mut params = vec![
            Param::new("dependent", "Dependent field", ParamKind::Field).field_of("in").help("The numeric field to explain."),
            Param::new("predictors", "Numeric predictors", ParamKind::Text).optional().help("Field names separated by commas."),
            Param::new("categorical", "Categorical predictors", ParamKind::Text).optional().help("Field names separated by commas; each is turned into 0/1 columns, leaving the first value as the reference."),
            Param::new("conf", "Confidence level", ParamKind::Number).default(0.95),
            Param::new("residual_moran", "Test residuals for spatial autocorrelation", ParamKind::Enum).default("yes").options(&[("yes", "Yes (layers with geometry)"), ("no", "No")]),
        ];
        params.extend(weights_params());
        ToolSpec {
            id: "stat_ols",
            name: "Linear Regression (OLS)",
            category: "Statistics › Regression",
            role: Role::Tool,
            description: "Ordinary least squares regression of a numeric field on numeric and categorical predictors. Reports coefficients with standard errors, t, p and confidence intervals, R-squared, adjusted R-squared, the F test, variance inflation factors, the Breusch-Pagan test of constant variance, a residual-versus-fitted chart and Q-Q chart, and for layers Moran's I of the residuals. The output is the input layer with fitted value, residual, standardised residual, leverage and Cook's distance added as new fields; the source layer is not changed.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params,
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = inputs.get("in").ok_or_else(|| anyhow!("Linear Regression needs an input table"))?;
        let y = p.req_str("dependent", "Dependent field")?;
        let preds = split_fields(&p.str("predictors").unwrap_or_default());
        let cats = split_fields(&p.str("categorical").unwrap_or_default());
        let conf = p.f64("conf").unwrap_or(0.95);
        if !(conf > 0.5 && conf < 1.0) {
            bail!("'Confidence level' must be between 0.5 and 1");
        }
        let d = build_design(ctx, rel, &y, None, &preds, &cats)?;
        let o = reg::ols(&d.cols, &d.y, &d.names, conf).map_err(|e| anyhow!(e))?;
        let vifs = reg::vif(&d.cols);
        let mut rows = vec![];
        for j in 0..o.k {
            rows.push(vec![json!(if j == 0 { "(Intercept)".to_string() } else { d.names[j - 1].clone() }), num(o.beta[j]), num(o.se[j]), num(o.t[j]), num(o.p[j]), num(o.ci[j].0), num(o.ci[j].1), if j == 0 { Value::Null } else { opt(vifs[j - 1]) }]);
        }
        let mut r = result(
            format!("Ordinary least squares regression of {y} on {} predictor term(s) ({} coefficients including the intercept), solved by QR decomposition", preds.len() + cats.len(), o.k),
            &[C_OLS, C_VIF, C_BP, C_COOK],
        );
        r.tables.push(table("Coefficients", coef_cols_ols(), rows));
        let bp = reg::breusch_pagan(&d.cols, &o.resid);
        let fit_row = vec![json!(o.n), num(o.r2), num(o.adj_r2), num(o.sigma), num(o.f), num((o.k - 1) as f64), num(o.df_resid), num(o.f_p), opt(bp.as_ref().map(|b| b.lm)), opt(bp.as_ref().map(|b| b.df)), opt(bp.as_ref().map(|b| b.p))];
        r.tables.push(table(
            "Model fit",
            vec![col("n", K::Integer), col("r_squared", K::Number), col("adj_r_squared", K::Number), col("residual_se", K::Number), col("F", K::Number), col("df1", K::Number), col("df2", K::Number), col("F_p", K::Number), col("breusch_pagan_lm", K::Number), col("breusch_pagan_df", K::Number), col("breusch_pagan_p", K::Number)],
            vec![fit_row],
        ));
        r.n_used = Some(d.used.len() as u64);
        r.dropped = d.dropped.clone();
        r.notes = d.notes.clone();
        // Residual diagnostics.
        let std_res: Vec<f64> = (0..o.n).map(|i| o.resid[i] / (o.sigma * (1.0 - o.leverage[i]).max(1e-12).sqrt())).collect();
        let cook: Vec<f64> = (0..o.n).map(|i| std_res[i].powi(2) / o.k as f64 * o.leverage[i] / (1.0 - o.leverage[i]).max(1e-12)).collect();
        let high = cook.iter().filter(|c| **c > 4.0 / o.n as f64).count();
        if high > 0 {
            r.notes.push(format!("{high} observation(s) have Cook's distance above 4/n = {:.4}; they pull the fit more than most. Check them before trusting the coefficients.", 4.0 / o.n as f64));
        }
        if let Some(b) = &bp {
            if b.p < 0.05 {
                r.notes.push(format!("Breusch-Pagan (Koenker) test: LM = {:.3}, df = {}, p = {:.4}. The residual variance is not constant, so the standard errors and p-values are unreliable; consider a transformation or robust standard errors.", b.lm, b.df, b.p));
            } else {
                r.notes.push(format!("Breusch-Pagan (Koenker) test: LM = {:.3}, df = {}, p = {:.4}; no evidence of changing residual variance.", b.lm, b.df, b.p));
            }
        }
        if let Some(j) = vifs.iter().position(|v| v.map_or(false, |v| v > 10.0)) {
            r.notes.push(format!("Variance inflation factor above 10 for '{}' (and possibly others): the predictors overlap strongly, so individual coefficients are unstable. Rules of thumb: above 5 is a concern, above 10 is serious. Dummy columns of one categorical term share a reference level, so their VIFs can be high when a level is rare.", d.names[j]));
        }
        if let Ok((w, pv)) = shapiro_wilk(&o.resid) {
            if pv < 0.05 {
                r.notes.push(format!("Shapiro-Wilk test of the residuals: W = {w:.3}, p = {pv:.4}. The residuals depart from normality; the t and F p-values and intervals are approximate for small samples (see the Q-Q chart)."));
            }
        }
        r.notes.push(format!("Intervals are {} % t-intervals. R-squared is the share of the variation in {y} explained; adjusted R-squared penalises extra terms. A regression shows association, not cause, and only describes the range of the data.", (conf * 1000.0).round() / 10.0));
        let pts: Vec<(i64, f64, f64)> = (0..o.n).map(|i| (-1, o.fitted[i], o.resid[i])).collect();
        if let Ok(b) = charts::scatter(&pts, "Fitted values", "Residuals", false, 20_000) {
            let mut c = b.chart;
            c.title = "Residuals against fitted values".into();
            r.charts.push(c);
        }
        let items: Vec<(i64, f64)> = o.resid.iter().map(|v| (-1, *v)).collect();
        if let Ok(b) = charts::qq(&items, "residuals") {
            r.charts.push(b.chart);
        }
        moran_of(ctx, rel, p, &d, &o.resid, &mut r)?;
        // Output layer: the input with fitted values and diagnostics.
        let mut vals: Vec<Vec<Option<f64>>> = vec![vec![None; d.total]; 5];
        for (i, row) in d.used.iter().enumerate() {
            vals[0][*row] = Some(o.fitted[i]);
            vals[1][*row] = Some(o.resid[i]);
            vals[2][*row] = Some(std_res[i]);
            vals[3][*row] = Some(o.leverage[i]);
            vals[4][*row] = Some(cook[i]);
        }
        r.notes.push("The output layer adds fitted, residual, std_residual, leverage and cooks_d to every row (empty for rows left out). Map the residuals to look for spatial patterns.".into());
        let sql = join_columns_sql(&rel.table, &["fitted", "residual", "std_residual", "leverage", "cooks_d"], &vals);
        ctx.set_stat(r);
        Ok(Output::Select { sql, crs: rel.crs.clone(), view: false })
    }
}

// ------------------------------------------------------------------ logistic

struct LogitTool;

impl Tool for LogitTool {
    fn spec(&self) -> ToolSpec {
        let mut params = vec![
            Param::new("outcome", "Outcome field", ParamKind::Field).field_of("in"),
            Param::new("success", "Count as 1 (event)", ParamKind::Text).help("The value of the outcome field that counts as the event, for example 1, yes or present; every other value counts as 0. A true/false field uses true or false."),
            Param::new("predictors", "Numeric predictors", ParamKind::Text).optional().help("Field names separated by commas."),
            Param::new("categorical", "Categorical predictors", ParamKind::Text).optional(),
            Param::new("conf", "Confidence level", ParamKind::Number).default(0.95),
            Param::new("residual_moran", "Test residuals for spatial autocorrelation", ParamKind::Enum).default("yes").options(&[("yes", "Yes (layers with geometry)"), ("no", "No")]),
        ];
        params.extend(weights_params());
        ToolSpec {
            id: "stat_logistic",
            name: "Logistic Regression",
            category: "Statistics › Regression",
            role: Role::Tool,
            description: "Logistic regression of a yes/no outcome on numeric and categorical predictors, fitted by iteratively reweighted least squares. Reports coefficients with Wald intervals and odds ratios, deviance, the likelihood-ratio test, McFadden's pseudo R-squared, AIC and the area under the ROC curve, warns about perfect separation, and for layers tests the deviance residuals for spatial autocorrelation. The output is the input layer with the fitted probability and deviance residual added.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params,
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = inputs.get("in").ok_or_else(|| anyhow!("Logistic Regression needs an input table"))?;
        let y = p.req_str("outcome", "Outcome field")?;
        let success = p.req_str("success", "Count as 1 (event)")?;
        let preds = split_fields(&p.str("predictors").unwrap_or_default());
        let cats = split_fields(&p.str("categorical").unwrap_or_default());
        let conf = p.f64("conf").unwrap_or(0.95);
        if !(conf > 0.5 && conf < 1.0) {
            bail!("'Confidence level' must be between 0.5 and 1");
        }
        let d = build_design(ctx, rel, &y, Some(&success), &preds, &cats)?;
        let events = d.y.iter().filter(|v| **v > 0.5).count();
        let l = reg::logit(&d.cols, &d.y, &d.names, conf).map_err(|e| anyhow!(e))?;
        let vifs = reg::vif(&d.cols);
        let mut rows = vec![];
        for j in 0..l.beta.len() {
            rows.push(vec![
                json!(if j == 0 { "(Intercept)".to_string() } else { d.names[j - 1].clone() }),
                num(l.beta[j]),
                num(l.se[j]),
                num(l.z[j]),
                num(l.p[j]),
                num(l.ci[j].0),
                num(l.ci[j].1),
                num(l.beta[j].exp()),
                num(l.ci[j].0.exp()),
                num(l.ci[j].1.exp()),
                if j == 0 { Value::Null } else { opt(vifs[j - 1]) },
            ]);
        }
        let mut r = result(format!("Logistic regression of {y} = {success} on {} predictor term(s), logit link, iteratively reweighted least squares ({} iterations{})", preds.len() + cats.len(), l.iterations, if l.converged { "" } else { ", did not converge" }), &[C_LOGIT, C_MCF, C_AUC, C_VIF]);
        let mut cols = vec![col("term", K::Text)];
        for n in ["estimate", "std_error", "z", "p", "ci_low", "ci_high", "odds_ratio", "or_ci_low", "or_ci_high", "vif"] {
            cols.push(col(n, K::Number));
        }
        r.tables.push(table("Coefficients", cols, rows));
        let a = auc(&d.y, &l.fitted);
        r.tables.push(table(
            "Model fit",
            vec![col("n", K::Integer), col("events", K::Integer), col("deviance", K::Number), col("null_deviance", K::Number), col("df_residual", K::Number), col("lr_chi_square", K::Number), col("lr_p", K::Number), col("mcfadden_r2", K::Number), col("aic", K::Number), col("auc", K::Number)],
            vec![vec![json!(l.n), json!(events), num(l.deviance), num(l.null_deviance), num(l.df_resid), num(l.lr_chi2), num(l.lr_p), num(l.mcfadden), num(l.aic), opt(a)]],
        ));
        r.n_used = Some(d.used.len() as u64);
        r.dropped = d.dropped.clone();
        r.notes = d.notes.clone();
        if !l.converged {
            r.notes.push("The fitting did not converge in 50 iterations; treat the coefficients with suspicion.".into());
        }
        if l.near_separation || l.beta.iter().any(|b| b.abs() > 15.0) {
            r.notes.push("Some fitted probabilities are numerically 0 or 1 or a coefficient is very large: the predictors (almost) separate the two outcomes perfectly, so the estimates and standard errors are not reliable. Remove the separating predictor, merge sparse categories, or use a penalised method.".into());
        }
        if events < 10 * l.beta.len().saturating_sub(1).max(1) || d.y.len() - events < 10 * l.beta.len().saturating_sub(1).max(1) {
            r.notes.push(format!("There are {events} events and {} non-events for {} predictor coefficients; the usual rule of thumb is at least 10 of the rarer outcome per coefficient, so the estimates may be unstable.", d.y.len() - events, l.beta.len() - 1));
        }
        r.notes.push(format!("Odds ratio = exp(coefficient): the multiplicative change in the odds of {y} = {success} per unit of the predictor (or versus the reference level). Intervals are {} % Wald intervals. AUC: 0.5 is chance, 1 is perfect ranking; it is computed on the same rows the model was fitted on, so it is optimistic.", (conf * 1000.0).round() / 10.0));
        let dev_res: Vec<f64> = d
            .y
            .iter()
            .zip(&l.fitted)
            .map(|(yi, m)| {
                let a = if *yi > 0.0 { yi * (yi / m).ln() } else { 0.0 };
                let b = if *yi < 1.0 { (1.0 - yi) * ((1.0 - yi) / (1.0 - m)).ln() } else { 0.0 };
                (yi - m).signum() * (2.0 * (a + b)).max(0.0).sqrt()
            })
            .collect();
        moran_of(ctx, rel, p, &d, &dev_res, &mut r)?;
        let _ = pnorm(0.0);
        let mut vals: Vec<Vec<Option<f64>>> = vec![vec![None; d.total]; 2];
        for (i, row) in d.used.iter().enumerate() {
            vals[0][*row] = Some(l.fitted[i]);
            vals[1][*row] = Some(dev_res[i]);
        }
        let sql = join_columns_sql(&rel.table, &["fitted_probability", "deviance_residual"], &vals);
        ctx.set_stat(r);
        Ok(Output::Select { sql, crs: rel.crs.clone(), view: false })
    }
}
