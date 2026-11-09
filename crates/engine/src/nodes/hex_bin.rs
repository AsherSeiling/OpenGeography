//! Hex Bins (Wave 1, package T6): count, sum and mean of points per hexagonal cell of the H3 grid (Uber's hexagonal hierarchical spatial index,
//! `h3o` crate, BSD-3-Clause, a Rust port of H3, Apache-2.0). Points are transformed explicitly to longitude/latitude (WGS 84) to find their
//! cell; the cell outlines are transformed back to the layer's coordinate system. The area of each hexagon is the geodesic area of its
//! outline on the WGS 84 ellipsoid (edges as geodesics, Karney 2013), reported in the table because H3 cells vary in area with latitude and
//! the 12 pentagons are smaller than the hexagons.

use super::centrography::sqlnum;
use super::*;
use crate::geomkit;
use geographiclib_rs::{Geodesic, PolygonArea, Winding};
use h3o::{LatLng, Resolution};
use std::collections::HashMap;

pub struct HexBins;
impl Tool for HexBins {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "hex_bins", name: "Hex Bins (H3)", category: "Vector › Sampling", role: Role::Tool,
            description: "Summarise points on a grid of hexagons: for every H3 cell (Uber's hexagonal global grid) that holds points, the number of points and, with a number field, the sum and the mean. The result is the hexagons as polygons in the layer's coordinate system, with the H3 cell id, the count, sum, mean and the cell's geodesic area in square metres on the WGS 84 ellipsoid. The resolution is 0 (about 4 million km2 per cell) to 15 (about 0.9 m2); 5 is about 250 km2, 7 about 5 km2, 9 about 0.1 km2. Points are transformed to longitude/latitude to find their cell, whatever the layer's coordinate system. Hexagons with no points are not drawn. Points without a position, or without a number when a field is chosen, are left out of the sum and mean and counted.",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }], output: Some(PortKind::Vector),
            params: vec![
                Param::new("resolution", "H3 resolution (0 to 15)", ParamKind::Number).default(7),
                Param::new("field", "Value field (for sum and mean)", ParamKind::Field).field_of("in").optional(),
            ] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("h3", "VARCHAR")).add(NewField::new("count", "BIGINT")).add(NewField::new("sum", "DOUBLE")).add(NewField::new("mean", "DOUBLE")).add(NewField::new("values_used", "BIGINT")).add(NewField::new("area_m2", "DOUBLE"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?;
        let res = p.f64("resolution").unwrap_or(f64::NAN);
        if !(res.is_finite() && res.fract() == 0.0 && (0.0..=15.0).contains(&res)) {
            bail!("'H3 resolution' must be a whole number from 0 to 15");
        }
        let resolution = Resolution::try_from(res as u8).map_err(|_| anyhow!("'H3 resolution' must be from 0 to 15"))?;
        if !i.has_geom {
            bail!("Hex Bins needs a layer with geometry");
        }
        let g = ctx.geom_in(i, "a", "EPSG:4326")?;
        let val = p.str("field").map(|f| format!("TRY_CAST(a.{}::VARCHAR AS DOUBLE)", ident(&f))).unwrap_or_else(|| "NULL".into());
        let mut stmt = ctx.conn.prepare(&format!(
            "SELECT ST_X(c), ST_Y(c), v FROM (SELECT ST_Centroid({g}) AS c, {val} AS v FROM {} a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom)) WHERE c IS NOT NULL",
            ident(&i.table)
        ))?;
        let pts: Vec<(f64, f64, Option<f64>)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&i.table)), [], |r| r.get(0))?;
        if total > pts.len() as i64 {
            ctx.add_note(format!("Hex Bins: {} feature(s) without a usable position were left out.", total - pts.len() as i64));
        }
        let with_field = p.str("field").is_some();
        let mut cells: HashMap<h3o::CellIndex, (i64, f64, i64)> = HashMap::new();
        let mut no_value = 0i64;
        for (lon, lat, v) in &pts {
            let Ok(ll) = LatLng::new(*lat, *lon) else { continue };
            let e = cells.entry(ll.to_cell(resolution)).or_insert((0, 0.0, 0));
            e.0 += 1;
            match v.filter(|v| v.is_finite()) {
                Some(v) => {
                    e.1 += v;
                    e.2 += 1;
                }
                None => no_value += with_field as i64,
            }
        }
        if cells.is_empty() {
            bail!("No point could be placed on the hexagon grid");
        }
        if no_value > 0 {
            ctx.add_note(format!("Hex Bins: {no_value} point(s) have no number in the value field; they count in 'count' but not in the sum or mean."));
        }
        let back = geomkit::tx("EPSG:4326", crs)?;
        let geod = Geodesic::wgs84();
        let geographic = crate::crs::is_geographic(crs);
        let (mut wrapped, mut unplaced) = (0usize, 0usize);
        let mut keys: Vec<_> = cells.keys().cloned().collect();
        keys.sort();
        let mut rows = vec![];
        for cell in keys {
            let (n, sum, nv) = cells[&cell];
            let b = cell.boundary();
            let mut pa = PolygonArea::new(&geod, Winding::CounterClockwise);
            for ll in b.iter() {
                pa.add_point(ll.lat(), ll.lng());
            }
            let (_, area, _) = pa.compute(true);
            let ring: Option<Vec<(f64, f64)>> = b.iter().chain(b.first()).map(|ll| back.pt(ll.lng(), ll.lat())).collect();
            let Some(ring) = ring else {
                unplaced += 1;
                continue;
            };
            if geographic && ring.windows(2).any(|w| (w[0].0 - w[1].0).abs() > 180.0) {
                wrapped += 1;
            }
            let (s, m) = if with_field && nv > 0 { (sqlnum(sum), sqlnum(sum / nv as f64)) } else { ("NULL".into(), "NULL".into()) };
            rows.push(format!(
                "('{cell}', {n}, {s}, {m}, {nv}, {}, ST_GeomFromText('POLYGON(({}))'))",
                sqlnum(area.abs()),
                ring.iter().map(|(x, y)| format!("{x:?} {y:?}")).collect::<Vec<_>>().join(", ")
            ));
        }
        if unplaced > 0 {
            ctx.add_note(format!("Hex Bins: {unplaced} hexagon(s) cannot be drawn in this coordinate system (outside its domain) and were left out of the result."));
        }
        if wrapped > 0 {
            ctx.add_note(format!("Hex Bins: {wrapped} hexagon(s) cross the date line and are drawn as one outline that wraps around."));
        }
        let hexes = rows.len();
        ctx.add_note(format!("Hex Bins: {} point(s) in {hexes} hexagon(s) at H3 resolution {}.", pts.len(), res as u8));
        Ok(Output::Select { sql: format!("SELECT * FROM (VALUES {}) t(h3, count, sum, mean, values_used, area_m2, geom)", rows.join(", ")), crs: i.crs.clone(), view: false })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "hex_bins" => &["hexagon", "hexbin", "h3", "binning", "aggregate points", "grid of hexagons", "density map"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(HexBins)]
}
