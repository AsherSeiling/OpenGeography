//! Vertical datums of elevation rasters (CRS accuracy plan C1/C2): which height reference a raster's values use (NAVD88, EGM2008, …),
//! kept in the file's metadata, carried through tools that keep heights, and compared when rasters are combined.
//! Converting between height references needs geoid grids (see the Datum Grids menu) and is not done here: mixing is reported.

use anyhow::{anyhow, bail, Result};
use gdal::{Dataset, Metadata};
use std::path::Path;

const KEY: &str = "GIS_STUDIO_VERTICAL_DATUM";

/// The choices the Define Vertical Datum node offers: (value stored, label).
pub const DATUMS: [(&str, &str); 6] = [
    ("NAVD88", "NAVD88 (North American Vertical Datum 1988)"),
    ("NGVD29", "NGVD29 (National Geodetic Vertical Datum 1929)"),
    ("EGM96", "EGM96 geoid (mean sea level)"),
    ("EGM2008", "EGM2008 geoid (mean sea level)"),
    ("Ellipsoidal WGS 84", "Ellipsoidal heights (WGS 84, as from GPS)"),
    ("Other", "Another or local vertical datum"),
];

/// The vertical datum recorded in a raster file, if any.
pub fn get(path: &Path) -> Option<String> {
    Dataset::open(path).ok()?.metadata_item(KEY, "").filter(|s| !s.trim().is_empty())
}

/// Record the vertical datum in the raster file's metadata.
pub fn set(path: &Path, datum: &str) -> Result<()> {
    if datum.trim().is_empty() {
        bail!("Choose a vertical datum");
    }
    let mut ds = Dataset::open_ex(path, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })
        .map_err(|e| anyhow!("Could not open the raster to record its vertical datum: {e}"))?;
    ds.set_metadata_item(KEY, datum.trim(), "")?;
    Ok(())
}

/// A copy of `src` that records `datum` as the vertical datum of its values (the source file is never changed).
pub fn define(src: &Path, datum: &str) -> Result<std::path::PathBuf> {
    if datum.trim().is_empty() {
        bail!("Choose a vertical datum");
    }
    let ds = Dataset::open(src).map_err(|e| anyhow!("Could not open the raster: {e}"))?;
    let out = crate::raster::new_store_path("vdatum", "tif");
    let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-co", "BIGTIFF=IF_SAFER"].map(String::from).to_vec();
    crate::raster::translate(&ds, &out, &args)?;
    set(&out, datum)?;
    crate::raster::build_overviews(&out)?;
    Ok(out)
}

/// Give `out` the vertical datum of its inputs when they agree (or only one says), so tools that keep heights keep the label.
pub fn inherit(out: &Path, inputs: &[&Path]) {
    let mut found: Vec<String> = inputs.iter().filter_map(|p| get(p)).collect();
    found.sort();
    found.dedup();
    if found.len() == 1 {
        let _ = set(out, &found[0]);
    }
}

/// Compare the vertical datums of rasters about to be combined. `Ok(None)` when nothing is known or all agree; a sentence otherwise,
/// prefixed "Warning," — or an error in Strict mode.
pub fn check(what: &str, inputs: &[&Path]) -> Result<Option<String>> {
    let datums: Vec<Option<String>> = inputs.iter().map(|p| get(p)).collect();
    let known: Vec<&String> = datums.iter().flatten().collect();
    if known.is_empty() {
        return Ok(None);
    }
    let mut distinct: Vec<&String> = known.clone();
    distinct.sort();
    distinct.dedup();
    let unknown = datums.iter().any(|d| d.is_none());
    if distinct.len() == 1 && !unknown {
        return Ok(None);
    }
    let detail = if distinct.len() > 1 {
        format!("the rasters use different vertical datums ({}); heights differ by up to metres between them", distinct.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" and "))
    } else {
        format!("one raster is on {} and the vertical datum of another is not recorded", distinct[0])
    };
    if crate::transform_report::is_strict() {
        bail!("Stopped: {what} would combine heights that may not match: {detail}. Strict mode is on; use Define Vertical Datum to record the missing one, or switch to Warn to continue.");
    }
    Ok(Some(format!("Warning, {what}: {detail}. Use Define Vertical Datum to record each raster's datum; converting between them needs a geoid grid (Datum Grids).")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(path: &Path) {
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(path, 4, 4, 1).unwrap();
        ds.set_geo_transform(&[0.0, 1.0, 0.0, 4.0, 0.0, -1.0]).unwrap();
        ds.rasterband(1).unwrap().write((0, 0), (4, 4), &mut gdal::raster::Buffer::new((4, 4), vec![1.0f32; 16])).unwrap();
    }

    #[test]
    fn datums_are_recorded_inherited_and_compared() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, c, out) = (dir.path().join("a.tif"), dir.path().join("b.tif"), dir.path().join("c.tif"), dir.path().join("o.tif"));
        for p in [&a, &b, &c, &out] {
            raster(p);
        }
        assert_eq!(get(&a), None);
        set(&a, "NAVD88").unwrap();
        set(&b, "EGM2008").unwrap();
        assert_eq!(get(&a).as_deref(), Some("NAVD88"));
        // Nothing known: silent. Same datum: silent. Different datums or one unknown: a warning.
        assert_eq!(check("x", &[&c, &out]).unwrap(), None);
        set(&c, "NAVD88").unwrap();
        assert_eq!(check("x", &[&a, &c]).unwrap(), None);
        let w = check("Raster Calculator", &[&a, &b]).unwrap().unwrap();
        assert!(w.starts_with("Warning, Raster Calculator: the rasters use different vertical datums (EGM2008 and NAVD88)"), "{w}");
        let u = check("Mosaic", &[&a, &out]).unwrap().unwrap();
        assert!(u.contains("one raster is on NAVD88 and the vertical datum of another is not recorded"), "{u}");
        // Inheritance: one datum among the inputs is passed on; two different ones are not.
        inherit(&out, &[&a, &c]);
        assert_eq!(get(&out).as_deref(), Some("NAVD88"));
        let out2 = dir.path().join("o2.tif");
        raster(&out2);
        inherit(&out2, &[&a, &b]);
        assert_eq!(get(&out2), None);
        assert!(set(&a, "  ").is_err());
    }
}
