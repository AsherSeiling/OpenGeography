//! Natural Neighbour interpolation and Constrained TIN (breaklines) to a raster (Wave 1, package T6), on the `spade` crate (MIT OR Apache-2.0,
//! Stefan Altmayer and contributors: Delaunay and constrained Delaunay triangulation with robust predicates).
//!
//! Natural neighbour (Sibson 1981, "A brief description of natural neighbour interpolation"): the value at a place is the average of the values
//! of the points whose Voronoi cells it would take area from, weighted by the area taken. It reproduces a plane exactly, is smooth between the
//! points and has no overshoot. Constrained TIN: the Delaunay triangulation is forced to contain the breaklines (Chew 1989; Sloan 1993), and the
//! value is linear (barycentric) inside each triangle, so ridges, streams and road edges stay sharp. Cells outside the convex hull of the points
//! are no data. Both need a projected coordinate system: the triangulation depends on distances, which degrees distort.

use super::*;
use crate::raster;
use spade::{ConstrainedDelaunayTriangulation, DelaunayTriangulation, FloatTriangulation, HasPosition, Point2, Triangulation};
use std::collections::HashMap;

struct Pv {
    pos: Point2<f64>,
    z: f64,
}
impl HasPosition for Pv {
    type Scalar = f64;
    fn position(&self) -> Point2<f64> {
        self.pos
    }
}

/// Points with a number: positions in the layer's CRS, duplicates at one place averaged. Returns the points and the number of duplicates merged.
fn read_values(ctx: &Ctx, rel: &Rel, field: &str, tool: &str) -> Result<(Vec<(f64, f64, f64)>, usize)> {
    let crs = need_crs(rel)?;
    if crate::crs::is_geographic(crs) {
        bail!("{tool} triangulates in the plane, and this layer is in degrees, which distort distances. Project the points to a metre-based coordinate system first (Project).");
    }
    if !rel.has_geom {
        bail!("{tool} needs points with a geometry");
    }
    let mut stmt = ctx.conn.prepare(&format!(
        "SELECT ST_X(c), ST_Y(c), v FROM (SELECT ST_Centroid(geom) AS c, TRY_CAST({}::VARCHAR AS DOUBLE) AS v FROM {} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)) WHERE c IS NOT NULL",
        ident(field), ident(&rel.table)
    ))?;
    let rows: Vec<(f64, f64, Option<f64>)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&rel.table)), [], |r| r.get(0))?;
    let mut by_pos: HashMap<(u64, u64), (f64, f64, f64, usize)> = HashMap::new();
    let mut order: Vec<(u64, u64)> = vec![];
    let mut skipped = 0i64;
    for (x, y, v) in rows {
        let Some(v) = v.filter(|v| v.is_finite() && x.is_finite() && y.is_finite()) else {
            skipped += 1;
            continue;
        };
        let e = by_pos.entry((x.to_bits(), y.to_bits())).or_insert_with(|| {
            order.push((x.to_bits(), y.to_bits()));
            (x, y, 0.0, 0)
        });
        e.2 += v;
        e.3 += 1;
    }
    let left_out = (total - by_pos.values().map(|e| e.3 as i64).sum::<i64>()).max(skipped);
    if left_out > 0 {
        ctx.add_note(format!("{tool}: {left_out} feature(s) without a position or a number in '{field}' were left out."));
    }
    let dup = by_pos.values().map(|e| e.3 - 1).sum::<usize>();
    if dup > 0 {
        ctx.add_note(format!("{tool}: {dup} point(s) share a place with another point; the values at one place were averaged."));
    }
    let pts: Vec<(f64, f64, f64)> = order.iter().map(|k| { let e = &by_pos[k]; (e.0, e.1, e.2 / e.3 as f64) }).collect();
    if pts.len() < 3 {
        bail!("{tool} needs at least 3 points with a number");
    }
    Ok((pts, dup))
}

/// Write a raster over the bounding box of the points; `f` gives the value at a cell centre or None (no data).
fn make_raster(ctx: &Ctx, crs: &str, pts: &[(f64, f64, f64)], cell: f64, tool: &str, f: &dyn Fn(f64, f64) -> Option<f64>) -> Result<std::path::PathBuf> {
    let bb = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]);
    let (x0, y1) = ((bb[0] / cell).floor() * cell, (bb[3] / cell).ceil() * cell);
    let cols = (((bb[2] - x0) / cell).ceil() as usize).max(1);
    let rows = (((y1 - bb[1]) / cell).ceil() as usize).max(1);
    crate::raster_ops::check_cells(cols as f64, rows as f64)?;
    let task = crate::progress::Task::start(format!("{tool}: filling the raster")).cancellable();
    let mut data = vec![-9999.0f32; cols * rows];
    let mut nodata = 0usize;
    for r in 0..rows {
        if r % 16 == 0 {
            task.check()?;
            task.set(r as f64 / rows as f64, None::<String>);
        }
        let y = y1 - (r as f64 + 0.5) * cell;
        for c in 0..cols {
            match f(x0 + (c as f64 + 0.5) * cell, y) {
                Some(v) if v.is_finite() => data[r * cols + c] = v as f32,
                _ => nodata += 1,
            }
        }
    }
    let wkt = crate::crs::srs(&crate::crs::resolve(crs))?.to_wkt()?;
    let grid = crate::raster_ops::Grid { cols, rows, gt: [x0, cell, 0.0, y1, 0.0, -cell], wkt, bands: 1 };
    let out = raster::new_store_path("tin", "tif");
    {
        let ds = crate::raster_ops::create_like(&out, &grid, 1, Some(-9999.0))?;
        ds.rasterband(1)?.write((0, 0), (cols, rows), &mut gdal::raster::Buffer::new((cols, rows), data))?;
    }
    raster::build_overviews(&out)?;
    if nodata > 0 {
        ctx.add_note(format!("{tool}: {nodata} of {} cells lie outside the convex hull of the points and have no data.", cols * rows));
    }
    Ok(out)
}

fn cell_param(p: &Params) -> Result<f64> {
    let c = p.f64("cell").unwrap_or(f64::NAN);
    if !(c.is_finite() && c > 0.0) {
        bail!("'Cell size' must be a number greater than 0 (in the layer's units)");
    }
    Ok(c)
}

pub struct NaturalNeighbour;
impl Tool for NaturalNeighbour {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "natural_neighbour", name: "Natural Neighbour Interpolation", category: "Raster › Interpolation", role: Role::Tool,
            description: "A smooth surface through points with a number field (Sibson 1981): each cell takes the average of the neighbouring points' values weighted by the area its position would take from their Voronoi cells. It passes through every point, never goes above the highest or below the lowest value, and reproduces a plane exactly. Cells outside the convex hull of the points have no data (the number is reported). Points at the same place are averaged. The cell size is in the layer's units and the layer must be projected (a layer in degrees is refused: project it first). Computed with the spade triangulation library.",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }], output: Some(PortKind::Raster),
            params: vec![Param::new("field", "Value field", ParamKind::Field).field_of("in"), Param::new("cell", "Cell size", ParamKind::Number).default(100)] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let cell = cell_param(p)?;
        let (pts, _) = read_values(ctx, i, &p.req_str("field", "Value field")?, "Natural Neighbour")?;
        let mut tri: DelaunayTriangulation<Pv> = DelaunayTriangulation::new();
        for (x, y, z) in &pts {
            tri.insert(Pv { pos: Point2::new(*x, *y), z: *z }).map_err(|e| anyhow!("A point could not be used in the triangulation: {e:?}"))?;
        }
        if tri.all_vertices_on_line() {
            bail!("Natural Neighbour needs points that do not all lie on one line");
        }
        let nn = tri.natural_neighbor();
        let out = make_raster(ctx, need_crs(i)?, &pts, cell, "Natural Neighbour", &|x, y| nn.interpolate(|v| v.data().z, Point2::new(x, y)))?;
        Ok(Output::Raster { path: out.display().to_string(), crs: i.crs.clone() })
    }
}

/// Coordinates of every innermost list in a WKT string, with Z when there is one.
fn coords_z(wkt: &str) -> Vec<Vec<(f64, f64, Option<f64>)>> {
    let mut out = vec![];
    let mut start = None;
    for (i, ch) in wkt.char_indices() {
        match ch {
            '(' => start = Some(i + 1),
            ')' => {
                if let Some(s) = start.take() {
                    let l: Vec<(f64, f64, Option<f64>)> = wkt[s..i]
                        .split(',')
                        .filter_map(|t| {
                            let v: Vec<f64> = t.split_whitespace().filter_map(|n| n.parse().ok()).collect();
                            (v.len() >= 2).then(|| (v[0], v[1], v.get(2).cloned()))
                        })
                        .collect();
                    if !l.is_empty() {
                        out.push(l);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

pub struct TinBreaklines;
impl Tool for TinBreaklines {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "tin_breaklines", name: "TIN with Breaklines", category: "Raster › Interpolation", role: Role::Tool,
            description: "A surface from a triangulated irregular network (TIN) that is forced to follow breaklines such as ridges, streams, cliffs and road edges (constrained Delaunay triangulation, Chew 1989). The value is linear inside each triangle, so the surface has a sharp edge along every breakline. Breakline heights come from the lines' own Z values, or from a number field of the line layer used as one constant height per line (then every vertex of that line has that height). Breaklines that cross another breakline cannot be used and are counted; a breakline vertex at the place of a point keeps the point's value. Cells outside the convex hull have no data (the number is reported). The layers must be projected (degrees are refused: project first); breaklines are transformed to the points' coordinate system explicitly. Computed with the spade triangulation library.",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }, Port { id: "lines", label: "Breaklines", kind: PortKind::Vector }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("field", "Value field of the points", ParamKind::Field).field_of("in"),
                Param::new("line_field", "Height field of the breaklines (empty: use Z)", ParamKind::Field).field_of("lines").optional(),
                Param::new("cell", "Cell size", ParamKind::Number).default(100),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (i, l) = (&inputs["in"], &inputs["lines"]);
        let crs = need_crs(i)?;
        let cell = cell_param(p)?;
        let (pts, _) = read_values(ctx, i, &p.req_str("field", "Value field of the points")?, "TIN with Breaklines")?;
        if !l.has_geom || ctx.dimension(l)? != 1 {
            bail!("The breaklines must be a line layer");
        }
        let g = ctx.geom_in(l, "a", crs)?;
        let lf = p.str("line_field").map(|f| format!("TRY_CAST(a.{}::VARCHAR AS DOUBLE)", ident(&f))).unwrap_or_else(|| "NULL".into());
        let mut stmt = ctx.conn.prepare(&format!("SELECT ST_AsText(g), h FROM (SELECT {g} AS g, {lf} AS h FROM {} a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom)) WHERE g IS NOT NULL", ident(&l.table)))?;
        let lines: Vec<(String, Option<f64>)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let mut cdt: ConstrainedDelaunayTriangulation<Pv> = ConstrainedDelaunayTriangulation::new();
        let mut at: HashMap<(u64, u64), spade::handles::FixedVertexHandle> = HashMap::new();
        let mut point_keys: std::collections::HashSet<(u64, u64)> = Default::default();
        for (x, y, z) in &pts {
            let h = cdt.insert(Pv { pos: Point2::new(*x, *y), z: *z }).map_err(|e| anyhow!("A point could not be used in the triangulation: {e:?}"))?;
            at.insert((x.to_bits(), y.to_bits()), h);
            point_keys.insert((x.to_bits(), y.to_bits()));
        }
        let (mut used, mut crossing, mut no_height, mut on_point) = (0usize, 0usize, 0usize, 0usize);
        for (wkt, h) in &lines {
            for line in coords_z(wkt) {
                let mut handles = vec![];
                for (x, y, z) in &line {
                    let Some(z) = h.or(*z).filter(|z| z.is_finite()) else {
                        no_height += 1;
                        handles.clear();
                        break;
                    };
                    let key = (x.to_bits(), y.to_bits());
                    let hd = match at.get(&key) {
                        Some(hd) => {
                            on_point += point_keys.contains(&key) as usize;
                            *hd
                        }
                        None => {
                            let hd = cdt.insert(Pv { pos: Point2::new(*x, *y), z }).map_err(|e| anyhow!("A breakline vertex could not be used: {e:?}"))?;
                            at.insert(key, hd);
                            hd
                        }
                    };
                    handles.push(hd);
                }
                for w in handles.windows(2) {
                    if w[0] == w[1] {
                        continue;
                    }
                    if cdt.can_add_constraint(w[0], w[1]) {
                        cdt.add_constraint(w[0], w[1]);
                        used += 1;
                    } else {
                        crossing += 1;
                    }
                }
            }
        }
        if no_height > 0 {
            ctx.add_note(format!("TIN with Breaklines: {no_height} breakline(s) have no height (no Z and no value in the height field) and were not used."));
        }
        if crossing > 0 {
            ctx.add_note(format!("TIN with Breaklines: {crossing} breakline segment(s) cross another breakline and were not used."));
        }
        if on_point > 0 {
            ctx.add_note(format!("TIN with Breaklines: {on_point} breakline vertex(es) fall on a point and keep that point's value."));
        }
        if used == 0 {
            ctx.add_note("TIN with Breaklines: no breakline was used, so this is an ordinary TIN of the points.".into());
        }
        if cdt.all_vertices_on_line() {
            bail!("TIN with Breaklines needs points that do not all lie on one line");
        }
        let bary = cdt.barycentric();
        let out = make_raster(ctx, crs, &pts, cell, "TIN with Breaklines", &|x, y| bary.interpolate(|v| v.data().z, Point2::new(x, y)))?;
        Ok(Output::Raster { path: out.display().to_string(), crs: i.crs.clone() })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "natural_neighbour" => &["sibson", "interpolate", "surface from points", "smooth surface", "gridding", "voronoi"],
        "tin_breaklines" => &["triangulated surface", "ridge lines", "stream lines", "constrained delaunay", "surface with breaklines", "terrain model"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(NaturalNeighbour), Box::new(TinBreaklines)]
}
