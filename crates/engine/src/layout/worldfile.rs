//! World file (`.pgw`) and `.prj` for a PNG export, so GIS software can place
//! the map frame's area. The six numbers are ESRI's world-file format
//! (A, D, B, E, C, F); the georeferencing holds at the frame's nominal scale
//! and only inside the map frame (the rest of the page is layout, not map).

use super::geo::{self, Body, FrameCrs};
use super::model::{Element, Layout};
use anyhow::{anyhow, bail, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct WorldFile {
    /// `[A, D, B, E, C, F]`: pixel width, rotation terms, negative pixel height, and the centre of the top-left pixel.
    pub params: [f64; 6],
    /// Id of the frame described.
    pub frame: String,
    pub crs: String,
}

/// World file for the first map frame of `layout` as exported at `dpi`.
pub fn compute(layout: &Layout, dpi: f64) -> Result<WorldFile> {
    let Some((at, crs, center, scale)) = layout.elements.iter().find_map(|e| match e {
        Element::MapFrame { at, crs, center, scale, .. } if at.visible => Some((at, crs, *center, *scale)),
        _ => None,
    }) else {
        bail!("A world file needs a map frame on the page");
    };
    if crs.trim().is_empty() {
        bail!("Set a coordinate system for the map frame before writing a world file");
    }
    if at.rotation != 0.0 {
        bail!("A world file cannot describe a rotated map frame; set the frame rotation to 0");
    }
    if !(dpi.is_finite() && dpi > 0.0) {
        bail!("The export resolution must be above 0 dpi");
    }
    let body = Body::get(&layout.body)?;
    let fc = FrameCrs::new(crs, &body)?;
    let extent = geo::frame_extent(&body, &fc, center, scale, at.w, at.h)?;
    let (ux, uy) = ((extent[2] - extent[0]) / at.w, (extent[3] - extent[1]) / at.h); // CRS units per mm
    let px = 25.4 / dpi; // mm per pixel
    let outer = layout.page.bleed.clamp(0.0, 10.0) + if layout.page.crop_marks { 10.0 } else { 0.0 };
    // Centre of the top-left pixel, in mm from the page's trim corner, then relative to the frame's corner.
    let (cx, cy) = (-outer + px / 2.0 - at.x, -outer + px / 2.0 - at.y);
    Ok(WorldFile { params: [ux * px, 0.0, 0.0, -uy * px, extent[0] + cx * ux, extent[3] - cy * uy], frame: at.id.clone(), crs: crs.clone() })
}

impl WorldFile {
    /// The six lines of a `.pgw` file.
    pub fn text(&self) -> String {
        self.params.iter().map(|v| format!("{v:.9}\n")).collect()
    }

    /// ESRI-style WKT for the `.prj` file.
    pub fn prj(&self) -> Result<String> {
        let srs = gdal::spatial_ref::SpatialRef::from_definition(&crate::crs::resolve(&self.crs)).map_err(|e| anyhow!("Unknown coordinate system '{}': {e}", self.crs))?;
        srs.to_wkt().map_err(|e| anyhow!("{e}"))
    }
}
