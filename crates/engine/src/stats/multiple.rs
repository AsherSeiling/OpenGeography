//! Multiple-comparison adjustment of p-values: Holm (1979) and Benjamini-Hochberg (1995).

/// Holm step-down adjusted p-values (controls the family-wise error rate).
pub fn holm(p: &[f64]) -> Vec<f64> {
    let m = p.len();
    let mut idx: Vec<usize> = (0..m).collect();
    idx.sort_by(|&a, &b| p[a].partial_cmp(&p[b]).unwrap());
    let mut out = vec![0.0; m];
    let mut run = 0.0f64;
    for (rank, &i) in idx.iter().enumerate() {
        run = run.max(((m - rank) as f64 * p[i]).min(1.0));
        out[i] = run;
    }
    out
}

/// Benjamini-Hochberg adjusted p-values (controls the false discovery rate).
pub fn benjamini_hochberg(p: &[f64]) -> Vec<f64> {
    let m = p.len();
    let mut idx: Vec<usize> = (0..m).collect();
    idx.sort_by(|&a, &b| p[b].partial_cmp(&p[a]).unwrap());
    let mut out = vec![0.0; m];
    let mut run = 1.0f64;
    for (k, &i) in idx.iter().enumerate() {
        let rank = m - k;
        run = run.min((p[i] * m as f64 / rank as f64).min(1.0));
        out[i] = run;
    }
    out
}

/// Adjust by name: "holm", "bh", anything else leaves the p-values as they are.
pub fn adjust(p: &[f64], how: &str) -> Vec<f64> {
    match how {
        "holm" => holm(p),
        "bh" => benjamini_hochberg(p),
        _ => p.to_vec(),
    }
}

pub fn adjust_label(how: &str) -> Option<&'static str> {
    match how {
        "holm" => Some("Holm (1979) step-down"),
        "bh" => Some("Benjamini-Hochberg (1995) false discovery rate"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_r_p_adjust() {
        // p.adjust(c(0.01, 0.04, 0.03, 0.005), "holm") = 0.03 0.06 0.06 0.02; "BH" = 0.02 0.04 0.04 0.02
        let p = [0.01, 0.04, 0.03, 0.005];
        for (a, b) in holm(&p).iter().zip([0.03, 0.06, 0.06, 0.02]) {
            assert!((a - b).abs() < 1e-12, "{a} {b}");
        }
        for (a, b) in benjamini_hochberg(&p).iter().zip([0.02, 0.04, 0.04, 0.02]) {
            assert!((a - b).abs() < 1e-12, "{a} {b}");
        }
    }
}
