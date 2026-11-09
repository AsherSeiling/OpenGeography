//! Go To Coordinates (RB4): read a typed position in any supported form, say in plain words how it was read, and return the place as
//! WGS 84 lon/lat, in the map's coordinate system, and written in the other common formats.
//!
//! Nothing is guessed: the CRS and datum of every input are explicit (the request names them), latitude and longitude are separate
//! fields, a value that looks swapped is refused with a message that says so (never swapped for the user), and a place the map's CRS
//! cannot show gets a sentence instead of a jump. Typed coordinates in shape editing (`edit_tools::edit_parse_coords`) use the same
//! number reader and the same USNG/MGRS reader (`two_numbers`, `usng_to_lonlat`), so there is one parser.
use crate::{crs, usng, Engine};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};

/// What the panel sends. Every field is the text of one box, so what was typed is exactly what is read.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct LocationRequest {
    /// "latlon", "xy" (x/y in the map's own CRS), "utm", "usng" or "crs" (x/y in `crs`).
    pub kind: String,
    pub lat: String,
    pub lon: String,
    pub x: String,
    pub y: String,
    pub zone: String,
    /// "N" or "S".
    pub hemisphere: String,
    pub easting: String,
    pub northing: String,
    /// A USNG/MGRS reference.
    pub text: String,
    /// latlon and utm: "wgs84" (default) or "nad83". usng: "wgs84" (default), "nad83" or "auto".
    pub datum: String,
    /// The CRS of x/y for kind "crs".
    pub crs: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct LocationFormat {
    pub id: String,
    pub label: String,
    pub text: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocationResult {
    /// The place on WGS 84.
    pub lon: f64,
    pub lat: f64,
    /// The place in the map's coordinate system, or None when the map has none or cannot show it (then `map_message` says why).
    pub map_x: Option<f64>,
    pub map_y: Option<f64>,
    pub map_crs: Option<String>,
    pub map_message: Option<String>,
    /// How the input was read, in a sentence.
    pub reading: String,
    pub formats: Vec<LocationFormat>,
}

/// Split a line into exactly two numbers (separated by comma, semicolon or space). Shared with typed coordinates in shape editing.
pub fn two_numbers(line: &str) -> Result<(f64, f64)> {
    let p: Vec<&str> = line.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
    match p.as_slice() {
        [a, b] => Ok((a.parse::<f64>().map_err(|_| anyhow!("'{a}' is not a number"))?, b.parse::<f64>().map_err(|_| anyhow!("'{b}' is not a number"))?)),
        _ => bail!("expected two numbers (for example 500000, 4000000), found '{}'", line.trim()),
    }
}

/// A USNG/MGRS reference as WGS 84 lon/lat of the centre of the cell it names, with the cell width in metres. The grid is on `datum`
/// (polar references are on WGS 84 whatever the datum, as UPS has one datum). Shared with typed coordinates in shape editing.
pub fn usng_to_lonlat(conn: &Connection, text: &str, datum: usng::Datum) -> Result<(f64, f64, f64, String)> {
    let r = usng::parse(text.trim())?;
    let north = if r.zone == 0 { r.band == 'Y' || r.band == 'Z' } else { r.band >= 'N' };
    let code = usng::utm_crs(r.zone, north, datum);
    let to_ll = |e: f64, n: f64, _: bool| crs::transform_point(conn, e, n, &code, crs::WGS84);
    let (e, n, _) = if r.zone == 0 { usng::to_ups(&r)? } else { usng::to_utm(&r, &to_ll)? };
    let half = r.cell_m() / 2.0;
    let (lon, lat) = to_ll(e + half, n + half, north)?;
    Ok((lon, lat, r.cell_m(), code))
}

/// Read one latitude or longitude box: decimal degrees, or degrees minutes seconds, with or without the symbols ° ′ ″ (or ' "),
/// with a minus sign or a hemisphere letter (N S for latitude, E W for longitude) before or after. A minus sign together with a
/// letter, a letter of the other axis, minutes or seconds of 60 or more, and decimals on anything but the last part are refused.
pub fn parse_angle(text: &str, is_lat: bool) -> Result<f64> {
    let name = if is_lat { "latitude" } else { "longitude" };
    let t = text.trim();
    if t.is_empty() {
        bail!("Type the {name}.");
    }
    let chars: Vec<char> = t.chars().collect();
    let mut hemi: Option<char> = None;
    let mut rest = String::new();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            'N' | 'n' | 'S' | 's' | 'E' | 'e' | 'W' | 'w' => {
                if hemi.is_some() {
                    bail!("'{t}' has more than one of the letters N, S, E and W.");
                }
                let at_edge = chars[..i].iter().all(|c| !c.is_alphanumeric() && *c != '.') || chars[i + 1..].iter().all(|c| !c.is_alphanumeric() && *c != '.');
                if !at_edge {
                    bail!("In '{t}' the letter {c} must come before or after the numbers, not between them.");
                }
                hemi = Some(c.to_ascii_uppercase());
                rest.push(' ');
            }
            '°' | 'º' | '˚' | '′' | '″' | '\'' | '"' | '’' | '”' | '‘' | '“' | '`' | ':' => rest.push(' '),
            ',' => bail!("'{t}': use a point for the decimal mark, and put each coordinate in its own box."),
            c if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c.is_whitespace() => rest.push(c),
            other => bail!("'{t}' has '{other}', which can't be part of a {name}. Use digits, ° ′ ″ and a letter N, S, E or W."),
        }
    }
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.is_empty() || parts.len() > 3 {
        bail!("'{t}' should be decimal degrees (34.0522) or degrees, minutes and seconds (34 03 08).");
    }
    let num = |s: &str| s.parse::<f64>().map_err(|_| anyhow!("'{s}' in '{t}' is not a number"));
    let deg = num(parts[0])?;
    let negative = parts[0].starts_with('-');
    let mut value = deg.abs();
    if parts.len() > 1 {
        if deg.fract() != 0.0 {
            bail!("'{t}': when minutes are given, the degrees must be a whole number.");
        }
        let m = num(parts[1])?;
        if !(0.0..60.0).contains(&m) || parts[1].starts_with('-') {
            bail!("'{t}': minutes run from 0 up to 60.");
        }
        if parts.len() == 3 && m.fract() != 0.0 {
            bail!("'{t}': when seconds are given, the minutes must be a whole number.");
        }
        value += m / 60.0;
        if parts.len() == 3 {
            let s = num(parts[2])?;
            if !(0.0..60.0).contains(&s) || parts[2].starts_with('-') {
                bail!("'{t}': seconds run from 0 up to 60.");
            }
            value += s / 3600.0;
        }
    }
    let mut v = if negative { -value } else { value };
    if let Some(h) = hemi {
        let (ok, other) = if is_lat { ("NS", "longitude (E or W)") } else { ("EW", "latitude (N or S)") };
        if !ok.contains(h) {
            bail!("'{t}' ends in {h}, which belongs to a {other}. This box is the {name}: put it in the other box.");
        }
        if negative {
            bail!("'{t}' has both a minus sign and the letter {h}; use one or the other.");
        }
        if h == 'S' || h == 'W' {
            v = -v;
        }
    }
    if !v.is_finite() {
        bail!("'{t}' is not a usable {name}.");
    }
    Ok(v)
}

/// Refuse a latitude beyond 90° or a longitude beyond 180°. When the pair would be valid the other way round the message says the
/// values look swapped; nothing is ever swapped for the user.
pub fn check_lat_lon(lat: f64, lon: f64) -> Result<()> {
    if lat.abs() > 90.0 {
        if lon.abs() <= 90.0 && lat.abs() <= 180.0 {
            bail!(
                "The latitude {lat}° is beyond 90°, but it would be a valid longitude, and {lon}° would be a valid latitude. The two values look swapped. Nothing was changed: check which box each number belongs in."
            );
        }
        bail!("The latitude {lat}° is beyond 90°. Latitude runs from 90° S to 90° N.");
    }
    if lon.abs() > 180.0 {
        bail!("The longitude {lon}° is beyond 180°. Longitude runs from 180° W to 180° E.");
    }
    Ok(())
}

fn number(text: &str, what: &str) -> Result<f64> {
    let t = text.trim();
    if t.is_empty() {
        bail!("Type the {what}.");
    }
    let v: f64 = t.parse().map_err(|_| anyhow!("'{t}' is not a number for the {what}."))?;
    if !v.is_finite() {
        bail!("'{t}' is not a usable {what}.");
    }
    Ok(v)
}

fn hemi_letter(lat: f64, pos: char, neg: char) -> char {
    if lat >= 0.0 { pos } else { neg }
}

/// Degrees, minutes and seconds to 0.01″ (rounded as a whole so 59.999″ carries into the minute).
fn dms(v: f64, pos: char, neg: char) -> String {
    let total = (v.abs() * 3600.0 * 100.0).round() as i64;
    let (d, m, s) = (total / 360_000, (total % 360_000) / 6000, (total % 6000) as f64 / 100.0);
    format!("{d}°{m:02}′{s:05.2}″{}", hemi_letter(v, pos, neg))
}

fn datum_label(d: usng::Datum) -> &'static str {
    match d {
        usng::Datum::Wgs84 => "WGS 84",
        usng::Datum::Nad83 => "NAD 83",
        usng::Datum::Auto => "NAD 83 in zones 1-23 north of the equator, WGS 84 elsewhere",
    }
}

/// "15T VG 97470 47556" or a polar reference, for a WGS 84 place, with the grid on `datum`.
fn grid_ref(conn: &Connection, lon: f64, lat: f64, style: usng::Style, datum: usng::Datum) -> Result<Option<String>> {
    if lat.abs() > 90.0 {
        return Ok(None);
    }
    if usng::is_polar(lat) {
        let code = usng::utm_crs(0, lat >= 0.0, datum);
        let (e, n) = crs::transform_point(conn, lon, lat, crs::WGS84, &code)?;
        return Ok(usng::format_ups(lon, lat, e, n, 5, style));
    }
    let zone = usng::zone_of(lon, lat);
    let code = usng::utm_crs(zone, lat >= 0.0, datum);
    let (e, n) = crs::transform_point(conn, lon, lat, crs::WGS84, &code)?;
    Ok(usng::format_ref(zone, lat, e, n, 5, style))
}

fn utm_text(conn: &Connection, lon: f64, lat: f64, datum: usng::Datum) -> Result<String> {
    if usng::is_polar(lat) {
        return Ok("Not in UTM: beyond 84° N or 80° S (see the polar USNG / MGRS reference)".into());
    }
    let zone = usng::zone_of(lon, lat);
    let north = lat >= 0.0;
    let code = usng::utm_crs(zone, north, datum);
    let (e, n) = crs::transform_point(conn, lon, lat, crs::WGS84, &code)?;
    Ok(format!("Zone {zone} {}, easting {e:.1} m, northing {n:.1} m", if north { "N" } else { "S" }))
}

/// Does the map's CRS cover the place? Outside its area of use (a margin of one UTM zone in longitude and half a degree in latitude
/// is allowed, as maps are routinely used just beyond a zone) the answer is no, and so when the point does not convert back to
/// the same place (a transverse projection far from its central meridian, a Mercator map at the pole).
fn shown_on_map(conn: &Connection, map_crs: &str, lon: f64, lat: f64) -> Result<(f64, f64), String> {
    let info = crs::info(map_crs).map_err(|_| format!("The map's coordinate system ({map_crs}) can't be read, so the position can't be placed on it."))?;
    if let Some([w, s, e, n]) = info.area_of_use {
        let in_lon = if w <= e { lon >= w - 6.0 && lon <= e + 6.0 } else { lon >= w - 6.0 || lon <= e + 6.0 };
        if !in_lon || lat < s - 0.5 || lat > n + 0.5 {
            let which = if lon >= 0.0 { "E" } else { "W" };
            return Err(format!(
                "{} can only draw places from {s:.1}° to {n:.1}° latitude and {w:.1}° to {e:.1}° longitude, and this place ({lat:.5}° N, {:.5}° {which}) is outside that. Change the map's coordinate system (Map, Coordinate System), or go to a place inside that area.",
                info.name,
                lon.abs()
            ));
        }
    }
    let cant = || format!("{} can't show this place ({lat:.5}°, {lon:.5}°): it lies where the projection breaks down. Change the map's coordinate system (Map, Coordinate System) to go there.", info.name);
    let (x, y) = crs::transform_point(conn, lon, lat, crs::WGS84, map_crs).map_err(|_| cant())?;
    if !x.is_finite() || !y.is_finite() {
        return Err(cant());
    }
    let (lo2, la2) = crs::transform_point(conn, x, y, map_crs, crs::WGS84).map_err(|_| cant())?;
    let dlon = ((lo2 - lon + 540.0).rem_euclid(360.0) - 180.0).abs();
    if (la2 - lat).abs() > 1e-5 || (lat.abs() < 89.9 && dlon > 1e-5) {
        return Err(cant());
    }
    Ok((x, y))
}

impl Engine {
    /// Read a typed position. `map_crs` is the map's coordinate system (None while the map has none). Errors are sentences the panel
    /// shows as they are; a position the map cannot show is NOT an error: it comes back with `map_message`, so the panel can still show
    /// how the input was read and the other formats.
    pub fn parse_location(&self, map_crs: Option<&str>, req: &LocationRequest) -> Result<LocationResult> {
        let map_crs = map_crs.map(str::trim).filter(|s| !s.is_empty());
        self.with_conn(|conn| {
            // (lon, lat on WGS 84, how it was read, the input in its own CRS: x, y, crs id)
            let (lon, lat, reading, native, usng_datum): (f64, f64, String, Option<(f64, f64, String)>, usng::Datum) = match req.kind.as_str() {
                "latlon" => {
                    let (lat_in, lon_in) = (parse_angle(&req.lat, true)?, parse_angle(&req.lon, false)?);
                    check_lat_lon(lat_in, lon_in)?;
                    let nad = match req.datum.to_lowercase().as_str() {
                        "" | "wgs84" => false,
                        "nad83" => true,
                        o => bail!("Unknown datum '{o}' (WGS 84 or NAD 83)."),
                    };
                    let said = format!("latitude {:.6}° {}, longitude {:.6}° {}", lat_in.abs(), hemi_letter(lat_in, 'N', 'S'), lon_in.abs(), hemi_letter(lon_in, 'E', 'W'));
                    if nad {
                        let (lo, la) = crs::transform_point(conn, lon_in, lat_in, "EPSG:4269", crs::WGS84)?;
                        let note = crs::datum_change_note("EPSG:4269", crs::WGS84).unwrap_or_default();
                        (lo, la, format!("Read as {said} on NAD 83 (EPSG:4269), moved to WGS 84 by an explicit PROJ transform. {note}").trim().to_string(), Some((lon_in, lat_in, "EPSG:4269".into())), usng::Datum::Nad83)
                    } else {
                        (lon_in, lat_in, format!("Read as {said} on WGS 84 (EPSG:4326)."), Some((lon_in, lat_in, crs::WGS84.into())), usng::Datum::Wgs84)
                    }
                }
                "xy" | "crs" => {
                    let from = if req.kind == "xy" {
                        map_crs.ok_or_else(|| anyhow!("This map has no coordinate system yet, so map x and y have no meaning. Set the map's coordinate system first (Map, Coordinate System), or type the position as latitude and longitude."))?.to_string()
                    } else {
                        if req.crs.trim().is_empty() {
                            bail!("Choose the coordinate system the x and y are in.");
                        }
                        req.crs.trim().to_string()
                    };
                    crs::validate(conn, &from)?;
                    let (x, y) = (number(&req.x, "x (easting)")?, number(&req.y, "y (northing)")?);
                    let info = crs::info(&from)?;
                    let (lo, la) = crs::transform_point(conn, x, y, &from, crs::WGS84).map_err(|e| anyhow!("{x}, {y} can't be converted from {}: {e}", info.name))?;
                    if !(lo.is_finite() && la.is_finite()) || la.abs() > 90.0 || lo.abs() > 360.0 {
                        bail!("{x}, {y} is not a place on Earth in {}. Check the numbers and the coordinate system.", info.name);
                    }
                    let note = crs::datum_change_note(&from, crs::WGS84).unwrap_or_default();
                    let unit = if info.geographic { "degrees" } else { "metres" };
                    (lo, la, format!("Read as x {x}, y {y} ({unit}) in {} ({}), moved to WGS 84 by an explicit PROJ transform. {note}", info.name, info.code.clone().unwrap_or_else(|| "custom".into())).trim().to_string(), Some((x, y, from)), usng::Datum::Wgs84)
                }
                "utm" => {
                    let zone: u8 = req.zone.trim().parse().ok().filter(|z| (1..=60).contains(z)).ok_or_else(|| anyhow!("'{}' is not a UTM zone: zones are the whole numbers 1 to 60.", req.zone.trim()))?;
                    let north = match req.hemisphere.trim().to_uppercase().chars().next() {
                        Some('N') => true,
                        Some('S') => false,
                        _ => bail!("Choose the hemisphere: north (N) or south (S)."),
                    };
                    let nad = match req.datum.to_lowercase().as_str() {
                        "" | "wgs84" => false,
                        "nad83" => true,
                        o => bail!("Unknown datum '{o}' (WGS 84 or NAD 83)."),
                    };
                    if nad && !(north && zone <= 23) {
                        bail!("NAD 83 UTM exists only for zones 1 to 23 north of the equator. Choose WGS 84, or another zone.");
                    }
                    let (e, n) = (number(&req.easting, "easting")?, number(&req.northing, "northing")?);
                    if !(100_000.0..=900_000.0).contains(&e) {
                        bail!("A UTM easting runs from about 100 000 to 900 000 m; {e} is outside that. Check the zone and the numbers.");
                    }
                    let (lo_n, hi_n) = if north { (0.0, 9_400_000.0) } else { (1_000_000.0, 10_000_000.0) };
                    if !(lo_n..=hi_n).contains(&n) {
                        bail!("In the {} hemisphere a UTM northing runs from {lo_n} to {hi_n} m; {n} is outside that.", if north { "northern" } else { "southern" });
                    }
                    let datum = if nad { usng::Datum::Nad83 } else { usng::Datum::Wgs84 };
                    let code = usng::utm_crs(zone, north, datum);
                    let (lo, la) = crs::transform_point(conn, e, n, &code, crs::WGS84)?;
                    let name = crs::info(&code).map(|i| i.name).unwrap_or_else(|_| code.clone());
                    (lo, la, format!("Read as UTM zone {zone}{}, easting {e} m, northing {n} m, on {name} ({code}), moved to WGS 84 by an explicit PROJ transform.", if north { "N" } else { "S" }), Some((e, n, code)), datum)
                }
                "usng" => {
                    if req.text.trim().is_empty() {
                        bail!("Type a USNG / MGRS reference, for example 15T VG 97470 47556.");
                    }
                    let datum = usng::Datum::parse(&req.datum)?;
                    let (lo, la, cell, code) = usng_to_lonlat(conn, &req.text, datum)?;
                    let name = crs::info(&code).map(|i| i.name).unwrap_or_else(|_| code.clone());
                    let size = if cell >= 1000.0 { format!("{} km", cell / 1000.0) } else { format!("{cell} m") };
                    (lo, la, format!("Read as the grid reference {}: the centre of its {size} square, on {name} ({code}; datum rule: {}).", req.text.trim().to_uppercase(), datum_label(datum)), None, datum)
                }
                other => bail!("Unknown kind of position '{other}' (latlon, xy, utm, usng or crs)."),
            };

            // Where it goes on the map.
            let (mut map_xy, mut map_message, mut map_name) = (None, None, None);
            match map_crs {
                None => map_message = Some("This map has no coordinate system yet. Set the map's coordinate system first (Map, Coordinate System); the position is read below and can be copied.".to_string()),
                Some(m) => {
                    map_name = crs::info(m).map(|i| i.name).ok();
                    match shown_on_map(conn, m, lon, lat) {
                        // The input was already in the map's CRS: keep its own numbers rather than a round trip through WGS 84.
                        Ok(xy) => map_xy = Some(match &native { Some((x, y, c)) if crs::same(c, m) => (*x, *y), _ => xy }),
                        Err(msg) => map_message = Some(msg),
                    }
                }
            }

            let mut formats = vec![
                LocationFormat { id: "dd".into(), label: "Latitude, longitude, decimal degrees (WGS 84)".into(), text: format!("{lat:.6}, {lon:.6}") },
                LocationFormat { id: "dms".into(), label: "Latitude, longitude, degrees minutes seconds (WGS 84)".into(), text: format!("{} {}", dms(lat, 'N', 'S'), dms(lon, 'E', 'W')) },
            ];
            let (lo83, la83) = crs::transform_point(conn, lon, lat, crs::WGS84, "EPSG:4269")?;
            formats.push(LocationFormat { id: "nad83".into(), label: "Latitude, longitude, decimal degrees (NAD 83)".into(), text: format!("{la83:.6}, {lo83:.6}") });
            formats.push(LocationFormat { id: "utm".into(), label: "UTM (WGS 84)".into(), text: utm_text(conn, lon, lat, usng::Datum::Wgs84)? });
            let none = || "Not available here (beyond the grid's limits)".to_string();
            formats.push(LocationFormat { id: "mgrs".into(), label: "MGRS (WGS 84)".into(), text: grid_ref(conn, lon, lat, usng::Style::Mgrs, usng::Datum::Wgs84)?.unwrap_or_else(none) });
            let ud = if req.kind == "usng" { usng_datum } else { usng::Datum::Auto };
            formats.push(LocationFormat { id: "usng".into(), label: format!("USNG ({})", datum_label(ud)), text: grid_ref(conn, lon, lat, usng::Style::Usng, ud)?.unwrap_or_else(none) });
            if let (Some((x, y)), Some(name)) = (map_xy, &map_name) {
                formats.push(LocationFormat { id: "map".into(), label: format!("Map coordinates ({name})"), text: format!("{x:.3}, {y:.3}") });
            }
            Ok(LocationResult { lon, lat, map_x: map_xy.map(|p| p.0), map_y: map_xy.map(|p| p.1), map_crs: map_crs.map(String::from), map_message, reading, formats })
        })
    }
}
