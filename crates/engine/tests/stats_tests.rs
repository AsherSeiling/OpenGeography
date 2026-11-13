//! ST4: every test statistic against stored R (4.x) output on classic datasets (sleep, PlantGrowth, mtcars and the examples of the R help pages).
//! The R calls are named beside each check.

use engine::stats::inference::*;

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: got {a}, expected {b}");
}

const S1: [f64; 10] = [0.7, -1.6, -0.2, -1.2, -0.1, 3.4, 3.7, 0.8, 0.0, 2.0];
const S2: [f64; 10] = [1.9, 0.8, 1.1, 0.1, -0.1, 4.4, 5.5, 1.6, 4.6, 3.4];
const CTRL: [f64; 10] = [4.17, 5.58, 5.18, 6.11, 4.50, 4.61, 5.17, 4.53, 5.33, 5.14];
const TRT1: [f64; 10] = [4.81, 4.17, 4.41, 3.59, 5.87, 3.83, 6.03, 4.89, 4.32, 4.69];
const TRT2: [f64; 10] = [6.31, 5.12, 5.54, 5.50, 5.37, 5.29, 4.92, 6.15, 5.80, 5.26];
const MPG: [f64; 32] = [21.0, 21.0, 22.8, 21.4, 18.7, 18.1, 14.3, 24.4, 22.8, 19.2, 17.8, 16.4, 17.3, 15.2, 10.4, 10.4, 14.7, 32.4, 30.4, 33.9, 21.5, 15.5, 15.2, 13.3, 19.2, 27.3, 26.0, 30.4, 15.8, 19.7, 15.0, 21.4];
const WT: [f64; 32] = [2.620, 2.875, 2.320, 3.215, 3.440, 3.460, 3.570, 3.190, 3.150, 3.440, 3.440, 4.070, 3.730, 3.780, 5.250, 5.424, 5.345, 2.200, 1.615, 1.835, 2.465, 3.520, 3.435, 3.840, 3.845, 1.935, 2.140, 1.513, 3.170, 2.770, 3.570, 2.780];

fn plant() -> Vec<Vec<f64>> {
    vec![CTRL.to_vec(), TRT1.to_vec(), TRT2.to_vec()]
}

#[test]
fn t_tests_match_r_on_the_sleep_data() {
    // t.test(extra ~ group, data = sleep)
    let w = t_two(&S1, &S2, false, Alt::Two, 0.95).unwrap();
    close(w.t, -1.8608, 1e-4, "welch t");
    close(w.df, 17.776, 1e-3, "welch df");
    close(w.p, 0.07939, 1e-5, "welch p");
    close(w.ci.0, -3.3654832, 1e-6, "welch ci lo");
    close(w.ci.1, 0.2054832, 1e-6, "welch ci hi");
    // var.equal = TRUE
    let p = t_two(&S1, &S2, true, Alt::Two, 0.95).unwrap();
    close(p.df, 18.0, 0.0, "pooled df");
    close(p.p, 0.07919, 1e-4, "pooled p");
    // t.test(x, y, paired = TRUE)
    let d: Vec<f64> = S1.iter().zip(&S2).map(|(a, b)| a - b).collect();
    let pr = t_one(&d, 0.0, Alt::Two, 0.95).unwrap();
    close(pr.t, -4.0621, 1e-4, "paired t");
    close(pr.p, 0.002833, 1e-6, "paired p");
    close(pr.ci.0, -2.4598858, 1e-6, "paired ci lo");
    close(pr.ci.1, -0.7001142, 1e-6, "paired ci hi");
    // Hedges g: pooled SD, small-sample correction; the interval brackets it.
    assert!(w.g < 0.0 && w.g_ci.0 < w.g && w.g < w.g_ci.1);
    // One-sided interval is half-infinite.
    assert!(t_two(&S1, &S2, false, Alt::Less, 0.95).unwrap().ci.0.is_infinite());
}

#[test]
fn anova_tukey_welch_levene_kruskal_match_r_on_plantgrowth() {
    let g = plant();
    // summary(aov(weight ~ group))
    let a = anova(&g, 0.95).unwrap();
    close(a.f, 4.846, 1e-3, "F");
    close(a.p, 0.01591, 1e-5, "anova p");
    // eta^2 = 3.76634 / 14.2584 = 0.2641
    close(a.eta2, 0.26410, 1e-4, "eta2");
    assert!(a.eta2_ci.0 >= 0.0 && a.eta2_ci.0 < a.eta2 && a.eta2 < a.eta2_ci.1 && a.eta2_ci.1 < 1.0);
    assert!(a.omega2 < a.eta2);
    // TukeyHSD(aov(weight ~ group))
    let t = tukey(&g, 0.95).unwrap();
    let get = |a: usize, b: usize| t.iter().find(|r| r.a == a && r.b == b).unwrap();
    let r = get(0, 1); // trt1-ctrl
    close(r.diff, -0.371, 1e-9, "trt1-ctrl");
    close(r.ci.0, -1.0622161, 1e-4, "trt1-ctrl lo");
    close(r.ci.1, 0.3202161, 1e-4, "trt1-ctrl hi");
    close(r.p_adj, 0.3908711, 1e-4, "trt1-ctrl p");
    let r = get(0, 2); // trt2-ctrl
    close(r.diff, 0.494, 1e-9, "trt2-ctrl");
    close(r.p_adj, 0.1979960, 1e-4, "trt2-ctrl p");
    let r = get(1, 2); // trt2-trt1
    close(r.diff, 0.865, 1e-9, "trt2-trt1");
    close(r.ci.0, 0.1737839, 1e-4, "trt2-trt1 lo");
    close(r.ci.1, 1.5562161, 1e-4, "trt2-trt1 hi");
    close(r.p_adj, 0.0120064, 1e-4, "trt2-trt1 p");
    // kruskal.test(weight ~ group)
    let k = kruskal(&g).unwrap();
    close(k.stat, 7.9882, 1e-4, "H");
    close(k.p, 0.01842, 1e-5, "kruskal p");
    // car::leveneTest(weight ~ group) (median centre)
    let l = levene(&g, true).unwrap();
    close(l.f, 1.1192, 1e-4, "levene F");
    close(l.p, 0.3412, 1e-4, "levene p");
    // oneway.test(weight ~ group): F = 5.181, num df = 2, denom df = 17.128, p = 0.01739
    let w = welch_anova(&g).unwrap();
    close(w.f, 5.181, 2e-3, "welch F");
    close(w.df2, 17.128, 2e-3, "welch df2");
    close(w.p, 0.01739, 2e-4, "welch p");
}

#[test]
fn kruskal_help_page_example() {
    // kruskal.test(list(x, y, z)) on the R help-page data: chi-squared = 0.77143, df = 2, p-value = 0.68
    let g = vec![vec![2.9, 3.0, 2.5, 2.6, 3.2], vec![3.8, 2.7, 4.0, 2.4], vec![2.8, 3.4, 3.7, 2.2, 2.0]];
    let k = kruskal(&g).unwrap();
    close(k.stat, 0.77143, 1e-5, "H");
    close(k.p, 0.68, 5e-3, "p");
}

#[test]
fn rank_tests_match_r() {
    // wilcox.test(extra ~ group, sleep): W = 25.5, p-value = 0.06933 (ties: normal approximation with correction)
    let m = mann_whitney(&S1, &S2, Alt::Two, 0.95).unwrap();
    close(m.stat, 25.5, 1e-9, "W");
    close(m.p, 0.06933, 1e-5, "p");
    assert!(!m.exact && m.ties);
    // wilcox.test(x, y, alternative = "g") on the R help-page data: W = 35, p-value = 0.1272 (exact)
    let x = [0.80, 0.83, 1.89, 1.04, 1.45, 1.38, 1.91, 1.64, 0.73, 1.46];
    let y = [1.15, 0.88, 0.90, 0.74, 1.21];
    let m = mann_whitney(&x, &y, Alt::Greater, 0.95).unwrap();
    close(m.stat, 35.0, 1e-9, "W exact");
    close(m.p, 0.1272, 1e-4, "p exact");
    assert!(m.exact);
    let two = mann_whitney(&x, &y, Alt::Two, 0.95).unwrap();
    close(two.p, 0.2544, 2e-4, "p exact two-sided");
    assert!(two.effect_ci.is_some());
    // wilcox.test(x, y, paired = TRUE, alternative = "greater"): V = 40, p-value = 0.01953
    let a = [1.83, 0.50, 1.62, 2.48, 1.68, 1.88, 1.55, 3.06, 1.30];
    let b = [0.878, 0.647, 0.598, 2.05, 1.06, 1.29, 1.06, 3.14, 1.29];
    let d: Vec<f64> = a.iter().zip(&b).map(|(p, q)| p - q).collect();
    let (s, zeros) = signed_rank(&d, Alt::Greater, 0.95).unwrap();
    assert_eq!(zeros, 0);
    close(s.stat, 40.0, 1e-9, "V");
    close(s.p, 0.01953, 1e-5, "p");
    assert!(s.exact);
    close(signed_rank(&d, Alt::Two, 0.95).unwrap().0.p, 0.03906, 1e-5, "two-sided");
}

#[test]
fn shapiro_wilk_matches_r_on_mtcars_mpg() {
    // shapiro.test(mtcars$mpg): W = 0.94756, p-value = 0.1229
    let (w, p) = shapiro_wilk(&MPG).unwrap();
    close(w, 0.94756, 1e-5, "W");
    close(p, 0.1229, 5e-4, "p");
    // n = 3 closed form: x = 1, 2, 4 gives W = 0.9642857, p = 0.6369 (R)
    let (w3, p3) = shapiro_wilk(&[1.0, 2.0, 4.0]).unwrap();
    close(w3, 0.9642857, 1e-6, "W n=3");
    close(p3, 0.6369, 1e-3, "p n=3");
    assert!(shapiro_wilk(&[1.0, 1.0, 1.0]).is_err());
}

#[test]
fn correlations_match_r_on_mtcars() {
    // cor.test(mpg, wt)
    let p = pearson(&MPG, &WT, 0.95).unwrap();
    close(p.r, -0.8676594, 1e-7, "r");
    close(p.p, 1.294e-10, 1e-12, "p");
    let ci = p.ci.unwrap();
    close(ci.0, -0.9338264, 1e-6, "ci lo");
    close(ci.1, -0.7440872, 1e-6, "ci hi");
    // cor.test(mpg, wt, method = "spearman"): rho = -0.886422 (R uses an exact/AS 89 p; the t approximation here is within 1e-9 of it)
    let s = spearman(&MPG, &WT, 0.95).unwrap();
    close(s.r, -0.886422, 1e-6, "rho");
    assert!(s.p < 1e-9 && s.ties);
    // cor.test(mpg, wt, method = "kendall"): tau = -0.7278321, z = -5.7981, p-value = 6.706e-09
    let k = kendall(&MPG, &WT, 0.95).unwrap();
    close(k.r, -0.7278321, 1e-7, "tau");
    close(k.stat, -5.7981, 1e-3, "z");
    close(k.p, 6.706e-9, 2e-11, "p");
    // No ties, n < 50: exact. cor.test(1:5, c(2, 1, 4, 3, 5), method = "kendall"): T = 8, tau = 0.6, p-value = 0.2333
    let e = kendall(&[1.0, 2.0, 3.0, 4.0, 5.0], &[2.0, 1.0, 4.0, 3.0, 5.0], 0.95).unwrap();
    assert!(e.exact);
    close(e.r, 0.6, 1e-12, "tau exact");
    close(e.p, 0.2333, 1e-4, "p exact");
}

#[test]
fn chi_square_and_proportions_match_r() {
    // chisq.test(rbind(c(762, 327, 468), c(484, 239, 477))): X-squared = 30.07, df = 2, p-value = 2.954e-07
    let c = chisq_independence(&[vec![762.0, 327.0, 468.0], vec![484.0, 239.0, 477.0]], false, 0.95).unwrap();
    close(c.chi2, 30.07, 5e-3, "X2");
    close(c.df, 2.0, 0.0, "df");
    close(c.p, 2.954e-7, 5e-10, "p");
    assert!(c.effect > 0.0 && c.effect_ci.0 <= c.effect && c.effect <= c.effect_ci.1);
    // chisq.test(c(89, 37, 30, 28, 2), p = c(40, 20, 20, 15, 5), rescale.p = TRUE): X-squared = 9.9901, df = 4, p-value = 0.04059
    let g = chisq_gof(&[89.0, 37.0, 30.0, 28.0, 2.0], &[40.0, 20.0, 20.0, 15.0, 5.0], 0.95).unwrap();
    close(g.chi2, 9.9901, 1e-3, "gof X2");
    close(g.p, 0.04059, 1e-5, "gof p");
    // prop.test(smokers, patients): X-squared = 12.6, df = 3, p-value = 0.005585
    let pe = prop_equal(&[83.0, 90.0, 129.0, 70.0], &[86.0, 93.0, 136.0, 82.0]).unwrap();
    close(pe.chi2, 12.6, 5e-2, "prop X2");
    close(pe.p, 0.005585, 2e-5, "prop p");
    // binom.test(682, 925, p = 0.75) (R help page): p-value = 0.3825, 95% CI 0.7076683 0.7654066
    let b = binom_test(682, 925, 0.75, Alt::Two, 0.95).unwrap();
    close(b.p, 0.3825, 5e-4, "binom p");
    close(b.ci_exact.0, 0.7076683, 1e-6, "cp lo");
    close(b.ci_exact.1, 0.7654066, 1e-6, "cp hi");
    // Wilson interval for 1 of 10, 95 %: 0.0179, 0.4041 (Newcombe 1998, Table I)
    let w = wilson(1.0, 10.0, 0.95);
    close(w.0, 0.0179, 1e-3, "wilson lo");
    close(w.1, 0.4041, 1e-3, "wilson hi");
}
