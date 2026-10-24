//! USGS base maps for print (docs/LAYOUTS_PLAN.md C2.4): GDAL's WMS/TMS driver
//! reads the National Map tile service, and the area of the frame is warped to
//! the frame CRS. OpenStreetMap is excluded: its tile policy forbids bulk
//! downloads, which a print export is.

use anyhow::{anyhow, bail, Result};
use gdal::Dataset;

/// Base maps a layout frame may use, besides "none".
pub const IDS: [&str; 3] = ["usgsTopo", "usgsImagery", "usgsRelief"];

/// Error text for a basemap id that can't be printed (None = fine).
pub fn check(id: &str) -> Result<()> {
    match id {
        "none" => Ok(()),
        i if IDS.contains(&i) => Ok(()),
        "osm" => bail!("OpenStreetMap can't be used in a print layout: its tile usage policy forbids bulk downloads. Choose a USGS base map."),
        other => bail!("Unknown basemap '{other}' (use none, usgsTopo, usgsImagery or usgsRelief)"),
    }
}

fn service(id: &str) -> Option<&'static str> {
    Some(match id {
        "usgsTopo" => "USGSTopo",
        "usgsImagery" => "USGSImageryOnly",
        "usgsRelief" => "USGSShadedReliefOnly",
        _ => return None,
    })
}

/// GDAL WMS/TMS definition of one National Map tile service (Web Mercator,
/// finest level 16 as in the map view).
fn tile_xml(svc: &str) -> String {
    let w = 20037508.342789244;
    format!(
        "<GDAL_WMS><Service name=\"TMS\"><ServerUrl>https://basemap.nationalmap.gov/arcgis/rest/services/{svc}/MapServer/tile/${{z}}/${{y}}/${{x}}</ServerUrl></Service>\
         <DataWindow><UpperLeftX>-{w}</UpperLeftX><UpperLeftY>{w}</UpperLeftY><LowerRightX>{w}</LowerRightX><LowerRightY>-{w}</LowerRightY><TileLevel>16</TileLevel><TileCountX>1</TileCountX><TileCountY>1</TileCountY><YOrigin>top</YOrigin></DataWindow>\
         <Projection>EPSG:3857</Projection><BlockSizeX>256</BlockSizeX><BlockSizeY>256</BlockSizeY><BandsCount>3</BandsCount>\
         <UserAgent>GIS Studio (open-source desktop GIS; print layout)</UserAgent><Timeout>30</Timeout></GDAL_WMS>"
    )
}

/// Open a USGS base map over the network (GDAL WMS driver).
pub fn open(id: &str) -> Result<Dataset> {
    let svc = service(id).ok_or_else(|| anyhow!("'{id}' is not a printable basemap"))?;
    Dataset::open(tile_xml(svc)).map_err(|e| anyhow!("Could not open the USGS base map service: {e}"))
}

/// `extent` (`[xmin, ymin, xmax, ymax]` in `crs`) of `ds` warped to `w_px` × `h_px`,
/// as an RGBA PNG; pixels outside the source are transparent.
pub fn render(ds: &Dataset, crs: &str, extent: [f64; 4], w_px: u32, h_px: u32) -> Result<Vec<u8>> {
    let (w, h) = (w_px as usize, h_px as usize);
    if w == 0 || h == 0 || w.saturating_mul(h) > 100_000_000 {
        bail!("Basemap image size {w_px}×{h_px} px is not supported");
    }
    let target = crate::crs::resolve(crs);
    let mut args: Vec<String> = ["-of", "MEM", "-t_srs", target.as_str(), "-r", "bilinear", "-dstalpha", "-b", "1", "-b", "2", "-b", "3"].map(String::from).to_vec();
    args.extend(["-te".into(), extent[0].to_string(), extent[1].to_string(), extent[2].to_string(), extent[3].to_string(), "-ts".into(), w.to_string(), h.to_string()]);
    let out = crate::raster::warp(ds, "", &args)?;
    let mut rgba = vec![0u8; w * h * 4];
    for band in 1..=4 {
        let v = out.rasterband(band)?.read_as::<u8>((0, 0), (w, h), (w, h), None)?;
        for (i, p) in v.data().iter().enumerate() {
            rgba[i * 4 + band - 1] = *p;
        }
    }
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, w_px, h_px);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header()?;
        wr.write_image_data(&rgba)?;
    }
    Ok(png_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osm_and_unknown_ids_are_refused() {
        assert!(check("none").is_ok() && check("usgsTopo").is_ok());
        assert!(check("osm").unwrap_err().to_string().contains("bulk downloads"));
        assert!(check("bing").is_err());
    }

    #[test]
    fn tile_definitions_name_the_service() {
        let x = tile_xml("USGSTopo");
        assert!(x.contains("USGSTopo/MapServer/tile/${z}/${y}/${x}") && x.contains("<TileLevel>16"));
    }
}
