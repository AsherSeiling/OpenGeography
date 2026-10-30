//! Topo Map Builder, layout half (docs/LAYOUT_REWORK_PLAN.md §7, LW9): from a centre point, a scale and a paper it builds a
//! land-navigation topographic sheet — UTM and lat/lon ticks, grid-zone designation, declination diagram, scale bars, grid-reference
//! box, legend, title and marginal notes — and a self-check list. Data preparation (DEM, contours, hillshade) is separate.

use super::declination;
use super::geo::{Body, FrameCrs};
use super::grid::mgrs_designation;
use super::model::{Layout, LAYOUT_VERSION};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TopoSpec {
    /// Longitude, latitude of the sheet's centre (WGS 84 degrees).
    pub center: [f64; 2],
    /// Scale denominator: 24000, 25000, 50000 or any positive number.
    pub scale: f64,
    /// "letter", "tabloid", "a3" or "a4".
    pub paper: String,
    #[serde(default = "yes")]
    pub landscape: bool,
    /// "imperial" or "metric" (legend, contour interval and the third scale bar).
    #[serde(default = "imperial")]
    pub units: String,
    /// "nad83" (North America) or "wgs84".
    #[serde(default = "nad83")]
    pub datum: String,
    #[serde(default)]
    pub title: String,
    /// YYYY-MM-DD for the declination; empty = today's date from the caller.
    #[serde(default)]
    pub date: String,
}

fn yes() -> bool { true }
fn imperial() -> String { "imperial".into() }
fn nad83() -> String { "nad83".into() }

#[derive(Serialize, Clone, Debug)]
pub struct Check {
    /// "ok", "note" or "warning".
    pub level: &'static str,
    pub text: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TopoBuild {
    pub layout: Layout,
    pub checks: Vec<Check>,
    pub crs: String,
    pub zone: u8,
    /// Ground size of the map frame, metres.
    pub ground_m: [f64; 2],
    /// Contour interval to use when preparing the data, in the chosen units ("40 ft" / "10 m").
    pub contour_interval: String,
    /// The same interval in metres, the unit of the elevation raster (a "40 ft" interval is 12.192 m).
    pub contour_interval_m: f64,
    /// West, south, east, north in longitude/latitude of the map frame plus a 5 % margin: the area to download elevation for.
    pub bbox_lonlat: [f64; 4],
}

/// Space between the map and the marginal strip for the bottom labels, mm.
const GAP: f64 = 8.0;

pub const PAPERS: &[(&str, f64, f64)] = &[("letter", 279.4, 215.9), ("tabloid", 431.8, 279.4), ("a3", 420.0, 297.0), ("a4", 297.0, 210.0)];

/// UTM zone of a longitude (the Norway/Svalbard exceptions are not applied).
pub fn utm_zone_of(lon: f64) -> u8 {
    (((lon + 180.0) / 6.0).floor() as i64).clamp(0, 59) as u8 + 1
}

/// EPSG code for the sheet: NAD 83 / UTM zones 1–23 north, else WGS 84 / UTM.
pub fn utm_crs(lon: f64, lat: f64, datum: &str) -> Result<(String, u8)> {
    let zone = utm_zone_of(lon);
    match datum {
        "nad83" => {
            if lat < 0.0 || zone > 23 {
                bail!("NAD 83 UTM zones cover North America (zones 1–23 north of the equator); choose the WGS 84 datum for this place");
            }
            Ok((format!("EPSG:269{zone:02}"), zone))
        }
        "wgs84" => Ok((format!("EPSG:{}{zone:02}", if lat >= 0.0 { 326 } else { 327 }), zone)),
        d => bail!("Unknown datum '{d}' (nad83 or wgs84)"),
    }
}

/// An interval written "40 ft" or "10 m" in metres.
pub fn interval_metres(text: &str) -> f64 {
    let n: f64 = text.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(10.0);
    if text.trim_end().ends_with("ft") { n * 0.3048 } else { n }
}

/// The contour interval for the scale: metres from the scale (about scale ÷ 2400), feet the nearest standard value.
pub fn contour_interval(scale: f64, units: &str) -> String {
    let metric = [1.0, 2.0, 5.0, 10.0, 20.0, 40.0, 50.0, 100.0, 200.0];
    let near = |list: &[f64], want: f64| *list.iter().min_by(|a, b| ((*a / want).ln().abs()).total_cmp(&(*b / want).ln().abs())).unwrap();
    let m = near(&metric, scale / 2400.0);
    if units == "metric" {
        format!("{m:.0} m")
    } else {
        format!("{:.0} ft", near(&[10.0, 20.0, 40.0, 50.0, 80.0, 100.0, 200.0, 400.0], m * 3.28084))
    }
}

/// Greedy word wrap for a text box `width` mm wide at `pt` points (an average glyph is about half an em); lines are joined with newlines.
fn wrap(text: &str, width: f64, pt: f64) -> String {
    let per_line = ((width / (pt * 25.4 / 72.0 * 0.52)).floor() as usize).max(12);
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.len() + 1 + word.len() <= per_line => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines.join("\n")
}

fn group(v: f64) -> String {
    super::grouped(v.round() as u64 as _)
}

pub fn build(spec: &TopoSpec, today: &str) -> Result<TopoBuild> {
    let [lon, lat] = spec.center;
    if !(lon.is_finite() && lat.is_finite()) || lat.abs() > 80.0 || lon.abs() > 180.0 {
        bail!("The centre must be a longitude and latitude between 80°S and 80°N (UTM does not cover the poles)");
    }
    if !(spec.scale.is_finite() && spec.scale >= 1000.0 && spec.scale <= 1_000_000.0) {
        bail!("The scale must be between 1:1,000 and 1:1,000,000");
    }
    let Some(&(_, pw, ph)) = PAPERS.iter().find(|p| p.0 == spec.paper) else { bail!("Unknown paper '{}' (letter, tabloid, a3 or a4)", spec.paper) };
    if !["imperial", "metric"].contains(&spec.units.as_str()) {
        bail!("Units must be imperial or metric");
    }
    let (crs, zone) = utm_crs(lon, lat, &spec.datum)?;
    let (w, h) = if spec.landscape { (pw, ph) } else { (ph, pw) };
    let body = Body::get("earth")?;
    let fc = FrameCrs::new(&crs, &body)?;
    let (ce, cn) = fc.from_lonlat(lon, lat)?;

    // Page geometry, mm: title strip on top, the map, a marginal strip and a sources line below.
    let m = 8.0;
    let title_h = 16.0;
    let strip = 50.0;
    let src_h = 6.0;
    // Room left of the map for the latitude labels (degrees, minutes, seconds) and right of it for the grid numbers.
    let (fx, fy) = (m + 10.0, m + title_h);
    let (fw, fh) = (w - fx - m - 6.0, h - fy - m - strip - src_h - GAP);
    if fw < 80.0 || fh < 60.0 {
        bail!("The paper is too small for this layout; choose a larger paper");
    }
    let sy = fy + fh + GAP; // top of the marginal strip, below the labels and the zone designation
    let big = spec.scale <= 50_000.0;
    let grid_m = if spec.scale >= 100_000.0 { 10_000.0 } else { 1_000.0 };
    let tick_deg = if spec.scale <= 25_000.0 { 2.5 / 60.0 } else if spec.scale <= 50_000.0 { 5.0 / 60.0 } else { 0.25 };
    let interval = contour_interval(spec.scale, &spec.units);
    let title = if spec.title.trim().is_empty() { "Topographic map".to_string() } else { spec.title.trim().to_string() };

    // Marginal strip, left to right: declination, scale bars, grid-reference box, legend.
    let usable = w - 2.0 * m;
    let cell = |i: usize, gap: f64, shares: &[f64]| -> (f64, f64) {
        let total: f64 = shares.iter().sum();
        let before: f64 = shares[..i].iter().sum();
        (m + usable * before / total + if i > 0 { gap } else { 0.0 }, usable * shares[i] / total - if i > 0 { gap } else { 0.0 })
    };
    let shares = [0.26, 0.26, 0.28, 0.20];
    let (dx, dw) = cell(0, 2.0, &shares);
    let (bx, bw) = cell(1, 2.0, &shares);
    let (gx, gw) = cell(2, 2.0, &shares);
    let (lx, lw) = cell(3, 2.0, &shares);
    let dec_w = dw.min(56.0);
    let units3 = if spec.units == "metric" { "m" } else { "ft" };
    let bar_h = 11.0;
    let date = if spec.date.is_empty() { today } else { spec.date.as_str() };

    // Grid-reference example from the centre of the sheet.
    let centre_ref = mgrs_designation(zone, lat < 0.0, ce, cn, lat).map(|sq| format!("{sq} {:03.0} {:03.0}", ((ce % 100_000.0) / 100.0).floor(), ((cn % 100_000.0) / 100.0).floor())).unwrap_or_else(|| "—".into());
    let gridref_pt = 7.0;
    let gridref = format!(
        "HOW TO GIVE A GRID REFERENCE\n{}\n{}",
        wrap("Read RIGHT along the bottom or top edge to the vertical grid line on the left of your point, then UP to the horizontal line below it. The numbers on the grid lines are the first digits; estimate tenths of a square for the last digit.", gw - 3.0, gridref_pt),
        wrap(&format!("Example, the centre of this sheet: {centre_ref}"), gw - 3.0, gridref_pt)
    );
    let info = format!(
        "Scale 1:{} · {{crs}} · {} datum · {} m UTM grid · contour interval {interval} · magnetic declination from the World Magnetic Model, {date}",
        group(spec.scale),
        if spec.datum == "nad83" { "NAD 83" } else { "WGS 84" },
        group(grid_m)
    );

    let layout = json!({
        "id": "topo", "name": title.clone(), "version": LAYOUT_VERSION, "body": "earth",
        "page": { "width": w, "height": h, "units": "mm", "margin": 0, "background": "#ffffff" },
        "elements": [
            { "kind": "map_frame", "id": "f1", "name": "Map", "x": fx, "y": fy, "w": fw, "h": fh, "crs": crs, "center": [ce, cn], "scale": spec.scale,
              "layers": "map", "basemap": "none",
              "graticule": { "interval": tick_deg, "labels": true, "style": "ticks", "labelFormat": "dms", "color": "#000000", "width": 0.25, "tickLength": 3.0 },
              "grid": { "interval": grid_m, "labels": true, "style": "lines", "labelFormat": if big { "short" } else { "km" }, "color": "#1f3a5f", "width": 0.12, "designation": true, "squareLetters": big },
              "border": { "width": 0.5, "color": "#000000" } },
            { "kind": "text", "id": "title", "name": "Title", "x": m, "y": m, "w": usable, "h": 9.0, "text": title, "font": "sans-bold", "size": 16.0, "align": "center", "color": "#000000" },
            { "kind": "text", "id": "marginal", "name": "Datum and grid note", "x": m, "y": m + 9.0, "w": usable, "h": 5.0, "text": info, "font": "sans", "size": 6.5, "align": "center", "color": "#000000" },
            { "kind": "declination", "id": "dc1", "name": "Declination diagram", "frame": "f1", "x": dx + (dw - dec_w) / 2.0, "y": sy, "w": dec_w, "h": strip - 2.0, "style": "army", "units": "both", "date": date },
            { "kind": "scale_bar", "id": "sb1", "name": "Scale bar km", "frame": "f1", "x": bx, "y": sy, "w": bw, "h": bar_h, "units": "km", "segments": 4 },
            { "kind": "scale_bar", "id": "sb2", "name": "Scale bar mi", "frame": "f1", "x": bx, "y": sy + bar_h + 1.5, "w": bw, "h": bar_h, "units": "mi", "segments": 4 },
            { "kind": "scale_bar", "id": "sb3", "name": "Scale bar", "frame": "f1", "x": bx, "y": sy + 2.0 * (bar_h + 1.5), "w": bw, "h": bar_h, "units": units3, "segments": 4, "ratio": true },
            { "kind": "text", "id": "gridref", "name": "Grid reference box", "x": gx, "y": sy, "w": gw, "h": strip - 2.0, "text": gridref, "font": "sans", "size": gridref_pt, "align": "left", "color": "#000000",
              "decor": { "background": "#ffffff", "frameColor": "#000000", "frameWidth": 0.3 } },
            { "kind": "legend", "id": "lg1", "name": "Legend", "frame": "f1", "x": lx, "y": sy, "w": lw, "h": strip - 2.0, "title": "Legend", "columns": 1,
              "decor": { "background": "#ffffff", "frameColor": "#000000", "frameWidth": 0.3 } },
            { "kind": "sources", "id": "src1", "name": "Sources", "frame": "f1", "x": m, "y": h - m - src_h, "w": usable, "h": src_h, "size": 5.0 }
        ]
    });
    let layout: Layout = serde_json::from_value(layout)?;
    layout.validate()?;

    // Self-check.
    let ground = [fw / 1000.0 * spec.scale, fh / 1000.0 * spec.scale];
    let mut checks = vec![
        Check { level: "ok", text: format!("Scale 1:{} on {} {} ({:.0} × {:.0} mm): the map shows {:.1} × {:.1} km on the ground.", group(spec.scale), spec.paper, if spec.landscape { "landscape" } else { "portrait" }, w, h, ground[0] / 1000.0, ground[1] / 1000.0) },
        Check { level: "ok", text: format!("Grid: {} / UTM zone {zone}, {} m lines with {} labels; lat/lon ticks every {}.", crs, group(grid_m), if big { "two-digit" } else { "kilometre" }, if tick_deg < 0.1 { format!("{:.1}′", tick_deg * 60.0) } else { format!("{:.0}′", tick_deg * 60.0) }) },
        Check { level: "note", text: format!("Contour interval for this scale: {interval}, index contours every fifth line (prepare the contours in the Data step)." ) },
    ];
    match declination::parse_date(date).and_then(|d| declination::declination(lon, lat, d)) {
        Ok(d) => {
            checks.push(Check { level: "ok", text: format!("Declination {} at the centre on {date} (World Magnetic Model 2025, ±{:.1}°); it changes {:.0}′ a year.", declination::angle_text(d.degrees, "both"), d.sigma_degrees, d.annual_degrees.abs() * 60.0) });
        }
        Err(e) => checks.push(Check { level: "warning", text: format!("The declination could not be computed: {e}") }),
    }
    if ground[0] < 2000.0 || ground[1] < 2000.0 {
        checks.push(Check { level: "warning", text: "The map shows less than 2 km across; choose a smaller scale for land navigation.".into() });
    }
    // The area to prepare data for: the frame's corners in longitude/latitude, a little wider than the frame.
    let ext = super::geo::frame_extent(&body, &fc, [ce, cn], spec.scale, fw, fh)?;
    let (mx, my) = ((ext[2] - ext[0]) * 0.05, (ext[3] - ext[1]) * 0.05);
    let mut bbox = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for (x, y) in [(ext[0] - mx, ext[1] - my), (ext[2] + mx, ext[1] - my), (ext[0] - mx, ext[3] + my), (ext[2] + mx, ext[3] + my)] {
        let (lo, la) = fc.to_lonlat(x, y)?;
        bbox = [bbox[0].min(lo), bbox[1].min(la), bbox[2].max(lo), bbox[3].max(la)];
    }
    let contour_interval_m = interval_metres(&interval);
    Ok(TopoBuild { layout, checks, crs, zone, ground_m: ground, contour_interval: interval, contour_interval_m, bbox_lonlat: bbox })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> TopoSpec {
        serde_json::from_value(json!({ "center": [-93.0, 45.0], "scale": 24000, "paper": "tabloid", "landscape": true, "title": "Test quad" })).unwrap()
    }

    #[test]
    fn the_sheet_knows_the_area_to_download_and_the_interval_in_metres() {
        let b = build(&spec(), "2026-01-01").unwrap();
        let [w, s, e, n] = b.bbox_lonlat;
        // Centred on -93, 45; about 10.5 km × 4.8 km (the map frame on tabloid landscape at 1:24,000, plus 5 %).
        assert!(w < -93.0 && e > -93.0 && s < 45.0 && n > 45.0, "{:?}", b.bbox_lonlat);
        assert!((e - w) > 0.1 && (e - w) < 0.25 && (n - s) > 0.03 && (n - s) < 0.1, "{:?}", b.bbox_lonlat);
        assert_eq!(interval_metres("40 ft"), 12.192);
        assert_eq!(interval_metres("10 m"), 10.0);
        assert!((b.contour_interval_m - 12.192).abs() < 1e-9);
    }

    #[test]
    fn wrapping_keeps_words_whole_and_lines_short() {
        let w = wrap("Read RIGHT along the bottom edge to the vertical grid line", 40.0, 6.0);
        assert!(w.lines().count() > 1 && w.lines().all(|l| l.len() <= 40), "{w}");
        assert_eq!(w.split_whitespace().collect::<Vec<_>>().join(" "), "Read RIGHT along the bottom edge to the vertical grid line");
    }

    #[test]
    fn zones_and_datums() {
        assert_eq!(utm_zone_of(-93.0), 15);
        assert_eq!(utm_zone_of(-177.0), 1);
        assert_eq!(utm_zone_of(179.9), 60);
        assert_eq!(utm_crs(-93.0, 45.0, "nad83").unwrap(), ("EPSG:26915".into(), 15));
        assert_eq!(utm_crs(-93.0, 45.0, "wgs84").unwrap(), ("EPSG:32615".into(), 15));
        assert_eq!(utm_crs(-70.0, -33.0, "wgs84").unwrap().0, "EPSG:32719");
        assert!(utm_crs(-70.0, -33.0, "nad83").unwrap_err().to_string().contains("North America"));
        assert!(utm_crs(10.0, 50.0, "nad83").is_err());
    }

    #[test]
    fn contour_intervals_follow_the_scale() {
        assert_eq!(contour_interval(24_000.0, "metric"), "10 m");
        assert_eq!(contour_interval(24_000.0, "imperial"), "40 ft");
        assert_eq!(contour_interval(50_000.0, "metric"), "20 m");
        assert_eq!(contour_interval(250_000.0, "metric"), "100 m");
    }

    #[test]
    fn builds_a_valid_sheet_for_each_paper_and_refuses_bad_input() {
        for paper in ["letter", "tabloid", "a3", "a4"] {
            for landscape in [true, false] {
                let mut s = spec();
                s.paper = paper.into();
                s.landscape = landscape;
                let b = build(&s, "2026-01-01").unwrap_or_else(|e| panic!("{paper} {landscape}: {e}"));
                assert_eq!(b.crs, "EPSG:26915");
                // Everything lies on the page.
                for e in &b.layout.elements {
                    let at = e.at();
                    assert!(at.x >= -0.01 && at.y >= -0.01 && at.x + at.w <= b.layout.page.width + 0.01 && at.y + at.h <= b.layout.page.height + 0.01, "{paper} {landscape}: {} off the page ({} {} {} {})", at.id, at.x, at.y, at.w, at.h);
                }
            }
        }
        let mut bad = spec();
        bad.center = [0.0, 85.0];
        assert!(build(&bad, "2026-01-01").is_err());
        bad = spec();
        bad.paper = "poster".into();
        assert!(build(&bad, "2026-01-01").unwrap_err().to_string().contains("Unknown paper"));
        bad = spec();
        bad.scale = 10.0;
        assert!(build(&bad, "2026-01-01").is_err());
    }

    #[test]
    fn the_self_check_reports_scale_grid_and_declination() {
        let b = build(&spec(), "2026-01-01").unwrap();
        let all = b.checks.iter().map(|c| c.text.clone()).collect::<Vec<_>>().join("\n");
        assert!(all.contains("1:24,000") && all.contains("UTM zone 15") && all.contains("Declination") && all.contains("Contour interval"), "{all}");
        let mut late = spec();
        late.date = "2045-01-01".into();
        assert!(build(&late, "2026-01-01").unwrap().checks.iter().any(|c| c.level == "warning" && c.text.contains("declination")));
    }
}
