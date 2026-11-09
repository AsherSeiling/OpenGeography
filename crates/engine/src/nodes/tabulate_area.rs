//! Tabulate Area (Wave 1, package T4): the area of each class of a whole-number raster inside each zone polygon, as a table.
//!
//! Cell assignment rule: a cell belongs to a zone when its centre lies inside the zone's polygon (even-odd rule, so holes are respected; a
//! centre exactly on the lower or left edge counts inside, on the upper or right edge outside). The area of a cell is its true area: the
//! cell's width times height times the squared linear unit in a projected CRS (planar in that projection, so exact only for an equal-area one),
//! and the authalic area of the longitude/latitude cell on the CRS's ellipsoid in a geographic CRS (Snyder 1987, eq. 3-12), which shrinks
//! towards the poles. The zones are transformed into the raster's CRS explicitly before anything is counted.

use super::*;
use crate::geomkit;
use std::collections::BTreeMap;

pub struct TabulateArea;
impl Tool for TabulateArea {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "tabulate_area", name: "Tabulate Area", category: "Analysis › Summaries", role: Role::Tool,
            description: "How much of each class lies in each zone: for every zone polygon and every class value of a whole-number raster (land cover, soil type), the number of cells, the area in square metres and the share of the zone's mapped area. A cell belongs to the zone its centre falls in. Areas are true cell areas: planar in a projected coordinate system, on the ellipsoid in a geographic one (cells shrink toward the poles). Zones are transformed to the raster's coordinate system first. Cells with no data inside a zone are listed as a class of their own (empty class); zones that cover no cell and cells claimed by more than one zone are reported. Zones with the same value are combined.",
            inputs: vec![Port { id: "zones", label: "Zones (polygons)", kind: PortKind::Vector }, Port { id: "classes", label: "Class raster (whole numbers)", kind: PortKind::Raster }], output: Some(PortKind::Table),
            params: vec![Param::new("zone_field", "Zone field", ParamKind::Field).field_of("zones")] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (zr, cr) = (&inputs["zones"], &inputs["classes"]);
        let zf = p.req_str("zone_field", "Zone field")?;
        let rcrs = need_raster_crs(cr)?;
        let rpath = need_raster(cr)?;
        if !zr.has_geom || ctx.dimension(zr)? != 2 {
            bail!("The zones must be a polygon layer");
        }
        let m = geomkit::metric(rcrs)?;
        let grid = crate::raster_ops::grid_of(std::path::Path::new(rpath))?;
        if grid.gt[2] != 0.0 || grid.gt[4] != 0.0 || grid.gt[5] >= 0.0 {
            bail!("Tabulate Area needs a north-up raster. Use Warp to make this one north-up first.");
        }
        let ds = gdal::Dataset::open(rpath)?;
        let band = ds.rasterband(1)?;
        if matches!(band.band_type(), gdal::raster::GdalDataType::Float32 | gdal::raster::GdalDataType::Float64) {
            bail!("The class raster holds decimal numbers. Tabulate Area needs whole-number classes; use Reclassify to make them first.");
        }
        let (cols, rows) = (grid.cols, grid.rows);
        crate::raster_ops::check_cells(cols as f64, rows as f64)?;
        let nodata: Option<i64> = band.no_data_value().filter(|v| v.is_finite()).map(|v| v as i64);
        let data = band.read_as::<i32>((0, 0), (cols, rows), (cols, rows), None)?;
        let data = data.data();
        // Zones in the raster's CRS, combined per zone value.
        let g = ctx.geom_in(zr, "a", rcrs)?;
        let mut stmt = ctx.conn.prepare(&format!(
            "SELECT z, ST_AsText(ST_Union_Agg(g)) FROM (SELECT {}::VARCHAR AS z, {g} AS g FROM {} a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom)) WHERE z IS NOT NULL AND g IS NOT NULL GROUP BY z ORDER BY z",
            ident(&zf), ident(&zr.table)
        ))?;
        let zones: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))?.collect::<Result<_, _>>()?;
        let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&zr.table)), [], |r| r.get(0))?;
        let used: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {} a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom) AND a.{}::VARCHAR IS NOT NULL", ident(&zr.table), ident(&zf)), [], |r| r.get(0))?;
        if total > used {
            ctx.add_note(format!("Tabulate Area: {} zone polygon(s) without a geometry or a zone value were left out.", total - used));
        }
        if zones.is_empty() {
            bail!("The zone layer has no polygons with a zone value");
        }
        let (dx, dy) = (grid.gt[1], grid.gt[5]);
        let row_area = |r: usize| -> f64 {
            if m.geographic {
                let (lat_a, lat_b) = (grid.gt[3] + r as f64 * dy, grid.gt[3] + (r + 1) as f64 * dy);
                m.band_area(lat_a, lat_b, dx)
            } else {
                (dx * dy).abs() * m.unit_m * m.unit_m
            }
        };
        let task = crate::progress::Task::start("Tabulate area").cancellable();
        let mut claimed = vec![0u8; cols * rows];
        let mut out: Vec<(String, Option<i64>, u64, f64)> = vec![];
        let mut empty_zones: Vec<String> = vec![];
        for (zi, (zid, wkt)) in zones.iter().enumerate() {
            task.check()?;
            task.set(zi as f64 / zones.len() as f64, Some(format!("zone {} of {}", zi + 1, zones.len())));
            let parts = geomkit::parse_wkt(wkt)?;
            let rings: Vec<&Vec<(f64, f64)>> = parts.iter().flatten().collect();
            let (mut ymin, mut ymax) = (f64::MAX, f64::MIN);
            for r in &rings {
                for (_, y) in r.iter() {
                    ymin = ymin.min(*y);
                    ymax = ymax.max(*y);
                }
            }
            let mut acc: BTreeMap<Option<i64>, (u64, f64)> = BTreeMap::new();
            if ymin <= ymax {
                let r0 = (((ymax - grid.gt[3]) / dy).floor() as i64).max(0);
                let r1 = (((ymin - grid.gt[3]) / dy).ceil() as i64).min(rows as i64 - 1);
                for r in r0..=r1 {
                    let y = grid.gt[3] + (r as f64 + 0.5) * dy;
                    let area = row_area(r as usize);
                    for (xa, xb) in geomkit::spans(&rings, y) {
                        // Columns whose centre x satisfies xa <= x < xb.
                        let c0 = (((xa - grid.gt[0]) / dx) - 0.5).ceil().max(0.0) as i64;
                        let c1 = ((((xb - grid.gt[0]) / dx) - 0.5).ceil() as i64 - 1).min(cols as i64 - 1);
                        for c in c0..=c1 {
                            let idx = r as usize * cols + c as usize;
                            claimed[idx] = claimed[idx].saturating_add(1);
                            let v = data[idx] as i64;
                            let key = if nodata == Some(v) { None } else { Some(v) };
                            let e = acc.entry(key).or_insert((0, 0.0));
                            e.0 += 1;
                            e.1 += area;
                        }
                    }
                }
            }
            if acc.is_empty() {
                empty_zones.push(zid.clone());
            }
            for (k, (n, a)) in acc {
                out.push((zid.clone(), k, n, a));
            }
        }
        let overlap = claimed.iter().filter(|c| **c > 1).count();
        if overlap > 0 {
            ctx.add_note(format!("Tabulate Area: {overlap} cell(s) lie in more than one zone and are counted in each of them."));
        }
        if !empty_zones.is_empty() {
            ctx.add_note(format!("Tabulate Area: {} zone(s) cover no cell centre of the raster (outside it, or smaller than a cell) and have no rows: {}.", empty_zones.len(), empty_zones.iter().take(10).cloned().collect::<Vec<_>>().join(", ")));
        }
        let nd_cells: u64 = out.iter().filter(|r| r.1.is_none()).map(|r| r.2).sum();
        if nd_cells > 0 {
            ctx.add_note(format!("Tabulate Area: {nd_cells} cell(s) inside the zones have no data; they appear as rows with an empty class and are not part of the percentages."));
        }
        if !m.geographic {
            if let Some(n) = crate::crs_warnings::planar_note("Tabulate Area", rcrs, [grid.gt[0], grid.gt[3] + rows as f64 * dy, grid.gt[0] + cols as f64 * dx, grid.gt[3]]) {
                ctx.add_note(n);
            }
        }
        let mut mapped: BTreeMap<&str, f64> = BTreeMap::new();
        for (z, k, _, a) in &out {
            if k.is_some() {
                *mapped.entry(z.as_str()).or_insert(0.0) += a;
            }
        }
        let rows_sql: Vec<String> = out
            .iter()
            .map(|(z, k, n, a)| {
                let pct = if k.is_some() { mapped.get(z.as_str()).filter(|t| **t > 0.0).map(|t| 100.0 * a / t) } else { None };
                format!("({}, {}, {n}, {a:?}, {})", lit(z), k.map(|v| v.to_string()).unwrap_or_else(|| "NULL".into()), pct.map(|v| format!("{v:?}")).unwrap_or_else(|| "NULL".into()))
            })
            .collect();
        if rows_sql.is_empty() {
            bail!("No zone covers any cell of the raster. Check that the zones and the raster overlap.");
        }
        Ok(Output::Select {
            sql: format!("SELECT * FROM (VALUES {}) t(zone, class, cells, area_m2, percent_of_zone)", rows_sql.join(", ")),
            crs: None,
            view: false,
        })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "tabulate_area" => &["area by class", "land cover by zone", "cross tabulate", "class areas", "zonal area", "area per category"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(TabulateArea)]
}
