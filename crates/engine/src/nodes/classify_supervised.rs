//! Supervised classification of a multi-band raster (Wave 1, package T6) with `smartcore` (Apache-2.0, pure Rust, no BLAS): Random Forest
//! (Breiman 2001, "Random forests", Machine Learning 45:5-32) or k-nearest neighbours (Cover and Hart 1967). Training areas are points (the cell
//! they fall in) or polygons (every cell whose centre is inside) with a whole-number class field, transformed to the raster's CRS explicitly.
//!
//! Honest accuracy: the samples are split at random, class by class, with a seed that is an input and is written in the result; the model is
//! fitted on the training part and scored on the hold-out part (overall accuracy and the confusion matrix, rows = true class, columns =
//! predicted class). The map is then made by a model fitted on all samples, so the reported accuracy is that of the same method trained on a
//! smaller sample. Cells with no data in any band stay no data.

use super::*;
use crate::geomkit::{self, Rng};
use crate::raster;
use gdal::Metadata;
use smartcore::ensemble::random_forest_classifier::{RandomForestClassifier, RandomForestClassifierParameters};
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::neighbors::knn_classifier::{KNNClassifier, KNNClassifierParameters};

const MAX_SAMPLES: usize = 400_000;

struct Image {
    ds: gdal::Dataset,
    cols: usize,
    rows: usize,
    gt: [f64; 6],
    bands: usize,
    nodata: Vec<Option<f64>>,
}

impl Image {
    fn row(&self, r: usize) -> Result<Vec<Vec<f32>>> {
        (1..=self.bands).map(|b| Ok(self.ds.rasterband(b)?.read_as::<f32>((0, r as isize), (self.cols, 1), (self.cols, 1), None)?.data().to_vec())).collect()
    }
    /// The band values of a cell, or None when any band has no data there.
    fn values(&self, row: &[Vec<f32>], c: usize) -> Option<Vec<f64>> {
        let mut v = Vec::with_capacity(self.bands);
        for b in 0..self.bands {
            let x = row[b][c] as f64;
            if !x.is_finite() || self.nodata[b] == Some(x) {
                return None;
            }
            v.push(x);
        }
        Some(v)
    }
}

pub struct ClassifySupervised;
impl Tool for ClassifySupervised {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "classify_supervised", name: "Supervised Classification (Random Forest / k-NN)", category: "Raster › Remote Sensing", role: Role::Tool,
            description: "Classify a multi-band image (satellite bands, indices) from training areas: points or polygons with a whole-number class field. The samples are split at random, class by class, with a fixed seed shown in the result; the model is fitted on the training part and scored on the held-out part, and the overall accuracy and the confusion matrix (rows = true class, columns = predicted class) are reported in the result message and stored in the raster's metadata. The map is then made by a model fitted on all samples. Methods: Random Forest (Breiman 2001; number of trees) or k-nearest neighbours (bands are standardised first). Training polygons use every cell whose centre is inside; training layers are transformed to the image's coordinate system. Cells with no data in any band stay no data; samples on no data are counted. Each class needs at least 2 samples. Computed with the smartcore library (pure Rust).",
            inputs: vec![Port { id: "in", label: "Image (bands are the predictors)", kind: PortKind::Raster }, Port { id: "train", label: "Training areas (points or polygons)", kind: PortKind::Vector }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("class_field", "Class field (whole numbers)", ParamKind::Field).field_of("train"),
                Param::new("method", "Method", ParamKind::Enum).default("random_forest").options(&[("random_forest", "Random Forest"), ("knn", "k-nearest neighbours")]),
                Param::new("trees", "Random Forest: number of trees", ParamKind::Number).default(100),
                Param::new("k", "k-NN: number of neighbours", ParamKind::Number).default(5),
                Param::new("holdout", "Share of samples held out to measure accuracy (0.05 to 0.5)", ParamKind::Number).default(0.3),
                Param::new("seed", "Random seed", ParamKind::Number).default(12345),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (im, tr) = (&inputs["in"], &inputs["train"]);
        let crs = need_raster_crs(im)?;
        let path = std::path::Path::new(need_raster(im)?);
        let class_field = p.req_str("class_field", "Class field")?;
        let knn = p.str("method").as_deref() == Some("knn");
        let num = |k: &str, d: f64| p.f64(k).unwrap_or(d);
        let (trees, k, hold, seed) = (num("trees", 100.0), num("k", 5.0), num("holdout", 0.3), num("seed", 12345.0));
        if !(trees.is_finite() && (1.0..=2000.0).contains(&trees) && trees.fract() == 0.0) {
            bail!("'Number of trees' must be a whole number from 1 to 2000");
        }
        if !(k.is_finite() && k >= 1.0 && k.fract() == 0.0) {
            bail!("'Number of neighbours' must be a whole number, 1 or more");
        }
        if !(hold.is_finite() && (0.05..=0.5).contains(&hold)) {
            bail!("'Share held out' must be between 0.05 and 0.5");
        }
        if !(seed.is_finite() && seed >= 0.0 && seed.fract() == 0.0) {
            bail!("'Random seed' must be a whole number, 0 or more");
        }
        let grid = crate::raster_ops::grid_of(path)?;
        if grid.gt[2] != 0.0 || grid.gt[4] != 0.0 || grid.gt[5] >= 0.0 {
            bail!("Classification needs a north-up image. Use Warp to make it north-up first.");
        }
        crate::raster_ops::check_cells(grid.cols as f64, grid.rows as f64)?;
        let ds = gdal::Dataset::open(path)?;
        let nodata: Vec<Option<f64>> = (1..=grid.bands).map(|b| ds.rasterband(b).ok().and_then(|r| r.no_data_value())).collect();
        let img = Image { ds, cols: grid.cols, rows: grid.rows, gt: grid.gt, bands: grid.bands, nodata };
        if img.bands > 64 {
            bail!("The image has {} bands; the limit is 64. Use Select Bands first.", img.bands);
        }
        // Training samples.
        let g = ctx.geom_in(tr, "a", crs)?;
        let mut stmt = ctx.conn.prepare(&format!(
            "SELECT cls, ST_GeometryType(g)::VARCHAR, ST_AsText(g) FROM (SELECT TRY_CAST(a.{f}::VARCHAR AS BIGINT) AS cls, {g} AS g FROM {t} a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom)) WHERE g IS NOT NULL",
            f = ident(&class_field), t = ident(&tr.table)
        ))?;
        let rows: Vec<(Option<i64>, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&tr.table)), [], |r| r.get(0))?;
        let (mut samples, mut on_nodata, mut outside, mut no_class) = (vec![], 0usize, 0usize, 0usize);
        let (dx, dy) = (img.gt[1], img.gt[5]);
        for (cls, ty, wkt) in rows {
            let Some(cls) = cls else {
                no_class += 1;
                continue;
            };
            if ty.contains("POINT") {
                let nums: Vec<f64> = wkt.split(|c: char| c == '(' || c == ')' || c.is_whitespace()).filter_map(|t| t.parse().ok()).collect();
                if nums.len() < 2 {
                    continue;
                }
                let (c, r) = (((nums[0] - img.gt[0]) / dx).floor(), ((nums[1] - img.gt[3]) / dy).floor());
                if c < 0.0 || r < 0.0 || c >= img.cols as f64 || r >= img.rows as f64 {
                    outside += 1;
                    continue;
                }
                match img.values(&img.row(r as usize)?, c as usize) {
                    Some(v) => samples.push((cls, v)),
                    None => on_nodata += 1,
                }
            } else {
                let parts = geomkit::parse_wkt(&wkt)?;
                let rings: Vec<&Vec<(f64, f64)>> = parts.iter().flatten().collect();
                let (mut y0, mut y1) = (f64::MAX, f64::MIN);
                for ring in &rings {
                    for (_, y) in ring.iter() {
                        y0 = y0.min(*y);
                        y1 = y1.max(*y);
                    }
                }
                let (r0, r1) = ((((y1 - img.gt[3]) / dy).floor() as i64).max(0), (((y0 - img.gt[3]) / dy).ceil() as i64).min(img.rows as i64 - 1));
                let mut any = false;
                for r in r0..=r1 {
                    let y = img.gt[3] + (r as f64 + 0.5) * dy;
                    let spans = geomkit::spans(&rings, y);
                    if spans.is_empty() {
                        continue;
                    }
                    let row = img.row(r as usize)?;
                    for (xa, xb) in spans {
                        let c0 = (((xa - img.gt[0]) / dx) - 0.5).ceil().max(0.0) as i64;
                        let c1 = ((((xb - img.gt[0]) / dx) - 0.5).ceil() as i64 - 1).min(img.cols as i64 - 1);
                        for c in c0..=c1 {
                            any = true;
                            match img.values(&row, c as usize) {
                                Some(v) => samples.push((cls, v)),
                                None => on_nodata += 1,
                            }
                        }
                    }
                    if samples.len() > MAX_SAMPLES {
                        bail!("The training areas hold more than {MAX_SAMPLES} cells. Use smaller or fewer training polygons.");
                    }
                }
                if !any {
                    outside += 1;
                }
            }
        }
        if total as usize > 0 && no_class > 0 {
            ctx.add_note(format!("Classification: {no_class} training feature(s) have no whole-number class and were left out."));
        }
        if outside > 0 {
            ctx.add_note(format!("Classification: {outside} training feature(s) cover no cell of the image and were left out."));
        }
        if on_nodata > 0 {
            ctx.add_note(format!("Classification: {on_nodata} training cell(s) have no data in some band and were left out."));
        }
        let mut classes: Vec<i64> = samples.iter().map(|s| s.0).collect();
        classes.sort();
        classes.dedup();
        if classes.len() < 2 {
            bail!("Classification needs training samples of at least 2 classes; found {}.", classes.len());
        }
        for c in &classes {
            let n = samples.iter().filter(|s| s.0 == *c).count();
            if n < 2 {
                bail!("Class {c} has only {n} training sample; each class needs at least 2.");
            }
        }
        let idx = |c: i64| classes.iter().position(|x| *x == c).unwrap() as u32;
        // Seeded stratified split.
        let mut rng = Rng::new(seed as u64);
        let (mut train, mut test): (Vec<usize>, Vec<usize>) = (vec![], vec![]);
        for c in &classes {
            let mut ids: Vec<usize> = samples.iter().enumerate().filter(|(_, s)| s.0 == *c).map(|(i, _)| i).collect();
            for i in (1..ids.len()).rev() {
                ids.swap(i, (rng.f64() * (i + 1) as f64) as usize);
            }
            let nt = ((ids.len() as f64 * hold).round() as usize).clamp(1, ids.len() - 1);
            test.extend(&ids[..nt]);
            train.extend(&ids[nt..]);
        }
        train.sort();
        test.sort();
        // Standardise for k-NN with the mean and spread of the training part.
        let nb = img.bands;
        let (mut mean, mut sd) = (vec![0.0; nb], vec![1.0; nb]);
        if knn {
            for b in 0..nb {
                let m = train.iter().map(|i| samples[*i].1[b]).sum::<f64>() / train.len() as f64;
                let v = train.iter().map(|i| (samples[*i].1[b] - m).powi(2)).sum::<f64>() / train.len() as f64;
                mean[b] = m;
                sd[b] = if v > 1e-12 { v.sqrt() } else { 1.0 };
            }
        }
        let feat = |v: &[f64]| -> Vec<f64> { v.iter().enumerate().map(|(b, x)| (x - mean[b]) / sd[b]).collect() };
        let matrix = |ids: &[usize]| -> Result<DenseMatrix<f64>> { DenseMatrix::from_2d_vec(&ids.iter().map(|i| feat(&samples[*i].1)).collect::<Vec<_>>()).map_err(|e| anyhow!("{e}")) };
        let labels = |ids: &[usize]| -> Vec<u32> { ids.iter().map(|i| idx(samples[*i].0)).collect() };
        enum Model {
            Rf(RandomForestClassifier<f64, u32, DenseMatrix<f64>, Vec<u32>>),
            Knn(KNNClassifier<f64, u32, DenseMatrix<f64>, Vec<u32>, smartcore::metrics::distance::euclidian::Euclidian<f64>>),
        }
        let fit = |ids: &[usize]| -> Result<Model> {
            let (x, y) = (matrix(ids)?, labels(ids));
            if knn {
                if k as usize > ids.len() {
                    bail!("k = {k} is more than the {} training samples", ids.len());
                }
                Ok(Model::Knn(KNNClassifier::fit(&x, &y, KNNClassifierParameters::default().with_k(k as usize)).map_err(|e| anyhow!("{e}"))?))
            } else {
                Ok(Model::Rf(RandomForestClassifier::fit(&x, &y, RandomForestClassifierParameters::default().with_n_trees(trees as u16).with_seed(seed as u64)).map_err(|e| anyhow!("{e}"))?))
            }
        };
        let predict = |m: &Model, x: &DenseMatrix<f64>| -> Result<Vec<u32>> {
            match m {
                Model::Rf(f) => f.predict(x).map_err(|e| anyhow!("{e}")),
                Model::Knn(f) => f.predict(x).map_err(|e| anyhow!("{e}")),
            }
        };
        let task = crate::progress::Task::start("Classification").cancellable();
        task.detail("fitting on the training part");
        let held = fit(&train)?;
        let pred = predict(&held, &matrix(&test)?)?;
        let truth = labels(&test);
        let kc = classes.len();
        let mut cm = vec![vec![0usize; kc]; kc];
        for (t, q) in truth.iter().zip(&pred) {
            cm[*t as usize][*q as usize] += 1;
        }
        let correct: usize = (0..kc).map(|i| cm[i][i]).sum();
        let acc = correct as f64 / test.len() as f64;
        task.check()?;
        task.detail("fitting on all samples");
        let all: Vec<usize> = (0..samples.len()).collect();
        let model = fit(&all)?;
        // Predict the whole image, a block of rows at a time.
        let mut out = vec![-9999.0f32; img.cols * img.rows];
        let block = (200_000 / img.cols.max(1)).clamp(1, img.rows);
        let mut r = 0;
        while r < img.rows {
            task.check()?;
            task.set(r as f64 / img.rows as f64, Some("classifying the image".to_string()));
            let (mut feats, mut at) = (vec![], vec![]);
            for rr in r..(r + block).min(img.rows) {
                let row = img.row(rr)?;
                for c in 0..img.cols {
                    if let Some(v) = img.values(&row, c) {
                        feats.push(feat(&v));
                        at.push(rr * img.cols + c);
                    }
                }
            }
            if !feats.is_empty() {
                let cls = predict(&model, &DenseMatrix::from_2d_vec(&feats).map_err(|e| anyhow!("{e}"))?)?;
                for (i, c) in at.iter().zip(cls) {
                    out[*i] = classes[c as usize] as f32;
                }
            }
            r += block;
        }
        let nd = out.iter().filter(|v| **v == -9999.0).count();
        if nd > 0 {
            ctx.add_note(format!("Classification: {nd} of {} cells have no data in some band and stay no data.", out.len()));
        }
        let wkt = crate::crs::srs(&crate::crs::resolve(crs))?.to_wkt()?;
        let g2 = crate::raster_ops::Grid { cols: img.cols, rows: img.rows, gt: img.gt, wkt, bands: 1 };
        let path_out = raster::new_store_path("classified", "tif");
        let header: Vec<String> = classes.iter().map(|c| c.to_string()).collect();
        let matrix_text = cm.iter().enumerate().map(|(i, row)| format!("class {}: {}", classes[i], row.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" "))).collect::<Vec<_>>().join("; ");
        let summary = format!(
            "{} with {} training samples ({} held out, seed {}): overall hold-out accuracy {:.1} % ({correct} of {}). Confusion matrix (rows = true class, columns = predicted class, in the order {}): {matrix_text}.",
            if knn { format!("k-NN (k = {})", k as usize) } else { format!("Random Forest ({} trees)", trees as usize) },
            samples.len(), test.len(), seed as u64, acc * 100.0, test.len(), header.join(", ")
        );
        {
            let mut d = crate::raster_ops::create_like(&path_out, &g2, 1, Some(-9999.0))?;
            d.rasterband(1)?.write((0, 0), (img.cols, img.rows), &mut gdal::raster::Buffer::new((img.cols, img.rows), out))?;
            d.set_metadata_item("CLASSIFICATION", &summary, "")?;
            d.set_metadata_item("HOLDOUT_ACCURACY", &format!("{acc}"), "")?;
            d.set_metadata_item("SEED", &format!("{}", seed as u64), "")?;
        }
        raster::build_overviews(&path_out)?;
        ctx.add_note(format!("Classification: {summary}"));
        Ok(Output::Raster { path: path_out.display().to_string(), crs: im.crs.clone() })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "classify_supervised" => &["land cover", "random forest", "knn", "training samples", "image classification", "machine learning", "satellite", "classes"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(ClassifySupervised)]
}
