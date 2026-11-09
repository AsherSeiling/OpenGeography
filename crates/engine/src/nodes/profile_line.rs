//! Profile Along Line (Wave 1, package T4): elevations sampled at equal geodesic spacing along line features.
//!
//! Stations: the line is taken in longitude/latitude (an explicit PROJ transform of each vertex), and each segment between vertices is followed
//! as a geodesic on the WGS 84 ellipsoid (Karney 2013, `geographiclib-rs`); a station is placed every `spacing` metres of geodesic length from
//! the start of each part of the line, and the last vertex is always a station. Each station is then transformed explicitly into the elevation
//! raster's CRS and sampled: bilinear interpolation between the four nearest cell centres (the usual raster convention that a value belongs to
//! its cell centre; within half a cell of the raster's edge the edge cells are used), or the value of the cell the station falls in. A station
//! outside the raster, or whose cells have no data, gets no elevation and a status saying which. The result is a point layer in the line's CRS.

use super::centrography::sqlnum;
use super::*;
use crate::geomkit;
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};

const MAX_STATIONS: usize = 300_000;

/// Elevation of (x, y) in a raster's CRS: Ok(Some(z)), or Err(status) when it cannot be given.
struct Sampler {
    ds: gdal::Dataset,
    gt: [f64; 6],
    cols: isize,
    rows: isize,
    nodata: Option<f64>,
    bilinear: bool,
}

impl Sampler {
    fn cell(&self, c: isize, r: isize) -> Result<Option<f64>> {
        let b = self.ds.rasterband(1)?;
        let v = b.read_as::<f64>((c, r), (1, 1), (1, 1), None)?.data()[0];
        Ok((v.is_finite() && self.nodata != Some(v)).then_some(v))
    }
    /// Err(Ok(status)) is a normal "no value" outcome; Err(Err(e)) an I/O failure.
    fn at(&self, x: f64, y: f64) -> Result<std::result::Result<f64, &'static str>> {
        let (u, v) = ((x - self.gt[0]) / self.gt[1], (y - self.gt[3]) / self.gt[5]);
        if !(u >= 0.0 && v >= 0.0 && u < self.cols as f64 && v < self.rows as f64) {
            return Ok(Err("outside the raster"));
        }
        if !self.bilinear {
            return Ok(self.cell(u.floor() as isize, v.floor() as isize)?.ok_or("no data"));
        }
        // Cell centres sit at integer + 0.5.
        let (fu, fv) = (u - 0.5, v - 0.5);
        let (c0, r0) = (fu.floor(), fv.floor());
        let (tx, ty) = (fu - c0, fv - r0);
        let clamp = |c: f64, hi: isize| (c as isize).clamp(0, hi - 1);
        let (ca, cb, ra, rb) = (clamp(c0, self.cols), clamp(c0 + 1.0, self.cols), clamp(r0, self.rows), clamp(r0 + 1.0, self.rows));
        let q = [self.cell(ca, ra)?, self.cell(cb, ra)?, self.cell(ca, rb)?, self.cell(cb, rb)?];
        let [Some(z00), Some(z10), Some(z01), Some(z11)] = q else { return Ok(Err("no data")) };
        Ok(Ok(z00 * (1.0 - tx) * (1.0 - ty) + z10 * tx * (1.0 - ty) + z01 * (1.0 - tx) * ty + z11 * tx * ty))
    }
}

pub struct ProfileLine;
impl Tool for ProfileLine {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "profile_line", name: "Profile Along Line", category: "Raster › Terrain and Contours", role: Role::Tool,
            description: "Elevation along line features: points at equal spacing measured along the line on the ellipsoid (geodesic, in metres, from the start of the line), each with the elevation read from a raster. Line vertices and stations are transformed into the raster's coordinate system explicitly. Elevation is bilinear (between the four nearest cell centres) or the value of the cell the point falls in. A point outside the raster or on no-data cells has no elevation and a status saying why; the result reports how many. The last vertex is always a point. Output is a point layer in the line's coordinate system with line_id (row number), part, dist_m (distance along the line), elevation (in the raster's height unit), status, lon and lat; chart dist_m against elevation for the profile.",
            inputs: vec![Port { id: "in", label: "Lines", kind: PortKind::Vector }, Port { id: "dem", label: "Elevation raster", kind: PortKind::Raster }], output: Some(PortKind::Vector),
            params: vec![
                Param::new("spacing", "Spacing along the line (metres)", ParamKind::Number).default(30),
                Param::new("method", "Sampling", ParamKind::Enum).default("bilinear").options(&[("bilinear", "Bilinear"), ("nearest", "Nearest cell")]),
            ] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&[])
            .add(NewField::new("line_id", "BIGINT"))
            .add(NewField::new("part", "INTEGER"))
            .add(NewField::new("dist_m", "DOUBLE"))
            .add(NewField::new("elevation", "DOUBLE"))
            .add(NewField::new("status", "VARCHAR"))
            .add(NewField::new("lon", "DOUBLE"))
            .add(NewField::new("lat", "DOUBLE"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (lines, dem) = (&inputs["in"], &inputs["dem"]);
        let lcrs = need_crs(lines)?;
        let dcrs = need_raster_crs(dem)?;
        let dpath = need_raster(dem)?;
        if !lines.has_geom || ctx.dimension(lines)? != 1 {
            bail!("Profile Along Line needs a line layer");
        }
        let spacing = p.f64("spacing").unwrap_or(f64::NAN);
        if !(spacing.is_finite() && spacing > 0.0) {
            bail!("'Spacing' must be a number greater than 0 (metres)");
        }
        let grid = crate::raster_ops::grid_of(std::path::Path::new(dpath))?;
        if grid.gt[2] != 0.0 || grid.gt[4] != 0.0 || grid.gt[5] >= 0.0 {
            bail!("Profile Along Line needs a north-up raster. Use Warp to make it north-up first.");
        }
        let ds = gdal::Dataset::open(dpath)?;
        let nodata = ds.rasterband(1)?.no_data_value();
        let sampler = Sampler { ds, gt: grid.gt, cols: grid.cols as isize, rows: grid.rows as isize, nodata, bilinear: p.str("method").as_deref() != Some("nearest") };
        // Lines as longitude/latitude (explicit transform; the datum change is noted by `geom_in`).
        let g = ctx.geom_in(lines, "a", "EPSG:4326")?;
        let mut stmt = ctx.conn.prepare(&format!("SELECT rn, ST_AsText(g) FROM (SELECT rn, {g} AS g FROM (SELECT row_number() OVER () AS rn, geom FROM {}) a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom)) WHERE g IS NOT NULL", ident(&lines.table)))?;
        let rows: Vec<(i64, String)> = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?.collect::<Result<_, _>>()?;
        let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&lines.table)), [], |r| r.get(0))?;
        if total > rows.len() as i64 {
            ctx.add_note(format!("Profile Along Line: {} feature(s) without a usable line were left out.", total - rows.len() as i64));
        }
        if rows.is_empty() {
            bail!("The line layer has no usable lines");
        }
        let geod = Geodesic::wgs84();
        let to_dem = geomkit::tx("EPSG:4326", dcrs)?;
        let to_lines = geomkit::tx("EPSG:4326", lcrs)?;
        if let Some(n) = crate::crs::datum_change_note("EPSG:4326", dcrs) {
            ctx.add_note(n);
        }
        let task = crate::progress::Task::start("Profile along line").cancellable();
        let (mut out_rows, mut outside, mut nod, mut unplaced) = (vec![], 0usize, 0usize, 0usize);
        for (li, (rn, wkt)) in rows.iter().enumerate() {
            task.check()?;
            task.set(li as f64 / rows.len() as f64, None::<String>);
            for (pi, part) in geomkit::parse_wkt(wkt)?.into_iter().enumerate() {
                let Some(line) = part.into_iter().next() else { continue };
                if line.len() < 2 {
                    continue;
                }
                // Stations as (distance along, lon, lat).
                let mut st: Vec<(f64, f64, f64)> = vec![(0.0, line[0].0, line[0].1)];
                let (mut done, mut next) = (0.0f64, spacing);
                for w in line.windows(2) {
                    let ((lo1, la1), (lo2, la2)) = (w[0], w[1]);
                    let (s12, az1, _az2, _a12): (f64, f64, f64, f64) = geod.inverse(la1, lo1, la2, lo2);
                    while next <= done + s12 + 1e-9 {
                        let s = (next - done).min(s12);
                        let (la, lo): (f64, f64) = geod.direct(la1, lo1, az1, s);
                        st.push((next, lo, la));
                        next += spacing;
                        if st.len() > MAX_STATIONS {
                            bail!("That would make more than {MAX_STATIONS} points. Use a larger spacing or fewer lines.");
                        }
                    }
                    done += s12;
                }
                let last = line[line.len() - 1];
                if (done - st.last().unwrap().0).abs() > 1e-6 {
                    st.push((done, last.0, last.1));
                }
                for (d, lo, la) in st {
                    let (Some((lx, ly)), Some((dx, dy))) = (to_lines.pt(lo, la), to_dem.pt(lo, la)) else {
                        unplaced += 1;
                        continue;
                    };
                    let (z, status) = match sampler.at(dx, dy)? {
                        Ok(z) => (Some(z), "ok"),
                        Err(why) => {
                            if why == "no data" { nod += 1 } else { outside += 1 }
                            (None, why)
                        }
                    };
                    out_rows.push(format!("({rn}, {pi}, {}, {}, '{status}', {}, {}, ST_Point({lx:?}, {ly:?}))", sqlnum(d), z.map(sqlnum).unwrap_or_else(|| "NULL".into()), sqlnum(lo), sqlnum(la)));
                }
            }
        }
        if out_rows.is_empty() {
            bail!("No point of the profile could be placed. Check the coordinate systems of the lines and the raster.");
        }
        if outside > 0 {
            ctx.add_note(format!("Profile Along Line: {outside} of {} points are outside the raster and have no elevation.", out_rows.len()));
        }
        if nod > 0 {
            ctx.add_note(format!("Profile Along Line: {nod} of {} points fall on cells with no data and have no elevation.", out_rows.len()));
        }
        if unplaced > 0 {
            ctx.add_note(format!("Profile Along Line: {unplaced} point(s) could not be transformed between the coordinate systems and were left out."));
        }
        Ok(Output::Select {
            sql: format!("SELECT line_id, part, dist_m, elevation, status, lon, lat, geom FROM (VALUES {}) t(line_id, part, dist_m, elevation, status, lon, lat, geom)", out_rows.join(", ")),
            crs: lines.crs.clone(),
            view: false,
        })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "profile_line" => &["elevation profile", "cross section", "transect", "terrain profile", "line of sight profile", "height along route"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(ProfileLine)]
}
