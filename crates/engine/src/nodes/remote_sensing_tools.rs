//! Remote sensing as model nodes (phase H5): spectral indices and unsupervised classification. Maths in `remote_sensing.rs`.
//! The equivalent ArcGIS tools are Band Arithmetic (NDVI etc.) and Iso Cluster Unsupervised Classification.

use super::*;
use crate::remote_sensing as rs;
use std::path::Path;

fn rout(path: std::path::PathBuf, i: &Rel) -> Result<Output> {
    Ok(Output::Raster { path: path.display().to_string(), crs: i.crs.clone() })
}

fn band_param(p: &Params, k: &str, label: &str) -> Result<usize> {
    let v = p.f64(k).unwrap_or(f64::NAN);
    if !(v.is_finite() && v >= 1.0 && v.fract() == 0.0) {
        bail!("{label} must be a band number, 1 or more");
    }
    Ok(v as usize)
}

pub struct SpectralIndex;
impl Tool for SpectralIndex {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "spectral_index", name: "Spectral Index (NDVI, NDWI, NBR …)", category: "Raster", role: Role::Tool,
            description: "A band-ratio index from two bands of an image. Band A and B mean: NDVI: A = near infrared, B = red; NDWI: A = green, B = near infrared; MNDWI: A = green, B = shortwave infrared 1; NBR: A = near infrared, B = shortwave infrared 2; NDBI: A = shortwave infrared 1, B = near infrared; SAVI: A = near infrared, B = red. Band numbers depend on the sensor (Sentinel-2: red 3, green 2, near infrared 7 or 8 in a stack of B2, B3, B4, B5, B6, B7, B8 …; check your file). Results run from -1 to 1; cells with no data in either band, or a zero denominator, are no data. 'Reflectance scale' only matters for SAVI: 1 if the bands are 0–1, 0.0001 for Sentinel-2 L2A numbers.",
            inputs: vec![Port { id: "in", label: "Multiband image", kind: PortKind::Raster }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("index", "Index", ParamKind::Enum).default("ndvi").options(&[("ndvi", rs::INDICES[0].1), ("ndwi", rs::INDICES[1].1), ("mndwi", rs::INDICES[2].1), ("nbr", rs::INDICES[3].1), ("ndbi", rs::INDICES[4].1), ("savi", rs::INDICES[5].1)]),
                Param::new("band_a", "Band A", ParamKind::Number).default(2),
                Param::new("band_b", "Band B", ParamKind::Number).default(1),
                Param::new("scale", "Reflectance scale (SAVI)", ParamKind::Number).default(1),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let out = rs::spectral_index(Path::new(need_raster(i)?), p.str("index").as_deref().unwrap_or("ndvi"), band_param(p, "band_a", "Band A")?, band_param(p, "band_b", "Band B")?, p.f64("scale").unwrap_or(1.0))?;
        rout(out, i)
    }
}

pub struct UnsupervisedClassification;
impl Tool for UnsupervisedClassification {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "classify_kmeans", name: "Unsupervised Classification (K-means)", category: "Raster", role: Role::Tool,
            description: "Group the cells of an image into classes of similar colour, without training areas. Bands are put on the same scale first, so a band with big numbers does not dominate. Classes are numbered 1, 2, 3 … from the lowest to the highest value of the first band; 0 is no data. The class centres are stored in the raster's CLASS_CENTRES metadata. Look at the result and name the classes yourself (water, forest …). Repeating the run gives the same map.",
            inputs: vec![Port { id: "in", label: "Multiband image", kind: PortKind::Raster }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("classes", "Number of classes", ParamKind::Number).default(5),
                Param::new("bands", "Bands to use (empty: all)", ParamKind::Text).optional().help("1,2,3"),
                Param::new("sample", "Cells used to find the classes", ParamKind::Number).default(50000),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let k = p.f64("classes").unwrap_or(f64::NAN);
        if !(k.is_finite() && k.fract() == 0.0) {
            bail!("The number of classes must be a whole number between 2 and {}", rs::MAX_CLASSES);
        }
        let mut bands = vec![];
        for t in p.str("bands").unwrap_or_default().split([',', ' ']).filter(|t| !t.trim().is_empty()) {
            bands.push(t.trim().parse::<usize>().map_err(|_| anyhow!("'{}' is not a band number. Type numbers like 1,2,3", t.trim()))?);
        }
        let sample = p.f64("sample").unwrap_or(50000.0);
        if !(sample.is_finite() && sample >= 0.0) {
            bail!("The sample must be a number of cells, at least 1000");
        }
        rout(rs::classify(Path::new(need_raster(i)?), k as usize, &bands, sample as usize)?, i)
    }
}
