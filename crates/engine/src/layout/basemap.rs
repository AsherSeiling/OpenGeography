//! Base maps for print (docs/LAYOUTS_PLAN.md C2.4): GDAL's WMS/TMS driver
//! reads the National Map tile service, and the area of the frame is warped to
//! the frame CRS. Natural Earth is the bundled raster (public domain, offline).
//! Every other screen basemap (OpenStreetMap, OpenFreeMap, VersaTiles,
//! OpenTopoMap) is excluded: their tile terms forbid bulk downloads, which a
//! print export is.

use anyhow::{anyhow, bail, Result};
use gdal::Dataset;

/// Base maps a layout frame may use, besides "none".
pub const IDS: [&str; 4] = ["usgsTopo", "usgsImagery", "usgsRelief", NATURAL_EARTH];

/// The bundled Natural Earth basemap's id.
pub const NATURAL_EARTH: &str = "naturalEarth";

/// Basemaps the map view draws from a live tile service that a layout may not use, with their names (the one engine-side list of
/// them; the map view's list is `ui/src/map/basemaps.ts`).
pub const SCREEN_ONLY: [(&str, &str); 6] = [
    ("osm", "OpenStreetMap"),
    ("ofmLiberty", "OpenFreeMap Liberty"),
    ("ofmBright", "OpenFreeMap Bright"),
    ("ofmPositron", "OpenFreeMap Positron"),
    ("versatiles", "VersaTiles Colorful"),
    ("openTopoMap", "OpenTopoMap"),
];

/// Error text for a basemap id that can't be printed (None = fine).
pub fn check(id: &str) -> Result<()> {
    match id {
        "none" => Ok(()),
        i if IDS.contains(&i) => Ok(()),
        "osm" => bail!("OpenStreetMap can't be used in a print layout: its tile usage policy forbids bulk downloads. Choose a USGS base map or Natural Earth."),
        i => match SCREEN_ONLY.iter().find(|(k, _)| *k == i) {
            Some((_, name)) => bail!("{name} can't be used in a print layout: the service's terms don't allow bulk tile downloads, which a print export is. Choose a USGS base map or Natural Earth."),
            None => bail!("Unknown basemap '{i}' (use none, usgsTopo, usgsImagery, usgsRelief or naturalEarth)"),
        },
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

/// The on-screen raster tile basemaps the engine can warp to any map CRS (UR-19): id, tile URL template (`${z}`, `${x}`, `${y}` as GDAL
/// writes them) and the finest tile level. The map view's own list is `ui/src/map/basemaps.ts`; a test keeps the two in step. Only the
/// USGS ones may also be printed (`IDS`); the others are looked at on screen, one tile at a time, and are never kept on disk.
pub const SCREEN_RASTERS: [(&str, &str, u32); 5] = [
    ("usgsTopo", "https://basemap.nationalmap.gov/arcgis/rest/services/USGSTopo/MapServer/tile/${z}/${y}/${x}", 16),
    ("usgsImagery", "https://basemap.nationalmap.gov/arcgis/rest/services/USGSImageryOnly/MapServer/tile/${z}/${y}/${x}", 16),
    ("usgsRelief", "https://basemap.nationalmap.gov/arcgis/rest/services/USGSShadedReliefOnly/MapServer/tile/${z}/${y}/${x}", 16),
    ("osm", "https://tile.openstreetmap.org/${z}/${x}/${y}.png", 19),
    ("openTopoMap", "https://a.tile.opentopomap.org/${z}/${x}/${y}.png", 17),
];

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

/// GDAL definition of a Web Mercator tile service read for the screen: four bands, so a tile the server doesn't have (404 or 204) is
/// transparent and the warp leaves that place blank; no disk cache; a polite user agent and a few parallel connections.
fn screen_xml(url: &str, level: u32) -> String {
    let w = 20037508.342789244;
    format!(
        "<GDAL_WMS><Service name=\"TMS\"><ServerUrl>{}</ServerUrl></Service>\
         <DataWindow><UpperLeftX>-{w}</UpperLeftX><UpperLeftY>{w}</UpperLeftY><LowerRightX>{w}</LowerRightX><LowerRightY>-{w}</LowerRightY><TileLevel>{level}</TileLevel><TileCountX>1</TileCountX><TileCountY>1</TileCountY><YOrigin>top</YOrigin></DataWindow>\
         <Projection>EPSG:3857</Projection><BlockSizeX>256</BlockSizeX><BlockSizeY>256</BlockSizeY><BandsCount>4</BandsCount>\
         <ZeroBlockHttpCodes>204,404</ZeroBlockHttpCodes><ZeroBlockOnServerException>true</ZeroBlockOnServerException><MaxConnections>8</MaxConnections>\
         <UserAgent>GIS Studio (open-source desktop GIS; map view)</UserAgent><Timeout>20</Timeout></GDAL_WMS>",
        xml_escape(url)
    )
}

/// One map tile (`bounds` = `[xmin, ymin, xmax, ymax]` of a tile of the map CRS `crs`, `size` px square) of the Web Mercator tile service
/// at `url` (finest level `level`), warped by PROJ and returned as an RGBA PNG. The whole range where the CRS and the service both have
/// something is drawn; everywhere else (past the poles of Web Mercator, where the CRS is undefined, where the server has no tile) the
/// pixels are transparent.
pub fn screen_tile(url: &str, level: u32, crs: &str, bounds: [f64; 4], size: u32) -> Result<Vec<u8>> {
    let ds = Dataset::open(screen_xml(url, level)).map_err(|e| anyhow!("Could not open the basemap tile service: {e}"))?;
    render_rgba(&ds, crs, bounds, size, size)
}

/// An on-screen basemap tile by provider id (see [`SCREEN_RASTERS`]).
pub fn screen_raster_tile(id: &str, crs: &str, bounds: [f64; 4], size: u32) -> Result<Vec<u8>> {
    let (_, url, level) = SCREEN_RASTERS.iter().find(|(k, _, _)| *k == id).ok_or_else(|| anyhow!("'{id}' is not an on-screen raster basemap"))?;
    screen_tile(url, *level, crs, bounds, size)
}

/// Like [`render`] but keeps the source's own transparency (a four-band source) instead of replacing it.
pub fn render_rgba(ds: &Dataset, crs: &str, extent: [f64; 4], w_px: u32, h_px: u32) -> Result<Vec<u8>> {
    let (w, h) = (w_px as usize, h_px as usize);
    if w == 0 || h == 0 || w.saturating_mul(h) > 100_000_000 {
        bail!("Basemap image size {w_px}×{h_px} px is not supported");
    }
    let target = crate::crs::resolve(crs);
    let mut args: Vec<String> = ["-of", "MEM", "-t_srs", target.as_str(), "-r", "bilinear", "-dstalpha"].map(String::from).to_vec();
    args.extend(["-te".into(), extent[0].to_string(), extent[1].to_string(), extent[2].to_string(), extent[3].to_string(), "-ts".into(), w.to_string(), h.to_string()]);
    let out = crate::raster::warp(ds, "", &args)?;
    let bands = out.raster_count();
    if bands < 4 {
        bail!("The warped basemap tile has {bands} bands, not RGBA");
    }
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

/// Open a tile service (`url` with `${z}/${y}/${x}`), caching tiles in `cache`.
pub fn open_url(url: &str, cache: Option<&std::path::Path>) -> Result<Dataset> {
    Dataset::open(tile_xml_at(url, cache)).map_err(|e| anyhow!("Could not open the base map service: {e}"))
}

/// Open the bundled Natural Earth raster.
pub fn open_natural_earth() -> Result<Dataset> {
    let path = crate::resources::natural_earth_raster().ok_or_else(|| anyhow!("The bundled Natural Earth basemap file is missing from this installation"))?;
    Dataset::open(&path).map_err(|e| anyhow!("Could not open the Natural Earth basemap: {e}"))
}

/// One map tile of the Natural Earth basemap on a grid of the map CRS `crs`: `bounds` (`[xmin, ymin, xmax, ymax]`) at `size` px
/// square, as PNG. Works in any CRS GDAL can warp to; pixels outside the data are transparent.
pub fn natural_earth_tile(crs: &str, bounds: [f64; 4], size: u32) -> Result<Vec<u8>> {
    let ds = open_natural_earth()?;
    render(&ds, crs, bounds, size, size)
}

/// Open a base map (GDAL WMS driver over the network for USGS, with the per-user tile cache; the bundled raster for Natural Earth).
pub fn open(id: &str) -> Result<Dataset> {
    if id == NATURAL_EARTH {
        return open_natural_earth();
    }
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
        assert!(check("naturalEarth").is_ok());
        for (id, name) in SCREEN_ONLY {
            let e = check(id).unwrap_err().to_string();
            assert!(e.contains(name) && e.contains("bulk"), "{e}");
        }
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
