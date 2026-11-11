//! Tool help (UX12): what the help panel shows besides the tool's own `ToolSpec` (name, description, inputs, output and each setting's help
//! text, which the UI reads from the spec as everywhere else). Two things are added here, neither a second catalog of tools:
//!
//! * the **citation** for tools that implement a published method (the same references as `docs/OPEN_SOURCE.md` section 5; a test keeps
//!   every id real);
//! * the **before-and-after picture**, which is not drawn by hand: the tool is run on a small built-in sample (a wavy polygon, a triangle
//!   and an overlay ellipse in a metric coordinate system) with its own default settings, and both the sample and the real result are sent as
//!   SVG paths. A picture can therefore never disagree with the tool. A tool that needs something a sample cannot give (a field, a layer,
//!   a raster) has no picture, and the help says so.

use super::*;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Citation {
    pub text: &'static str,
    pub url: Option<&'static str>,
}

/// The published method behind a tool, when it has one (the references of `docs/OPEN_SOURCE.md` section 5).
pub fn citation(id: &str) -> Option<Citation> {
    let c = |text, url| Some(Citation { text, url: Some(url) });
    match id {
        "kernel_density" => c("Silverman 1986, Density Estimation for Statistics and Data Analysis, quartic (biweight) kernel", "https://doi.org/10.1201/9781315140919"),
        "average_nearest_neighbour" => c("Clark and Evans 1954, Distance to nearest neighbor as a measure of spatial relationships in populations, Ecology 35:445-453", "https://doi.org/10.2307/1931034"),
        "ripleys_k" => c("Ripley 1976, The second-order analysis of stationary point processes, J. Applied Probability 13:255-266", "https://doi.org/10.2307/3212829"),
        "dbscan" => c("Ester, Kriegel, Sander and Xu 1996, A density-based algorithm for discovering clusters, KDD-96", "https://dl.acm.org/doi/10.5555/3001460.3001507"),
        "tabulate_area" => c("Snyder 1987, Map Projections: A Working Manual, equation 3-12 (area on the ellipsoid)", "https://pubs.usgs.gov/publication/pp1395"),
        "hex_bins" => c("H3 hexagonal grid (Uber), through the h3o library", "https://h3geo.org"),
        "cut_fill" => c("Easa 1988, Estimating earthwork volumes from grids", "https://doi.org/10.1061/(ASCE)0733-9453(1988)114:2(167)"),
        "buffer" => c("Karney 2013, Algorithms for geodesics, J. Geodesy 87:43-55 (true distances on the ellipsoid)", "https://doi.org/10.1007/s00190-012-0578-z"),
        "delaunay" => c("Delaunay triangulation (Delaunay 1934, Sur la sphere vide), computed by GEOS", "https://libgeos.org"),
        "simplify" => c("Douglas and Peucker 1973, Algorithms for the reduction of the number of points required to represent a digitized line, Cartographica 10:112-122", "https://doi.org/10.3138/FM57-6770-U75U-7727"),
        "project" => c("PROJ coordinate transformation library", "https://proj.org"),
        _ => None,
    }
}

/// Settings that make the sample picture clear when a tool's own default would swamp a 100 m sample (the tool still runs for real).
fn sample_params(id: &str) -> Map<String, Value> {
    let v = match id {
        "buffer" => serde_json::json!({"distance": 8, "units": "m"}),
        "simplify" => serde_json::json!({"tolerance": 4}),
        "near" => serde_json::json!({}),
        _ => serde_json::json!({}),
    };
    v.as_object().cloned().unwrap_or_default()
}

#[derive(Serialize, Clone, Debug)]
pub struct PicLayer {
    /// "input", "overlay" or "result".
    pub role: &'static str,
    /// SVG path data, one entry per feature (y already points down).
    pub paths: Vec<String>,
    /// "polygon", "line" or "point".
    pub kind: &'static str,
}

#[derive(Serialize, Clone, Debug)]
pub struct Picture {
    pub before: Vec<PicLayer>,
    pub after: Vec<PicLayer>,
    /// SVG viewBox: x, y, width, height.
    pub view: [f64; 4],
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolHelp {
    pub id: String,
    pub citation: Option<Citation>,
    pub picture: Option<Picture>,
    /// Why there is no picture, in words.
    pub picture_note: Option<String>,
}

const X0: f64 = 500_000.0;
const Y0: f64 = 4_400_000.0;
const CRS: &str = "EPSG:32615";

fn cache() -> &'static Mutex<HashMap<String, (Option<Picture>, Option<String>)>> {
    static C: OnceLock<Mutex<HashMap<String, (Option<Picture>, Option<String>)>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Help for one tool: its citation and (made on first request, then kept) its before-and-after picture.
pub fn help(conn: &Connection, tools: &[Box<dyn Tool>], id: &str) -> Result<ToolHelp> {
    let tool = tools.iter().find(|t| t.spec().id == id).ok_or_else(|| anyhow!("Unknown tool '{id}'"))?;
    if let Some((p, n)) = cache().lock().unwrap().get(id).cloned() {
        return Ok(ToolHelp { id: id.into(), citation: citation(id), picture: p, picture_note: n });
    }
    let (picture, note) = match picture(conn, tool.as_ref()) {
        Ok(p) => (Some(p), None),
        Err(e) => (None, Some(format!("No picture for this tool: {e:#}"))),
    };
    cache().lock().unwrap().insert(id.into(), (picture.clone(), note.clone()));
    Ok(ToolHelp { id: id.into(), citation: citation(id), picture, picture_note: note })
}

fn sample_tables(conn: &Connection) -> Result<()> {
    let shift = |pts: &[(f64, f64)]| pts.iter().map(|(x, y)| format!("{} {}", X0 + x, Y0 + y)).collect::<Vec<_>>().join(", ");
    // A wavy polygon (so Simplify has something to do) and a triangle.
    let wavy: Vec<(f64, f64)> = (0..=24)
        .map(|i| {
            let t = i as f64 / 24.0;
            (8.0 + 38.0 * t, 14.0 + if i % 2 == 0 { 0.0 } else { 3.0 })
        })
        .chain((0..=12).map(|i| (46.0 + if i % 2 == 0 { 0.0 } else { 2.5 }, 14.0 + 38.0 * i as f64 / 12.0)))
        .chain((0..=24).map(|i| (46.0 - 38.0 * i as f64 / 24.0, 52.0 - if i % 2 == 0 { 0.0 } else { 3.0 })))
        .chain((0..=12).map(|i| (8.0 - if i % 2 == 0 { 0.0 } else { 2.5 }, 52.0 - 38.0 * i as f64 / 12.0)))
        .collect();
    let mut ring = wavy;
    ring.dedup();
    let first = ring[0];
    ring.push(first);
    conn.execute_batch(&format!(
        "CREATE OR REPLACE TEMP TABLE __help_a AS SELECT 1 AS id, 'A' AS name, 10.0 AS value, ST_GeomFromText('POLYGON(({}))') AS geom \
         UNION ALL SELECT 2, 'B', 20.0, ST_GeomFromText('POLYGON(({}))');
         CREATE OR REPLACE TEMP TABLE __help_b AS SELECT 1 AS id, 'overlay' AS name, 5.0 AS value, ST_Buffer(ST_GeomFromText('POINT({} {})'), 22, 16) AS geom;",
        shift(&ring),
        shift(&[(56.0, 20.0), (92.0, 26.0), (72.0, 64.0), (56.0, 20.0)]),
        X0 + 52.0,
        Y0 + 56.0
    ))?;
    Ok(())
}

fn paths(conn: &Connection, sql: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(&format!("SELECT ST_AsSVG(geom, false, 1), ST_GeometryType(geom)::VARCHAR FROM ({sql}) WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)"))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn layer(role: &'static str, rows: Vec<(String, String)>) -> Option<PicLayer> {
    if rows.is_empty() {
        return None;
    }
    let kind = if rows[0].1.contains("POLYGON") { "polygon" } else if rows[0].1.contains("LINE") { "line" } else { "point" };
    Some(PicLayer { role, paths: rows.into_iter().map(|r| r.0).collect(), kind })
}

fn picture(conn: &Connection, tool: &dyn Tool) -> Result<Picture> {
    let spec = tool.spec();
    if spec.role != Role::Tool || spec.inputs.is_empty() {
        bail!("it has no features going in");
    }
    if spec.output == Some(PortKind::Raster) || spec.inputs.iter().any(|p| p.kind == PortKind::Raster) {
        bail!("it works with a raster, which the sample does not have");
    }
    if spec.output != Some(PortKind::Vector) {
        bail!("its result is a table, not shapes to draw");
    }
    if spec.params.iter().any(|p| matches!(p.kind, ParamKind::Layer | ParamKind::SavePath)) {
        bail!("it needs a layer or a file");
    }
    sample_tables(conn)?;
    let result = (|| -> Result<Picture> {
        let mut params: Map<String, Value> = Map::new();
        for p in &spec.params {
            if !p.default.is_null() {
                params.insert(p.id.into(), p.default.clone());
            }
        }
        for (k, v) in sample_params(spec.id) {
            params.insert(k, v);
        }
        if let Some(p) = spec.params.iter().find(|p| !p.optional && !p.hidden && !params.contains_key(p.id) && !matches!(p.kind, ParamKind::Bool)) {
            bail!("its setting '{}' needs a value only you can choose", p.label);
        }
        let mut inputs: Inputs = HashMap::new();
        for (i, port) in spec.inputs.iter().enumerate() {
            inputs.insert(port.id, Rel { table: if i == 0 { "__help_a".into() } else { "__help_b".into() }, crs: Some(CRS.into()), has_geom: true, raster: None });
        }
        let layers = HashMap::new();
        let ctx = Ctx::new(conn, &layers);
        let out = tool.build(&ctx, &inputs, &Params(&params))?;
        let Output::Select { sql, .. } = out else { bail!("it makes its result in several steps") };
        let mut after = Vec::new();
        let mut before = Vec::new();
        before.extend(layer("input", paths(conn, "SELECT geom FROM __help_a")?));
        if spec.inputs.len() > 1 {
            before.extend(layer("overlay", paths(conn, "SELECT geom FROM __help_b")?));
        }
        let res = paths(conn, &sql).map_err(|e| anyhow!("the sample could not be run through it ({e})"))?;
        if res.is_empty() {
            bail!("the sample gives an empty result");
        }
        if before.first().is_some_and(|b| b.role == "input" && b.paths == res.iter().map(|r| r.0.clone()).collect::<Vec<_>>()) {
            bail!("it changes attributes, not shapes, so the sample would look the same before and after");
        }
        // The faint inputs stay under the result so the change can be seen.
        after.extend(before.iter().cloned().map(|mut l| {
            l.role = if l.role == "input" { "input-faint" } else { "overlay-faint" };
            l
        }));
        after.extend(layer("result", res));
        Ok(Picture { before, after, view: [X0, -(Y0 + 100.0), 100.0, 100.0] })
    })();
    let _ = conn.execute_batch("DROP TABLE IF EXISTS __help_a; DROP TABLE IF EXISTS __help_b;");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cited_tool_is_a_real_tool() {
        let ids: Vec<&str> = registry().iter().map(|t| t.spec().id).collect();
        for id in [
            "kernel_density", "average_nearest_neighbour", "ripleys_k", "dbscan", "tabulate_area", "hex_bins", "cut_fill", "buffer", "delaunay", "simplify", "project",
        ] {
            assert!(ids.contains(&id), "{id} is not a registered tool; the ids are: {ids:?}");
            assert!(citation(id).is_some_and(|c| !c.text.is_empty() && c.url.is_some_and(|u| u.starts_with("https://"))), "{id}");
        }
        assert!(citation("clip").is_none());
    }
}
