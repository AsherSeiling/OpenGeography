//! USNG (United States National Grid) and MGRS (Military Grid Reference System) references.
//!
//! Both are the same grid: UTM zone + latitude band, a two-letter 100 km square and an easting/northing inside it. They differ only in
//! how they are written: USNG puts spaces in ("15T VG 97470 47556"), MGRS runs them together ("15TVG9747047556"). The letters are the
//! modern (WGS 84) lettering, and the datum is stated by the caller (NAD 83 for North America by default, as USNG specifies; WGS 84
//! elsewhere). The grid covers 80°S–84°N; the polar (UPS) grids are not offered, and the tools say so rather than guess.
//! Written from the grid's published description (zone, band, column and row letter sets, the even-zone row shift, the Norway and
//! Svalbard zone exceptions) and checked against the MIT `mgrs` package the status bar uses (tests below). No third-party code.

use crate::db::{ident, lit};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;

const BANDS: &str = "CDEFGHJKLMNPQRSTUVWX";
const COLUMN_SETS: [&str; 3] = ["ABCDEFGH", "JKLMNPQR", "STUVWXYZ"];
const ROW_LETTERS: &str = "ABCDEFGHJKLMNPQRSTUV";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// "15T VG 97470 47556"
    Usng,
    /// "15TVG9747047556"
    Mgrs,
}

impl Style {
    pub fn parse(s: &str) -> Result<Style> {
        match s.to_lowercase().as_str() {
            "usng" | "" => Ok(Style::Usng),
            "mgrs" => Ok(Style::Mgrs),
            o => bail!("Unknown grid reference style '{o}' (usng or mgrs)"),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Style::Usng => "USNG",
            Style::Mgrs => "MGRS",
        }
    }
}

/// The datum a reference is computed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Datum {
    /// NAD 83 in zones 1–23 north of the equator, WGS 84 elsewhere (USNG's rule).
    Auto,
    Nad83,
    Wgs84,
}

impl Datum {
    pub fn parse(s: &str) -> Result<Datum> {
        match s.to_lowercase().as_str() {
            "auto" | "" => Ok(Datum::Auto),
            "nad83" => Ok(Datum::Nad83),
            "wgs84" => Ok(Datum::Wgs84),
            o => bail!("Unknown datum '{o}' (auto, nad83 or wgs84)"),
        }
    }
}

/// The UTM CRS of a zone on the chosen datum.
pub fn utm_crs(zone: u8, north: bool, datum: Datum) -> String {
    let nad = match datum {
        Datum::Nad83 => true,
        Datum::Auto => north && zone <= 23,
        Datum::Wgs84 => false,
    };
    if nad && north && zone <= 23 {
        format!("EPSG:269{zone:02}")
    } else {
        format!("EPSG:{}{zone:02}", if north { 326 } else { 327 })
    }
}

/// The UTM zone of a place, with the Norway and Svalbard exceptions.
pub fn zone_of(lon: f64, lat: f64) -> u8 {
    if (56.0..64.0).contains(&lat) && (3.0..12.0).contains(&lon) {
        return 32;
    }
    if (72.0..84.0).contains(&lat) {
        for (a, b, z) in [(0.0, 9.0, 31), (9.0, 21.0, 33), (21.0, 33.0, 35), (33.0, 42.0, 37)] {
            if lon >= a && lon < b {
                return z;
            }
        }
    }
    ((((lon + 180.0) / 6.0).floor() as i64).clamp(0, 59) + 1) as u8
}

/// The same rule as [`zone_of`], as SQL over lon and lat expressions.
fn zone_sql(lon: &str, lat: &str) -> String {
    format!(
        "CASE WHEN {lat} >= 56 AND {lat} < 64 AND {lon} >= 3 AND {lon} < 12 THEN 32 \
         WHEN {lat} >= 72 AND {lat} < 84 AND {lon} >= 0 AND {lon} < 9 THEN 31 \
         WHEN {lat} >= 72 AND {lat} < 84 AND {lon} >= 9 AND {lon} < 21 THEN 33 \
         WHEN {lat} >= 72 AND {lat} < 84 AND {lon} >= 21 AND {lon} < 33 THEN 35 \
         WHEN {lat} >= 72 AND {lat} < 84 AND {lon} >= 33 AND {lon} < 42 THEN 37 \
         ELSE least(60, greatest(1, CAST(floor(({lon} + 180) / 6) AS INTEGER) + 1)) END"
    )
}

pub fn band_of(lat: f64) -> Option<char> {
    if !(-80.0..=84.0).contains(&lat) {
        return None;
    }
    BANDS.chars().nth((((lat + 80.0) / 8.0).floor() as usize).min(BANDS.len() - 1))
}

/// "15T VG" letters of a UTM point (`northing` as in the zone's CRS, including the 10 000 000 m southern offset).
pub fn square_of(zone: u8, easting: f64, northing: f64) -> Option<String> {
    let col = (easting / 100_000.0).floor() as i64;
    if !(1..=8).contains(&col) {
        return None;
    }
    let set = COLUMN_SETS[(zone as usize - 1) % 3];
    let row = ((northing / 100_000.0).floor() as i64 + if zone % 2 == 0 { 5 } else { 0 }).rem_euclid(20) as usize;
    Some(format!("{}{}", set.as_bytes()[col as usize - 1] as char, ROW_LETTERS.as_bytes()[row] as char))
}

/// The UTM system of a place for USNG/MGRS work.
#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct UsngCrs {
    pub id: String,
    pub name: String,
    pub zone: u8,
    pub north: bool,
}

/// The UTM CRS at a place (80°S–84°N), named from the CRS catalogue.
pub fn crs_at(lon: f64, lat: f64, datum: Datum) -> Result<UsngCrs> {
    if !(lon.is_finite() && lat.is_finite()) || !(-80.0..=84.0).contains(&lat) || lon.abs() > 180.0 {
        bail!("USNG and MGRS cover 80°S to 84°N; there is no grid at latitude {lat:.1}°");
    }
    let (zone, north) = (zone_of(lon, lat), lat >= 0.0);
    let id = utm_crs(zone, north, datum);
    let name = crate::crs::info(&id).map(|i| i.name).unwrap_or_else(|_| id.clone());
    Ok(UsngCrs { id, name, zone, north })
}

/// A reference written for a UTM point.
pub fn format_ref(zone: u8, lat: f64, easting: f64, northing: f64, precision: u8, style: Style) -> Option<String> {
    let band = band_of(lat)?;
    let sq = square_of(zone, easting, northing)?;
    let p = precision.min(5) as u32;
    let digits = |v: f64| {
        let q = ((v.rem_euclid(100_000.0)) / 10f64.powi(5 - p as i32)).floor() as i64;
        if p == 0 { String::new() } else { format!("{q:0width$}", width = p as usize) }
    };
    let (e, n) = (digits(easting), digits(northing));
    Some(match style {
        Style::Usng if p == 0 => format!("{zone}{band} {sq}"),
        Style::Usng => format!("{zone}{band} {sq} {e} {n}"),
        Style::Mgrs => format!("{zone}{band}{sq}{e}{n}"),
    })
}

/// A parsed reference: the grid square and the position inside it (the lower-left corner of the named cell).
#[derive(Debug, Clone, PartialEq)]
pub struct GridRef {
    pub zone: u8,
    pub band: char,
    pub square: String,
    /// Metres east of the square's west edge, and north of its south edge, at the written precision.
    pub easting_in_square: f64,
    pub northing_in_square: f64,
    /// Digits written per axis (0–5); the cell is 10^(5−digits) m wide.
    pub digits: u8,
}

impl GridRef {
    pub fn cell_m(&self) -> f64 {
        10f64.powi(5 - self.digits as i32)
    }
}

/// Read "15T VG 97470 47556", "15TVG9747047556", "15T VG" and the like (case and spacing do not matter).
pub fn parse(text: &str) -> Result<GridRef> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    let bad = || anyhow!("'{}' is not a USNG/MGRS reference (for example 15T VG 97470 47556)", text.trim());
    let zlen = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if !(1..=2).contains(&zlen) {
        return Err(bad());
    }
    let zone: u8 = t[..zlen].parse().map_err(|_| bad())?;
    if !(1..=60).contains(&zone) {
        return Err(anyhow!("'{}' has UTM zone {zone}; zones run from 1 to 60", text.trim()));
    }
    let rest: Vec<char> = t[zlen..].chars().collect();
    if rest.len() < 3 || !BANDS.contains(rest[0]) || !rest[1].is_ascii_alphabetic() || !rest[2].is_ascii_alphabetic() {
        return Err(bad());
    }
    let (band, sq) = (rest[0], format!("{}{}", rest[1], rest[2]));
    let digits: String = rest[3..].iter().collect();
    if digits.len() % 2 != 0 || digits.len() > 10 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow!("'{}' needs the same number of easting and northing digits (0 to 5 each)", text.trim()));
    }
    let n = digits.len() / 2;
    let scale = 10f64.powi(5 - n as i32);
    let (e, nn) = if n == 0 { (0.0, 0.0) } else { (digits[..n].parse::<f64>().unwrap() * scale, digits[n..].parse::<f64>().unwrap() * scale) };
    // The letters must belong to this zone's sets.
    let set = COLUMN_SETS[(zone as usize - 1) % 3];
    if !set.contains(rest[1]) || !ROW_LETTERS.contains(rest[2]) {
        return Err(anyhow!("'{}': the square letters {sq} do not exist in zone {zone}", text.trim()));
    }
    Ok(GridRef { zone, band, square: sq, easting_in_square: e, northing_in_square: nn, digits: n as u8 })
}

/// The latitude band's south and north edges.
pub fn band_range(band: char) -> (f64, f64) {
    let i = BANDS.find(band).unwrap_or(0) as f64;
    (-80.0 + 8.0 * i, if band == 'X' { 84.0 } else { -72.0 + 8.0 * i })
}

/// UTM easting and northing (zone CRS, southern offset included) of the lower-left corner of the cell a reference names, and whether
/// it is in the northern hemisphere. The 100 km row letters repeat every 2 000 km, so the band decides which repeat is meant.
pub fn to_utm(r: &GridRef, to_lonlat: &dyn Fn(f64, f64, bool) -> Result<(f64, f64)>) -> Result<(f64, f64, bool)> {
    let set = COLUMN_SETS[(r.zone as usize - 1) % 3];
    let col = set.find(r.square.chars().next().unwrap()).unwrap() as f64 + 1.0;
    let row = ROW_LETTERS.find(r.square.chars().nth(1).unwrap()).unwrap() as i64;
    let row100 = (row - if r.zone % 2 == 0 { 5 } else { 0 }).rem_euclid(20) as f64;
    let easting = col * 100_000.0 + r.easting_in_square;
    let (south, north_edge) = band_range(r.band);
    let north = r.band >= 'N';
    // Candidates 2 000 km apart; keep the one that lands in the band.
    for k in 0..6 {
        let n = row100 * 100_000.0 + k as f64 * 2_000_000.0 + r.northing_in_square;
        let utm_n = n;
        if let Ok((_, lat)) = to_lonlat(easting, utm_n, north) {
            if lat >= south - 0.05 && lat <= north_edge + 0.05 {
                return Ok((easting, utm_n, north));
            }
        }
    }
    bail!("'{}{}{}' does not fall inside latitude band {}", r.zone, r.band, r.square, r.band)
}

/// SQL that adds a USNG/MGRS text column to `table`. `pt` is a point expression over the alias `t` (`t.geom`, or
/// `ST_PointOnSurface(t.geom)` for lines and polygons) in `src_crs`. A row with no geometry, or outside 80°S–84°N, gets NULL; the
/// second value is how many rows that is. The UTM zones in use are looked up first (`conn`) so the SQL only names those; with `force`
/// (zone, north) every row is put in that one zone and nothing is looked up (the table may not exist yet).
pub fn add_field_sql(conn: Option<&Connection>, table: &str, src_crs: &str, pt: &str, field: &str, precision: u8, style: Style, datum: Datum, force: Option<(u8, bool)>) -> Result<(String, i64)> {
    let src = lit(&crate::crs::resolve(src_crs));
    let zone = zone_sql("__lon", "__lat");
    // Everything up to the zone, shared by the lookup and the final query.
    let upto_zone = format!(
        "WITH s AS (SELECT t.*, {pt} AS __pt FROM {t} t), \
         l AS (SELECT s.*, ST_Transform(__pt, {src}, 'EPSG:4326', true) AS __ll FROM s), \
         g AS (SELECT l.*, ST_X(__ll) AS __lon, ST_Y(__ll) AS __lat FROM l), \
         y AS (SELECT g.*, CASE WHEN __lat BETWEEN -80 AND 84 THEN {zone} END AS __z FROM g)",
        t = if table.contains('"') { table.to_string() } else { ident(table) },
        zone = match force {
            Some((z, _)) => z.to_string(),
            None => zone,
        }
    );
    let (pairs, skipped): (Vec<(i32, bool)>, i64) = match (force, conn) {
        (Some((z, n)), _) => (vec![(z as i32, n)], 0),
        (None, Some(conn)) => {
            let mut st = conn.prepare(&format!("{upto_zone} SELECT DISTINCT __z, __lat >= 0 FROM y WHERE __z IS NOT NULL"))?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, bool>(1)?)))?;
            let pairs: Vec<(i32, bool)> = rows.collect::<std::result::Result<_, _>>()?;
            let skipped: i64 = conn.query_row(&format!("{upto_zone} SELECT count(*) FROM y WHERE __z IS NULL"), [], |r| r.get(0))?;
            (pairs, skipped)
        }
        (None, None) => bail!("a connection is needed to look up the UTM zones"),
    };
    if pairs.is_empty() {
        // Nothing can be written: a column of NULLs, so the tool still reports why.
        return Ok((format!("SELECT t.*, CAST(NULL AS VARCHAR) AS {} FROM {} t", ident(field), if table.contains('"') { table.to_string() } else { ident(table) }), skipped));
    }
    let branch = |axis: &str| -> String {
        let mut s = String::from("CASE ");
        for (z, n) in &pairs {
            let crs = utm_crs(*z as u8, *n, datum);
            s.push_str(&format!("WHEN __z = {z} AND (__lat >= 0) = {} THEN {axis}(ST_Transform(__pt, {src}, {}, true)) ", if *n { "TRUE" } else { "FALSE" }, lit(&crs)));
        }
        s.push_str("END");
        s
    };
    let p = precision.min(5) as i32;
    let pow = 10f64.powi(5 - p);
    let band = format!("substr('{BANDS}', least(19, CAST(floor((__lat + 80) / 8) AS INTEGER)) + 1, 1)");
    let col = format!(
        "substr(CASE ((__z - 1) % 3) WHEN 0 THEN '{}' WHEN 1 THEN '{}' ELSE '{}' END, CAST(floor(__e / 100000) AS INTEGER), 1)",
        COLUMN_SETS[0], COLUMN_SETS[1], COLUMN_SETS[2]
    );
    let row = format!("substr('{ROW_LETTERS}', ((CAST(floor(__n / 100000) AS BIGINT) + CASE WHEN __z % 2 = 0 THEN 5 ELSE 0 END) % 20) + 1, 1)");
    let digits = |v: &str| if p == 0 { "''".to_string() } else { format!("lpad(CAST(CAST(floor(({v} % 100000) / {pow}) AS BIGINT) AS VARCHAR), {p}, '0')") };
    let (de, dn) = (digits("__e"), digits("__n"));
    let label = match style {
        Style::Usng if p == 0 => format!("CAST(__z AS VARCHAR) || {band} || ' ' || {col} || {row}"),
        Style::Usng => format!("CAST(__z AS VARCHAR) || {band} || ' ' || {col} || {row} || ' ' || {de} || ' ' || {dn}"),
        Style::Mgrs => format!("CAST(__z AS VARCHAR) || {band} || {col} || {row} || {de} || {dn}"),
    };
    let sql = format!(
        "{upto_zone}, c AS (SELECT y.*, {e} AS __e, {n} AS __n FROM y) \
         SELECT * EXCLUDE (__pt, __ll, __lon, __lat, __z, __e, __n), \
                CASE WHEN __e IS NULL OR __e < 100000 OR __e >= 900000 THEN NULL ELSE {label} END AS {f} FROM c",
        e = branch("ST_X"),
        n = branch("ST_Y"),
        f = ident(field),
    );
    Ok((sql, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_bands_and_squares() {
        assert_eq!(zone_of(-93.0, 45.0), 15);
        assert_eq!(zone_of(10.75, 59.91), 32, "Norway's widened zone 32");
        assert_eq!(zone_of(15.6, 78.22), 33, "Svalbard");
        assert_eq!(zone_of(-177.0, 0.0), 1);
        assert_eq!(band_of(45.0), Some('T'));
        assert_eq!(band_of(-33.4), Some('H'));
        assert_eq!(band_of(84.0), Some('X'));
        assert_eq!(band_of(-81.0), None);
        assert_eq!(square_of(15, 500_000.0, 4_982_950.0).as_deref(), Some("WK"));
        assert_eq!(square_of(15, 499_976.0, 4_982_994.0).as_deref(), Some("VK"));
        assert_eq!(square_of(15, 99_000.0, 4_982_994.0), None, "east of 100 km only");
    }

    #[test]
    fn writes_usng_with_spaces_and_mgrs_without() {
        // Values from the mgrs package for 45.0004 N, 93.0003 W (WGS 84 UTM 15N 499 976 E, 4 982 994 N).
        assert_eq!(format_ref(15, 45.0004, 499_976.0, 4_982_994.0, 5, Style::Mgrs).unwrap(), "15TVK9997682994");
        assert_eq!(format_ref(15, 45.0004, 499_976.0, 4_982_994.0, 5, Style::Usng).unwrap(), "15T VK 99976 82994");
        assert_eq!(format_ref(15, 45.0004, 499_976.0, 4_982_994.0, 3, Style::Usng).unwrap(), "15T VK 999 829");
        assert_eq!(format_ref(15, 45.0004, 499_976.0, 4_982_994.0, 0, Style::Usng).unwrap(), "15T VK");
        assert_eq!(format_ref(15, 45.0004, 499_976.0, 4_982_994.0, 0, Style::Mgrs).unwrap(), "15TVK");
        assert!(format_ref(15, 85.0, 499_976.0, 4_982_994.0, 5, Style::Usng).is_none(), "no polar grid");
    }

    #[test]
    fn the_utm_system_of_a_place_by_datum_and_the_polar_refusal() {
        let usng = crs_at(-97.5, 35.2, Datum::Auto).unwrap();
        assert_eq!((usng.id.as_str(), usng.zone, usng.north), ("EPSG:26914", 14, true));
        assert!(usng.name.contains("NAD83") && usng.name.contains("zone 14N"), "{}", usng.name);
        let mgrs = crs_at(-97.5, 35.2, Datum::Wgs84).unwrap();
        assert_eq!(mgrs.id, "EPSG:32614");
        assert!(mgrs.name.contains("WGS 84"), "{}", mgrs.name);
        assert_eq!(crs_at(10.75, 59.91, Datum::Auto).unwrap().id, "EPSG:32632", "Europe: WGS 84, Norway's zone 32");
        assert_eq!(crs_at(151.2, -33.9, Datum::Auto).unwrap().id, "EPSG:32756", "southern hemisphere");
        assert!(crs_at(0.0, 85.0, Datum::Auto).unwrap_err().to_string().contains("80°S to 84°N"));
    }

    #[test]
    fn reads_references_in_any_spacing_and_refuses_bad_ones_in_words() {
        let r = parse("15t vk 99976 82994").unwrap();
        assert_eq!((r.zone, r.band, r.square.as_str(), r.digits), (15, 'T', "VK", 5));
        assert_eq!((r.easting_in_square, r.northing_in_square), (99_976.0, 82_994.0));
        assert_eq!(parse("15TVK9997682994").unwrap(), r);
        let c = parse("15TVK9982").unwrap();
        assert_eq!((c.digits, c.easting_in_square, c.northing_in_square, c.cell_m()), (2, 99_000.0, 82_000.0, 1000.0));
        assert_eq!(parse("15TVK").unwrap().digits, 0);
        for (bad, needle) in [("hello", "not a USNG/MGRS"), ("61TVK", "zones run from 1 to 60"), ("15TVK123", "same number"), ("15TZZ1212", "do not exist in zone 15"), ("15IVK12", "not a USNG/MGRS")] {
            let e = parse(bad).unwrap_err().to_string();
            assert!(e.contains(needle), "{bad}: {e}");
        }
    }
}
