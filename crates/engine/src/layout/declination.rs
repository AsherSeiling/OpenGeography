//! Declination diagram (docs/LAYOUT_REWORK_PLAN.md §6, LW6): the map-margin drawing of grid north (GN), true north (TN, a star)
//! and magnetic north (MN, a half arrowhead) with the angles between them. Magnetic declination and its yearly change come from
//! the World Magnetic Model (WMM2025, NOAA/BGS, public domain) through the `world_magnetic_model` crate; the grid convergence is the
//! frame's own (`Factors`). Everything is drawn as vector SVG in mm.

use super::model::Placement;
use super::surround::{open_group, text_slot, PT_MM};
use super::svg::{self, esc, mm};
use anyhow::{anyhow, bail, Result};
use std::fmt::Write;
use world_magnetic_model::time::{Date, Month};
use world_magnetic_model::uom::si::angle::degree;
use world_magnetic_model::uom::si::f32::{Angle, Length};
use world_magnetic_model::uom::si::length::meter;
use world_magnetic_model::GeomagneticField;

pub const STYLES: &[&str] = &["usgs", "army", "simple"];
pub const UNITS: &[&str] = &["both", "deg", "mils"];
const SLOT: &str = "declination.text";
pub const DECLINATION_PT: f64 = 7.0;
/// NATO mils in a full circle.
/// Smallest spread (degrees) the drawing opens small angles up to.
const MIN_SHOWN_DEG: f64 = 10.0;
const MILS_PER_CIRCLE: f64 = 6400.0;

/// Magnetic declination at a place and date.
#[derive(Clone, Copy, Debug)]
pub struct Declination {
    /// Degrees, positive east of true north.
    pub degrees: f64,
    /// Change over the following 365 days, degrees (positive = towards east).
    pub annual_degrees: f64,
    /// 1σ model uncertainty, degrees.
    pub sigma_degrees: f64,
}

/// "YYYY-MM-DD" → a calendar date.
pub fn parse_date(s: &str) -> Result<Date> {
    let bad = || anyhow!("The declination date must be written YYYY-MM-DD, not '{s}'");
    let mut it = s.trim().split('-');
    let (y, m, d) = (it.next().ok_or_else(bad)?, it.next().ok_or_else(bad)?, it.next().ok_or_else(bad)?);
    if it.next().is_some() {
        return Err(bad());
    }
    let month = Month::try_from(m.parse::<u8>().map_err(|_| bad())?).map_err(|_| bad())?;
    Date::from_calendar_date(y.parse().map_err(|_| bad())?, month, d.parse().map_err(|_| bad())?).map_err(|_| bad())
}

/// WMM declination at `lon`/`lat` (WGS 84 degrees), sea level, on `date`.
pub fn declination(lon: f64, lat: f64, date: Date) -> Result<Declination> {
    let at = |d: Date| GeomagneticField::new(Length::new::<meter>(0.0), Angle::new::<degree>(lat as f32), Angle::new::<degree>(lon as f32), d).map_err(|e| anyhow!("The World Magnetic Model cannot be evaluated here: {e}"));
    let now = at(date)?;
    // The yearly change: the same place a year later (or earlier at the end of the model's life).
    let later = date.replace_year(date.year() + 1).ok().and_then(|d| at(d).ok());
    let annual = match later {
        Some(l) => l.declination().get::<degree>() as f64 - now.declination().get::<degree>() as f64,
        None => 0.0,
    };
    Ok(Declination { degrees: now.declination().get::<degree>() as f64, annual_degrees: annual, sigma_degrees: now.declination_uncertainty().get::<degree>() as f64 })
}

pub fn mils(deg: f64) -> f64 {
    deg * MILS_PER_CIRCLE / 360.0
}

fn dir(v: f64) -> &'static str {
    if v < 0.0 { "W" } else { "E" }
}

/// An angle as the unit choice asks: "1.9° W", "34 mils W" or "1.9° W (34 mils)".
pub fn angle_text(deg: f64, units: &str) -> String {
    let (a, d) = (deg.abs(), dir(deg));
    match units {
        "deg" => format!("{a:.1}° {d}"),
        "mils" => format!("{:.0} mils {d}", mils(a)),
        _ => format!("{a:.1}° {d} ({:.0} mils)", mils(a)),
    }
}

pub struct Opts<'a> {
    pub style: &'a str,
    pub units: &'a str,
    /// "YYYY-MM-DD"; empty = the layout's date.
    pub date: &'a str,
    pub color: &'a str,
    /// Open small angles up so the lines can be told apart (the caption says so).
    pub exaggerate: bool,
}

/// What the diagram needs from its map frame.
pub struct FrameGeo {
    pub center_lonlat: Option<(f64, f64)>,
    /// Bearing of true north from grid up, degrees clockwise (0 for a geographic frame).
    pub true_north_deg: f64,
    /// False for a geographic frame: there is no grid north to draw.
    pub projected: bool,
    pub rotation: f64,
}

/// A line from the diagram's origin at `bearing` degrees clockwise from page-up.
fn tip(o: (f64, f64), len: f64, bearing: f64) -> (f64, f64) {
    let r = bearing.to_radians();
    (o.0 + len * r.sin(), o.1 - len * r.cos())
}

fn star(out: &mut String, c: (f64, f64), r: f64, bearing: f64, color: &str) {
    let mut d = String::new();
    for i in 0..10 {
        let rad = if i % 2 == 0 { r } else { r * 0.4 };
        let p = tip(c, rad, bearing + i as f64 * 36.0);
        svg::cmd(&mut d, if i == 0 { 'M' } else { 'L' }, p.0, p.1);
    }
    let _ = write!(out, "<path d=\"{d}Z\" fill=\"{color}\" stroke=\"none\"/>");
}

/// Half arrowhead (one barb) at the end of a line.
fn half_arrow(out: &mut String, end: (f64, f64), bearing: f64, size: f64, color: &str) {
    let back = tip(end, size, bearing + 180.0 - 18.0);
    let _ = write!(out, "<path d=\"M{} {}L{} {}L{} {}Z\" fill=\"{color}\" stroke=\"{color}\" stroke-width=\"0.15\"/>", mm(end.0), mm(end.1), mm(back.0), mm(back.1), mm(tip(end, size * 0.8, bearing + 180.0).0), mm(tip(end, size * 0.8, bearing + 180.0).1));
}

fn arc(out: &mut String, o: (f64, f64), r: f64, from: f64, to: f64, color: &str) {
    if (to - from).abs() < 1e-6 {
        return;
    }
    let (a, b) = (tip(o, r, from), tip(o, r, to));
    let _ = write!(
        out,
        "<path d=\"M{} {}A{} {} 0 0 {} {} {}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"0.2\"/>",
        mm(a.0), mm(a.1), mm(r), mm(r), if to > from { 1 } else { 0 }, mm(b.0), mm(b.1)
    );
}

/// Where to put the tip labels: the lines' bearings, pushed apart (order kept, centred on their middle) so "GN", "TN" and "MN" never overprint when the angles are small.
pub fn spread(bearings: &[f64], min_gap: f64) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..bearings.len()).collect();
    idx.sort_by(|a, b| bearings[*a].total_cmp(&bearings[*b]));
    let mut out: Vec<f64> = idx.iter().map(|i| bearings[*i]).collect();
    for _ in 0..8 {
        for k in 1..out.len() {
            let gap = out[k] - out[k - 1];
            if gap < min_gap {
                let push = (min_gap - gap) / 2.0;
                out[k - 1] -= push;
                out[k] += push;
            }
        }
    }
    let mut res = vec![0.0; bearings.len()];
    for (k, i) in idx.iter().enumerate() {
        res[*i] = out[k];
    }
    res
}

/// Draw the diagram into `at`. Returns the computed declination (for tests and the UI's read-out).
pub fn draw(out: &mut String, warnings: &mut Vec<String>, at: &Placement, o: &Opts, geo: &FrameGeo, layout_date: &str) -> Option<Declination> {
    let color = svg::color(o.color, "#000000");
    let style = if o.style.is_empty() { "usgs" } else { o.style };
    let units = if o.units.is_empty() { "both" } else { o.units };
    let date_s = if o.date.is_empty() { layout_date } else { o.date };
    let pt = at.slot_pt(SLOT, DECLINATION_PT);
    let lh = pt * PT_MM * 1.35;

    let dec = match (geo.center_lonlat, parse_date(date_s)) {
        (Some((lon, lat)), Ok(date)) => match declination(lon, lat, date) {
            Ok(d) => Some(d),
            Err(e) => {
                warnings.push(format!("Declination diagram '{}': {e}; outside the model's years (2020–2029) or latitude limits the magnetic line is left out", at.id));
                None
            }
        },
        (None, _) => {
            warnings.push(format!("Declination diagram '{}': the frame's centre has no longitude and latitude in its coordinate system", at.id));
            None
        }
        (_, Err(e)) => {
            warnings.push(format!("Declination diagram '{}': {e}", at.id));
            None
        }
    };

    // Bearings on the page, clockwise from up: grid north is up (turned with the frame), the others follow from the angles.
    let (tn_rel, mn_rel) = (geo.true_north_deg, dec.map(|d| geo.true_north_deg + d.degrees));
    let widest_angle = tn_rel.abs().max(mn_rel.map_or(0.0, f64::abs));
    // A 1.6° angle is a hair on a 40 mm diagram: open anything under 10° up to 10° (all relative angles by one factor).
    let k = if o.exaggerate && widest_angle > 1e-9 && widest_angle < MIN_SHOWN_DEG { MIN_SHOWN_DEG / widest_angle } else { 1.0 };
    let gn = geo.rotation;
    let tn = tn_rel * k + geo.rotation;
    let mn = mn_rel.map(|m| m * k + geo.rotation);

    // Layout: the diagram above, the caption below.
    let caption_lines = match style {
        "army" => 3,
        "simple" => 2,
        _ if geo.projected => 4,
        _ => 3,
    };
    let cap_h = lh * (caption_lines + usize::from(k != 1.0)) as f64 + 0.8;
    let len = ((at.h - cap_h - pt * PT_MM * 1.2) * 0.95).min(at.w * 0.55).max(4.0);
    let o_pt = (at.x + at.w / 2.0, at.y + at.h - cap_h - 0.5);
    open_group(out, "declination", at);
    let _ = write!(out, "<g stroke=\"{color}\" stroke-width=\"0.3\" fill=\"none\" stroke-linecap=\"round\">");
    let label = |out: &mut String, p: (f64, f64), t: &str, anchor: &str| text_slot(out, p.0, p.1, t, "sans-bold", pt, anchor, at, SLOT);
    let tip_label = |bearing: f64| tip(o_pt, len + pt * PT_MM * 0.9, bearing);

    if geo.projected && style != "simple" {
        let e = tip(o_pt, len, gn);
        let _ = write!(out, "<path id=\"declination-{}-gn\" d=\"M{} {}L{} {}\"/>", esc(&at.id), mm(o_pt.0), mm(o_pt.1), mm(e.0), mm(e.1));
    }
    let e = tip(o_pt, len, tn);
    let _ = write!(out, "<path id=\"declination-{}-tn\" d=\"M{} {}L{} {}\"/>", esc(&at.id), mm(o_pt.0), mm(o_pt.1), mm(e.0), mm(e.1));
    if let Some(m) = mn {
        let e = tip(o_pt, len, m);
        let _ = write!(out, "<path id=\"declination-{}-mn\" d=\"M{} {}L{} {}\"/>", esc(&at.id), mm(o_pt.0), mm(o_pt.1), mm(e.0), mm(e.1));
    }
    out.push_str("</g>");
    // Heads, letters and the angle arcs.
    let _ = write!(out, "<g fill=\"{color}\">");
    star(out, tip(o_pt, len, tn), pt * PT_MM * 0.55, tn, &color);
    if let Some(m) = mn {
        half_arrow(out, tip(o_pt, len, m), m, pt * PT_MM * 0.9, &color);
    }
    out.push_str("</g>");
    // Tip labels at spread-out bearings; the anchor follows which side of the middle one each sits on.
    let mut names: Vec<(&str, f64)> = Vec::new();
    if geo.projected && style != "simple" {
        names.push(("GN", gn));
    }
    names.push(("TN", tn));
    if let Some(m) = mn {
        names.push(("MN", m));
    }
    let placed = spread(&names.iter().map(|n| n.1).collect::<Vec<_>>(), 14.0);
    let mid = placed.iter().sum::<f64>() / placed.len() as f64;
    for ((name, _), b) in names.iter().zip(&placed) {
        let anchor = if (b - mid).abs() < 1.0 { "middle" } else if *b > mid { "start" } else { "end" };
        label(out, tip_label(*b), name, anchor);
    }
    if let Some(m) = mn {
        // Arcs: declination (TN→MN), and grid–magnetic angle when GN is drawn.
        arc(out, o_pt, len * 0.45, tn, m, &color);
        if geo.projected && style != "simple" {
            arc(out, o_pt, len * 0.62, gn, m, &color);
        }
    }
    if geo.projected && style != "simple" {
        arc(out, o_pt, len * 0.3, gn, tn, &color);
    }

    // Caption.
    let mut y = at.y + at.h - cap_h + lh;
    let cx = at.x + at.w / 2.0;
    let line = |out: &mut String, y: &mut f64, t: &str| {
        text_slot(out, cx, *y, t, "sans", pt, "middle", at, SLOT);
        *y += lh;
    };
    let year = date_s.split('-').next().unwrap_or("");
    match (dec, style) {
        (Some(d), "army") => {
            let gm = d.degrees + geo.true_north_deg;
            line(out, &mut y, &format!("Declination {}", angle_text(d.degrees, units)));
            if geo.projected {
                line(out, &mut y, &format!("Grid convergence {}", angle_text(-geo.true_north_deg, units)));
                line(out, &mut y, &format!("G-M angle {}", angle_text(gm, units)));
            } else {
                line(out, &mut y, &format!("{year} · annual change {:.0}′ {}", d.annual_degrees.abs() * 60.0, dir(d.annual_degrees)));
            }
        }
        (Some(d), "simple") => {
            line(out, &mut y, &format!("Magnetic declination {}", angle_text(d.degrees, units)));
            line(out, &mut y, &format!("{year} · changing {:.0}′ {} a year", d.annual_degrees.abs() * 60.0, dir(d.annual_degrees)));
        }
        (Some(d), _) => {
            line(out, &mut y, &format!("{year} MAGNETIC NORTH DECLINATION"));
            line(out, &mut y, &format!("{} at centre of sheet", angle_text(d.degrees, units)));
            line(out, &mut y, &format!("annual change {:.0}′ {}", d.annual_degrees.abs() * 60.0, dir(d.annual_degrees)));
            if geo.projected {
                line(out, &mut y, &format!("Grid convergence {}", angle_text(-geo.true_north_deg, units)));
            }
        }
        (None, _) => line(out, &mut y, "Magnetic declination not available"),
    }
    if k != 1.0 {
        line(out, &mut y, "angles exaggerated, not to scale");
    }
    out.push_str("</g>");
    // Rough width of the longest caption line (an average glyph is about 0.55 em).
    let widest = 38.0 * pt * PT_MM * 0.55;
    if widest > at.w + 1e-6 {
        warnings.push(format!("Declination diagram '{}' is narrower than its caption ({:.0} mm needed); make it wider or the text smaller", at.id, widest));
    }
    if let Some(d) = dec {
        if d.sigma_degrees > 1.0 {
            warnings.push(format!("Declination diagram '{}': the model's uncertainty here is ±{:.1}°; do not navigate by it near the magnetic poles", at.id, d.sigma_degrees));
        }
    }
    dec
}

/// The diagram's options must be known words; the date a real date.
pub fn check(style: &str, units: &str, date: &str) -> Result<()> {
    if !style.is_empty() && !STYLES.contains(&style) {
        bail!("Unknown declination diagram style '{style}' ({})", STYLES.join(", "));
    }
    if !units.is_empty() && !UNITS.contains(&units) {
        bail!("Unknown declination units '{units}' ({})", UNITS.join(", "));
    }
    if !date.is_empty() {
        parse_date(date)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(lon: f64, lat: f64, y: i32, m: u8, day: u8) -> Declination {
        declination(lon, lat, parse_date(&format!("{y}-{m:02}-{day:02}")).unwrap()).unwrap()
    }

    #[test]
    fn dates_parse_and_bad_ones_are_explained() {
        assert!(parse_date("2026-01-01").is_ok());
        for bad in ["2026-13-01", "2026-02-30", "soon", "2026-01", "2026-01-01-01"] {
            assert!(parse_date(bad).unwrap_err().to_string().contains("YYYY-MM-DD"), "{bad}");
        }
    }

    #[test]
    fn matches_the_published_example_of_the_model_crate() {
        // The crate's documented evaluation: 100 m, 37.03 N, 7.91 W, day 15 of 2029 → −0.17° ± 0.33°. (Here at sea level, so close.)
        let v = d(-7.91, 37.03, 2029, 1, 15);
        assert!((v.degrees - -0.17).abs() < 0.02, "{v:?}");
        assert!((v.sigma_degrees - 0.33).abs() < 0.05, "{v:?}");
    }

    #[test]
    fn matches_noaas_wmm2025_test_values() {
        // NOAA/NCEI WMM2025_TEST_VALUES.txt (public domain), sea level: date, lat, lon, declination D, yearly change Ddot.
        for (y, m, day, lat, lon, dec, ddot) in [
            (2025, 1, 1, 80.0, 0.0, 1.28, 0.52),
            (2025, 1, 1, 0.0, 120.0, -0.16, -0.03),
            (2025, 1, 1, -80.0, -120.0, 68.78, -0.12),
            (2027, 7, 2, 80.0, 0.0, 2.59, 0.53),
            (2027, 7, 2, 0.0, 120.0, -0.24, -0.03),
            (2027, 7, 2, -80.0, -120.0, 68.49, -0.12),
        ] {
            let v = d(lon, lat, y, m, day);
            assert!((v.degrees - dec).abs() < 0.02, "{y} {lat} {lon}: {} vs {dec}", v.degrees);
            assert!((v.annual_degrees - ddot).abs() < 0.03, "{y} {lat} {lon}: change {} vs {ddot}", v.annual_degrees);
        }
    }

    #[test]
    fn well_known_places_have_the_right_sign_and_size() {
        // Signs and sizes known from NOAA's calculator (±1° tolerance; the exact NOAA table is a separate check).
        let seattle = d(-122.33, 47.61, 2026, 1, 1);
        assert!(seattle.degrees > 14.0 && seattle.degrees < 16.5, "{seattle:?}");
        let boston = d(-71.06, 42.36, 2026, 1, 1);
        assert!(boston.degrees < -13.0 && boston.degrees > -15.5, "{boston:?}");
        let denver = d(-104.99, 39.74, 2026, 1, 1);
        assert!(denver.degrees > 6.5 && denver.degrees < 9.0, "{denver:?}");
        // The field drifts: Boston's west declination is changing by a fraction of a degree a year.
        assert!(boston.annual_degrees.abs() < 0.5 && boston.annual_degrees.abs() > 0.01, "{boston:?}");
    }

    #[test]
    fn tip_labels_are_pushed_apart_but_keep_their_order() {
        let p = spread(&[0.0, 0.1, -0.2], 14.0);
        assert!(p[2] < p[0] && p[0] < p[1], "{p:?}");
        assert!(p[0] - p[2] >= 13.9 && p[1] - p[0] >= 13.9, "{p:?}");
        assert_eq!(spread(&[0.0, 30.0], 14.0), vec![0.0, 30.0]);
    }

    #[test]
    fn angles_in_degrees_mils_or_both() {
        assert_eq!(angle_text(-1.9, "deg"), "1.9° W");
        assert_eq!(angle_text(10.0, "mils"), "178 mils E");
        assert_eq!(angle_text(-1.9, "both"), "1.9° W (34 mils)");
        assert!((mils(360.0) - 6400.0).abs() < 1e-9);
    }

    #[test]
    fn outside_the_model_years_is_an_error_not_a_guess() {
        assert!(declination(-93.0, 45.0, parse_date("2040-01-01").unwrap()).is_err());
    }
}
