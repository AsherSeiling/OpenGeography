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

/// Tiles stay in the disk cache this many seconds before they are fetched again (30 days).
const CACHE_EXPIRES_S: u32 = 30 * 24 * 3600;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// GDAL WMS/TMS definition of a Web Mercator tile service (finest level 16 as in the map view). `url` has
/// `${z}`, `${y}`, `${x}`; with a `cache` folder GDAL keeps every tile it fetched there, so editing a layout
/// re-reads tiles from disk instead of the network. Only the tiles a preview needs are fetched (no prefetching).
fn tile_xml_at(url: &str, cache: Option<&std::path::Path>) -> String {
    let w = 20037508.342789244;
    let cache = cache
        .map(|p| format!("<Cache><Path>{}</Path><Depth>2</Depth><Expires>{CACHE_EXPIRES_S}</Expires></Cache>", xml_escape(&p.display().to_string())))
        .unwrap_or_default();
    format!(
        "<GDAL_WMS><Service name=\"TMS\"><ServerUrl>{}</ServerUrl></Service>\
         <DataWindow><UpperLeftX>-{w}</UpperLeftX><UpperLeftY>{w}</UpperLeftY><LowerRightX>{w}</LowerRightX><LowerRightY>-{w}</LowerRightY><TileLevel>16</TileLevel><TileCountX>1</TileCountX><TileCountY>1</TileCountY><YOrigin>top</YOrigin></DataWindow>\
         <Projection>EPSG:3857</Projection><BlockSizeX>256</BlockSizeX><BlockSizeY>256</BlockSizeY><BandsCount>3</BandsCount>{cache}\
         <UserAgent>GIS Studio (open-source desktop GIS; print layout)</UserAgent><Timeout>30</Timeout></GDAL_WMS>",
        xml_escape(url)
    )
}

fn tile_xml(svc: &str, cache: Option<&std::path::Path>) -> String {
    tile_xml_at(&format!("https://basemap.nationalmap.gov/arcgis/rest/services/{svc}/MapServer/tile/${{z}}/${{y}}/${{x}}"), cache)
}

/// Open a tile service (`url` with `${z}/${y}/${x}`), caching tiles in `cache`.
pub fn open_url(url: &str, cache: Option<&std::path::Path>) -> Result<Dataset> {
    Dataset::open(tile_xml_at(url, cache)).map_err(|e| anyhow!("Could not open the base map service: {e}"))
}

/// Open a USGS base map over the network (GDAL WMS driver), with the per-user tile cache.
pub fn open(id: &str) -> Result<Dataset> {
    let svc = service(id).ok_or_else(|| anyhow!("'{id}' is not a printable basemap"))?;
    let cache = crate::resources::tile_cache_dir();
    Dataset::open(tile_xml(svc, cache.as_deref())).map_err(|e| anyhow!("Could not open the USGS base map service: {e}"))
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
        let x = tile_xml("USGSTopo", None);
        assert!(x.contains("USGSTopo/MapServer/tile/${z}/${y}/${x}") && x.contains("<TileLevel>16"));
        assert!(!x.contains("<Cache>"), "no cache folder, no cache element");
        let c = tile_xml("USGSTopo", Some(std::path::Path::new("/tmp/a&b/tiles")));
        assert!(c.contains("<Cache><Path>/tmp/a&amp;b/tiles</Path><Depth>2</Depth><Expires>2592000</Expires></Cache>"), "{c}");
    }
}
