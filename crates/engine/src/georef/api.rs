//! Engine entry points for raster georeferencing (GR2): errors, image viewer, preview and the saved warp.

use super::short_crs;
use super::warp::{self, Ctx, Dest, ErrorReport, ImageInfo, Method, Preview, WarpOutput};
use super::ControlSet;
use crate::{layers, progress, sources, Engine, LayerInfo};
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// What the UI sends to georeference an image.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WarpRequest {
    pub set: ControlSet,
    pub method: Method,
    /// nearest (default), bilinear, cubic, cubicspline or lanczos.
    #[serde(default)]
    pub resampling: Option<String>,
    /// The image file. A project raster layer's file is passed with `source_layer` so its citation is kept.
    pub image: String,
    #[serde(default)]
    pub source_layer: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

fn citation_for(req: &WarpRequest, out: &WarpOutput, name: &str, layer_id: &str) -> sources::Citation {
    let pts: Vec<String> = req
        .set
        .points
        .iter()
        .map(|p| format!("#{} ({}, {}) -> ({}, {}){}", p.id, p.pixel, p.line, p.x, p.y, if p.enabled { "" } else { " [check point]" }))
        .collect();
    let r = &out.report;
    sources::Citation {
        key: format!("georef:{layer_id}"),
        title: format!("{name}: image georeferenced with control points"),
        kind: "Derived data".into(),
        annotation: format!(
            "Georeferenced from {} with {} control points in {} using a {} (GDAL GCP transformer), resampling {}. RMS {:.6} {} ({:.3} m, {:.3} px) over {} enabled points{}. Points (pixel, line -> x, y): {}.",
            Path::new(&req.image).file_name().and_then(|s| s.to_str()).unwrap_or(&req.image),
            req.set.points.len(),
            short_crs(&req.set.target_crs),
            r.method_label,
            req.resampling.as_deref().filter(|s| !s.is_empty()).unwrap_or("nearest"),
            r.rms,
            r.unit,
            r.rms_m,
            r.rms_px,
            r.enabled_count,
            r.check_rms.map(|c| format!("; check points RMS {c:.6} {}", r.unit)).unwrap_or_default(),
            pts.join("; ")
        ),
        complete: true,
        ..Default::default()
    }
}

impl Engine {
    /// Per-point errors and RMS of a set under a method (no image is read).
    pub fn georef_errors(&self, set: &ControlSet, method: Method) -> Result<ErrorReport> {
        warp::errors(set, method)
    }

    pub fn georef_image_info(&self, path: &Path) -> Result<ImageInfo> {
        warp::image_info(path)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn georef_image_region(&self, path: &Path, x0: f64, y0: f64, w: f64, h: f64, out_w: usize, out_h: usize) -> Result<String> {
        warp::image_region(path, x0, y0, w, h, out_w, out_h)
    }

    pub fn georef_preview(&self, image: &Path, set: &ControlSet, method: Method, resampling: &str, max_side: usize) -> Result<Preview> {
        warp::preview(image, set, method, resampling, max_side)
    }

    /// Warp the image into a new GeoTIFF in the raster store, in the target CRS, and add it as a layer. The image is only read.
    pub fn georef_warp(&self, req: &WarpRequest) -> Result<LayerInfo> {
        let image = Path::new(&req.image);
        let stem = image.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
        let name = req.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("{stem} (georeferenced)"));
        // A bad set fails here, before any task or file exists.
        warp::errors(&req.set, req.method)?;
        let task = progress::Task::start(format!("Georeferencing {stem}")).cancellable();
        let dest = crate::raster::new_store_path(&format!("{stem}_georef"), "tif");
        let ctx = Ctx { stop: Arc::new(AtomicBool::new(false)), task: Some(task.handle()) };
        let (ds, out) = warp::run(image, &req.set, req.method, req.resampling.as_deref().unwrap_or(""), Dest::GeoTiff(&dest), None, &ctx)?;
        drop(ds);
        task.detail("Building overviews");
        if let Err(e) = crate::raster::build_overviews(&dest) {
            let _ = std::fs::remove_file(&dest);
            return Err(e);
        }
        let id = self.alloc_id();
        let info = layers::raster_layer(&id, &name, &dest, Some(req.image.clone()))?;
        self.with_conn(|c| {
            self.save_meta(c, &info)?;
            match &req.source_layer {
                Some(l) => sources::inherit(c, &[l.as_str()], &info.id, &info.name)?,
                None => sources::record(c, &sources::for_file(image, "Image", 0, None), &info.id, &info.name)?,
            }
            sources::record(c, &citation_for(req, &out, &name, &info.id), &info.id, &info.name)
        })?;
        Ok(info)
    }
}
