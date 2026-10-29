//! Remote sensing (phase H5): spectral indices from a multiband raster and unsupervised (k-means) classification.
//! Index formulas are the published ones (Rouse et al. 1974 NDVI; McFeeters 1996 NDWI; Xu 2006 MNDWI; Key & Benson 2006 NBR;
//! Zha et al. 2003 NDBI; Huete 1988 SAVI). K-means is Lloyd's algorithm with k-means++ seeding (Arthur & Vassilvitskii 2007).
//! Written from the papers; the arithmetic runs through the raster calculator (`raster_ops::calc`).

use crate::raster_ops::{cancelled, check_cells, grid_of, select_bands};
use anyhow::{anyhow, bail, Result};
use gdal::raster::{Buffer, RasterCreationOptions};
use gdal::{Dataset, Metadata};
use std::path::{Path, PathBuf};

/// (id, name, what band A is, what band B is, expression of A and B; `s` is the reflectance scale factor).
pub const INDICES: [(&str, &str, &str, &str, &str); 6] = [
    ("ndvi", "NDVI (vegetation greenness)", "near infrared", "red", "(A - B) / (A + B)"),
    ("ndwi", "NDWI (open water)", "green", "near infrared", "(A - B) / (A + B)"),
    ("mndwi", "MNDWI (open water in built-up areas)", "green", "shortwave infrared 1", "(A - B) / (A + B)"),
    ("nbr", "NBR (burn severity)", "near infrared", "shortwave infrared 2", "(A - B) / (A + B)"),
    ("ndbi", "NDBI (built-up areas)", "shortwave infrared 1", "near infrared", "(A - B) / (A + B)"),
    ("savi", "SAVI (vegetation, bare-soil corrected, L = 0.5)", "near infrared", "red", "1.5 * (A * S - B * S) / (A * S + B * S + 0.5)"),
];

/// A normalised-difference style index of two bands of `src` (1-based band numbers). `scale` turns the stored numbers into
/// reflectance 0–1 (0.0001 for Sentinel-2 L2A); only SAVI depends on it. Cells with no data in either band, or a zero
/// denominator, are no data in the result.
pub fn spectral_index(src: &Path, index: &str, band_a: usize, band_b: usize, scale: f64) -> Result<PathBuf> {
    let (_, _, _, _, expr) = *INDICES.iter().find(|i| i.0 == index).ok_or_else(|| anyhow!("Unknown index \"{index}\""))?;
    if !(scale.is_finite() && scale > 0.0) {
        bail!("The reflectance scale must be a number greater than 0 (1 if the bands are already 0–1, 0.0001 for Sentinel-2)");
    }
    if band_a == band_b {
        bail!("The two bands must be different (both are band {band_a})");
    }
    let a = select_bands(src, &band_a.to_string())?;
    let b = select_bands(src, &band_b.to_string()).inspect_err(|_| {
        let _ = std::fs::remove_file(&a);
    })?;
    let expr = expr.replace('S', &format!("{scale}"));
    let out = crate::raster_ops::calc(&[a.as_path(), b.as_path()], &expr);
    let _ = std::fs::remove_file(&a);
    let _ = std::fs::remove_file(&b);
    out
}

// ---------------------------------------------------------------- k-means

pub const MAX_CLASSES: usize = 20;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 11
    }
    fn unit(&mut self) -> f64 {
        self.next() as f64 / (1u64 << 53) as f64
    }
}

fn dist2(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

fn nearest(c: &[Vec<f64>], p: &[f64]) -> (usize, f64) {
    c.iter().enumerate().map(|(i, c)| (i, dist2(c, p))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap()
}

/// Lloyd's algorithm on `pts` (already standardised). Deterministic for a given input.
pub fn kmeans(pts: &[Vec<f64>], k: usize, iterations: usize) -> Vec<Vec<f64>> {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    // k-means++ seeding: each new centre is drawn with probability proportional to its squared distance from the nearest centre so far.
    let mut c: Vec<Vec<f64>> = vec![pts[(rng.next() as usize) % pts.len()].clone()];
    while c.len() < k {
        let d: Vec<f64> = pts.iter().map(|p| nearest(&c, p).1).collect();
        let total: f64 = d.iter().sum();
        if total <= 0.0 {
            c.push(pts[(rng.next() as usize) % pts.len()].clone());
            continue;
        }
        let mut t = rng.unit() * total;
        let mut pick = pts.len() - 1;
        for (i, v) in d.iter().enumerate() {
            t -= v;
            if t <= 0.0 {
                pick = i;
                break;
            }
        }
        c.push(pts[pick].clone());
    }
    let dim = pts[0].len();
    for _ in 0..iterations {
        let mut sum = vec![vec![0.0; dim]; k];
        let mut n = vec![0usize; k];
        for p in pts {
            let (i, _) = nearest(&c, p);
            n[i] += 1;
            for (s, v) in sum[i].iter_mut().zip(p) {
                *s += v;
            }
        }
        let mut shift = 0.0;
        for i in 0..k {
            let new = if n[i] > 0 {
                sum[i].iter().map(|s| s / n[i] as f64).collect()
            } else {
                // An empty class takes the sample point farthest from its centre.
                pts.iter().max_by(|a, b| nearest(&c, a).1.partial_cmp(&nearest(&c, b).1).unwrap()).unwrap().clone()
            };
            shift += dist2(&c[i], &new);
            c[i] = new;
        }
        if shift < 1e-10 {
            break;
        }
    }
    c
}

const BLOCK_ROWS: usize = 128;

/// Unsupervised classification: cluster the cells of `bands` (1-based; empty = all) into `k` classes. Values are standardised per
/// band first so a band with large numbers does not dominate. Classes are numbered 1..k by their first band's value, low to high;
/// 0 is no data. `sample` cells (evenly drawn) fit the clusters; every cell is then assigned.
pub fn classify(src: &Path, k: usize, bands: &[usize], sample: usize) -> Result<PathBuf> {
    if !(2..=MAX_CLASSES).contains(&k) {
        bail!("The number of classes must be between 2 and {MAX_CLASSES}");
    }
    if sample < 1000 {
        bail!("The sample must be at least 1000 cells");
    }
    let g = grid_of(src)?;
    check_cells(g.cols as f64, g.rows as f64)?;
    let list: Vec<usize> = if bands.is_empty() { (1..=g.bands).collect() } else { bands.to_vec() };
    for &b in &list {
        if b == 0 || b > g.bands {
            bail!("The raster has {} band{}; band {b} does not exist", g.bands, if g.bands == 1 { "" } else { "s" });
        }
    }
    let ds = Dataset::open(src)?;
    let nds: Vec<Option<f64>> = list.iter().map(|&b| ds.rasterband(b).ok().and_then(|r| r.no_data_value())).collect();
    let dim = list.len();
    let valid = |vals: &[f32]| vals.iter().zip(&nds).all(|(v, nd)| v.is_finite() && nd.is_none_or(|n| *v as f64 != n));
    let read_block = |row: usize, h: usize| -> Result<Vec<Vec<f32>>> {
        list.iter().map(|&b| Ok(ds.rasterband(b)?.read_as::<f32>((0, row as isize), (g.cols, h), (g.cols, h), None)?.data().to_vec())).collect()
    };

    // Pass 1: statistics of the valid cells, and an even sample of them.
    let (mut n, mut mean, mut m2) = (0u64, vec![0.0f64; dim], vec![0.0f64; dim]);
    let mut row = 0;
    while row < g.rows {
        cancelled()?;
        let h = BLOCK_ROWS.min(g.rows - row);
        let blk = read_block(row, h)?;
        for i in 0..g.cols * h {
            let v: Vec<f32> = blk.iter().map(|b| b[i]).collect();
            if valid(&v) {
                n += 1;
                for d in 0..dim {
                    let x = v[d] as f64;
                    let delta = x - mean[d];
                    mean[d] += delta / n as f64;
                    m2[d] += delta * (x - mean[d]);
                }
            }
        }
        row += h;
    }
    if n < (k as u64) * 10 {
        bail!("Only {n} cells have data in every chosen band; at least {} are needed for {k} classes", k * 10);
    }
    let sd: Vec<f64> = m2.iter().map(|m| { let s = (m / n as f64).sqrt(); if s > 0.0 { s } else { 1.0 } }).collect();
    let step = ((n as usize) / sample).max(1);
    let mut pts: Vec<Vec<f64>> = vec![];
    let (mut seen, mut row) = (0usize, 0);
    while row < g.rows {
        cancelled()?;
        let h = BLOCK_ROWS.min(g.rows - row);
        let blk = read_block(row, h)?;
        for i in 0..g.cols * h {
            let v: Vec<f32> = blk.iter().map(|b| b[i]).collect();
            if valid(&v) {
                if seen % step == 0 {
                    pts.push((0..dim).map(|d| (v[d] as f64 - mean[d]) / sd[d]).collect());
                }
                seen += 1;
            }
        }
        row += h;
    }
    cancelled()?;
    let mut centres = kmeans(&pts, k, 50);
    // Number the classes by their first band, low to high, so the labels do not change between runs.
    centres.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());

    // Pass 2: assign every cell.
    let out = crate::raster::new_store_path("classes", "tif");
    let opts = RasterCreationOptions::from_iter(["TILED=YES", "COMPRESS=DEFLATE", "BIGTIFF=IF_SAFER"]);
    let mut dst = gdal::DriverManager::get_driver_by_name("GTiff")?
        .create_with_band_type_with_options::<u8, _>(&out, g.cols, g.rows, 1, &opts)
        .map_err(|e| anyhow!("Could not create the output raster ({e}). Is there enough free disk space?"))?;
    dst.set_geo_transform(&g.gt)?;
    if !g.wkt.is_empty() {
        dst.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_wkt(&g.wkt)?)?;
    }
    dst.rasterband(1)?.set_no_data_value(Some(0.0))?;
    let desc: Vec<String> = centres.iter().enumerate().map(|(i, c)| format!("class {}: {}", i + 1, c.iter().enumerate().map(|(d, z)| format!("band {} = {:.4}", list[d], z * sd[d] + mean[d])).collect::<Vec<_>>().join(", "))).collect();
    dst.set_metadata_item("CLASS_CENTRES", &desc.join("; "), "")?;
    let run = (|| -> Result<()> {
        let mut row = 0;
        while row < g.rows {
            cancelled()?;
            let h = BLOCK_ROWS.min(g.rows - row);
            let blk = read_block(row, h)?;
            let mut res = vec![0u8; g.cols * h];
            for (i, r) in res.iter_mut().enumerate() {
                let v: Vec<f32> = blk.iter().map(|b| b[i]).collect();
                if valid(&v) {
                    let z: Vec<f64> = (0..dim).map(|d| (v[d] as f64 - mean[d]) / sd[d]).collect();
                    *r = nearest(&centres, &z).0 as u8 + 1;
                }
            }
            dst.rasterband(1)?.write((0, row as isize), (g.cols, h), &mut Buffer::new((g.cols, h), res))?;
            row += h;
        }
        Ok(())
    })();
    drop(dst);
    if let Err(e) = run {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    crate::raster::build_overviews(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(path: &Path, bands: &[Vec<f32>], size: usize, nodata: Option<f64>) {
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(path, size, size, bands.len()).unwrap();
        ds.set_geo_transform(&[600_000.0, 10.0, 0.0, 3_900_000.0, 0.0, -10.0]).unwrap();
        ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614).unwrap()).unwrap();
        for (i, b) in bands.iter().enumerate() {
            let mut band = ds.rasterband(i + 1).unwrap();
            if let Some(n) = nodata {
                band.set_no_data_value(Some(n)).unwrap();
            }
            band.write((0, 0), (size, size), &mut Buffer::new((size, size), b.clone())).unwrap();
        }
    }

    fn px(p: &Path, band: usize, c: isize, r: isize) -> f32 {
        Dataset::open(p).unwrap().rasterband(band).unwrap().read_as::<f32>((c, r), (1, 1), (1, 1), None).unwrap().data()[0]
    }

    #[test]
    fn ndvi_and_friends_match_hand_computed_values() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.tif");
        // 2 × 2 cells; band 1 = red, band 2 = nir, band 3 = swir.
        make(&p, &[vec![0.1, 0.2, 0.0, -9999.0], vec![0.5, 0.2, 0.0, 0.4], vec![0.3, 0.1, 0.2, 0.2]], 2, Some(-9999.0));
        let ndvi = spectral_index(&p, "ndvi", 2, 1, 1.0).unwrap();
        assert!((px(&ndvi, 1, 0, 0) - (0.5 - 0.1) / (0.5 + 0.1)).abs() < 1e-6); // 0.6667
        assert_eq!(px(&ndvi, 1, 1, 0), 0.0); // equal bands
        assert!(px(&ndvi, 1, 0, 1) < -1e30, "0 / 0 is no data");
        assert!(px(&ndvi, 1, 1, 1) < -1e30, "no data in an input is no data in the result");
        let nbr = spectral_index(&p, "nbr", 2, 3, 1.0).unwrap();
        assert!((px(&nbr, 1, 0, 0) - (0.5 - 0.3) / (0.5 + 0.3)).abs() < 1e-6);
        // SAVI with reflectance stored as 10000 × (scale 0.0001): same answer as the 0–1 data.
        let big: Vec<Vec<f32>> = [vec![0.1f32, 0.2, 0.0, -9999.0], vec![0.5, 0.2, 0.0, 0.4]].iter().map(|b| b.iter().map(|v| if *v == -9999.0 { *v } else { v * 10000.0 }).collect()).collect();
        let q = dir.path().join("big.tif");
        make(&q, &big, 2, Some(-9999.0));
        let savi = spectral_index(&q, "savi", 2, 1, 0.0001).unwrap();
        let expect = 1.5 * (0.5 - 0.1) / (0.5 + 0.1 + 0.5);
        assert!((px(&savi, 1, 0, 0) - expect as f32).abs() < 1e-5, "{} vs {expect}", px(&savi, 1, 0, 0));
        let g = grid_of(&ndvi).unwrap();
        assert!(!g.wkt.is_empty() && g.cols == 2);
    }

    #[test]
    fn index_refusals_are_readable() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.tif");
        make(&p, &[vec![0.1; 4], vec![0.5; 4]], 2, None);
        assert!(spectral_index(&p, "ndvi", 1, 1, 1.0).unwrap_err().to_string().contains("different"));
        assert!(spectral_index(&p, "ndvi", 1, 5, 1.0).unwrap_err().to_string().contains("band 5 does not exist"));
        assert!(spectral_index(&p, "ndvi", 1, 2, 0.0).unwrap_err().to_string().contains("scale"));
        assert!(spectral_index(&p, "evi2", 1, 2, 1.0).unwrap_err().to_string().contains("Unknown index"));
    }

    /// 40 × 40 cells, two bands: the left half is "water-like" (low, low), the right half "land-like" (high, high), with small noise.
    fn two_clusters(path: &Path) {
        let n = 40;
        let mut rng = Rng(7);
        let mut b1 = vec![];
        let mut b2 = vec![];
        for i in 0..n * n {
            let land = (i % n) >= n / 2;
            let (c1, c2) = if land { (800.0, 3000.0) } else { (100.0, 150.0) };
            b1.push((c1 + rng.unit() * 20.0) as f32);
            b2.push((c2 + rng.unit() * 20.0) as f32);
        }
        make(path, &[b1, b2], n, Some(-9999.0));
    }

    #[test]
    fn kmeans_separates_two_clean_clusters_and_numbers_them_low_to_high() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.tif");
        two_clusters(&p);
        let out = classify(&p, 2, &[], 5000).unwrap();
        for r in [0, 20, 39] {
            assert_eq!(px(&out, 1, 3, r), 1.0, "water side, row {r}");
            assert_eq!(px(&out, 1, 36, r), 2.0, "land side, row {r}");
        }
        let g = grid_of(&out).unwrap();
        assert!(!g.wkt.is_empty());
        let ds = Dataset::open(&out).unwrap();
        assert!(ds.metadata_item("CLASS_CENTRES", "").unwrap().contains("class 2: band 1 = 8"));
        // The same run twice gives the same map.
        let again = classify(&p, 2, &[], 5000).unwrap();
        assert_eq!(px(&again, 1, 3, 3), 1.0);
    }

    #[test]
    fn classify_keeps_no_data_and_refuses_bad_input() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.tif");
        let n = 20;
        let mut b: Vec<f32> = (0..n * n).map(|i| if i % n < 10 { 1.0 + (i % 3) as f32 * 0.01 } else { 50.0 + (i % 3) as f32 * 0.01 }).collect();
        b[0] = -9999.0;
        make(&p, &[b], n, Some(-9999.0));
        let out = classify(&p, 2, &[], 1000).unwrap();
        assert_eq!(px(&out, 1, 0, 0), 0.0, "no data stays 0");
        assert_eq!((px(&out, 1, 5, 5), px(&out, 1, 15, 5)), (1.0, 2.0));
        assert!(classify(&p, 1, &[], 1000).unwrap_err().to_string().contains("between 2 and 20"));
        assert!(classify(&p, 2, &[3], 1000).unwrap_err().to_string().contains("band 3 does not exist"));
        assert!(classify(&p, 2, &[], 10).unwrap_err().to_string().contains("at least 1000"));
        let tiny = dir.path().join("tiny.tif");
        make(&tiny, &[(0..36).map(|i| i as f32).collect()], 6, None);
        assert!(classify(&tiny, 5, &[], 1000).unwrap_err().to_string().contains("cells have data"));
    }
}
