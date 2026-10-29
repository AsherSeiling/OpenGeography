//! CRS tool matrix (docs/CRS_PLAN.md R4, RD-5, I7, I8, I12): every vector tool runs on the same shapes stored in eight different
//! CRSs; the results must agree (feature count, total geodesic area or length) and keep the first input's CRS. A guard fails when a
//! registered tool is in neither the matrix nor the exclusion list, so a new tool can't skip CRS testing.

#[path = "common/crs_tol.rs"]
mod crs_tol;

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

/// The CRSs the shapes are stored in: both datums, geographic and projected, metres and US feet, equal-area and Web Mercator.
const ZOO: [(&str, &str); 8] = [
    ("4326", "EPSG:4326"), ("4269", "EPSG:4269"), ("26914", "EPSG:26914"), ("32614", "EPSG:32614"),
    ("5070", "EPSG:5070"), ("2277", "EPSG:2277"), ("32139", "EPSG:32139"), ("3857", "EPSG:3857"),
];
/// The reference the others are compared with.
const REFERENCE: &str = "32614";

fn engine() -> &'static Engine {
    static E: OnceLock<Engine> = OnceLock::new();
    E.get_or_init(|| Engine::new().unwrap())
}

/// Shapes in WGS 84 lon/lat around (-97.5, 31.5): two overlapping squares, a line and five points inside the first square.
fn shape_wkt(role: &str) -> &'static str {
    match role {
        "A" => "POLYGON((-97.6 31.4, -97.4 31.4, -97.4 31.6, -97.6 31.6, -97.6 31.4))",
        "B" => "POLYGON((-97.5 31.5, -97.3 31.5, -97.3 31.7, -97.5 31.7, -97.5 31.5))",
        "L" => "LINESTRING(-97.7 31.45, -97.5 31.55, -97.3 31.65)",
        "L2" => "LINESTRING(-97.55 31.38, -97.45 31.62)",
        "P" => "MULTIPOINT((-97.58 31.42), (-97.52 31.48), (-97.5 31.5), (-97.46 31.53), (-97.42 31.58))",
        _ => unreachable!(),
    }
}

/// A layer holding `role` stored in zoo CRS `key`, with a name and a numeric field.
fn layer(dir: &Path, role: &str, key: &str, def: &str) -> String {
    let path = dir.join(format!("{role}_{key}.gpkg"));
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    // One feature per part for points; the vertices are moved to `def` and the layer is written in that CRS.
    let geom = format!("ST_Transform(g, 'EPSG:4326', '{def}', true)");
    let src = format!("(SELECT unnest(ST_Dump(ST_GeomFromText('{}'))).geom AS g)", shape_wkt(role));
    c.execute_batch(&format!(
        "COPY (SELECT row_number() OVER () AS id, 'f' || row_number() OVER () AS name, CAST(10 * row_number() OVER () AS DOUBLE) AS val, {geom} AS geom FROM {src}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS '{def}', USE_TMP_FILE false)",
        path.display()
    ))
    .unwrap();
    engine().import(&path).unwrap_or_else(|e| panic!("{role} in {key}: {e}")).remove(0).id
}

/// What a tool is run with: its input ports mapped to shape roles, and parameters.
struct Recipe {
    tool: &'static str,
    inputs: &'static [(&'static str, &'static str)],
    params: Value,
    /// How the results of the CRSs are compared.
    check: Check,
}

#[derive(Clone, Copy)]
enum Check {
    /// Same feature count and the same total geodesic area and length (RD-4).
    Same,
    /// Same, with a wider area tolerance: the work is planar in the storage CRS (an overlay cut, a triangulation in Albers or Web
    /// Mercator), so the geodesic area of the result differs by a little more than 1e-4 between projected CRSs.
    SameWithin(f64),
    /// Only that it runs, is not empty and keeps the CRS (for a tool whose result legitimately depends on the metric of the storage
    /// CRS). Unused now: triangulations and hulls used to need it (bug C-6) until they were made to compute in a metric CRS.
    #[allow(dead_code)]
    RunsAndKeepsCrs,
}

fn recipes() -> Vec<Recipe> {
    let r = |tool, inputs, params| Recipe { tool, inputs, params, check: Check::Same };
    vec![
        r("buffer", &[("in", "A")], json!({ "distance": 500, "units": "m" })),
        r("buffer", &[("in", "L")], json!({ "distance": 500, "units": "m", "dissolve": true })),
        r("clip", &[("in", "A"), ("clip", "B")], json!({})),
        r("intersect", &[("a", "A"), ("b", "B")], json!({})),
        r("union", &[("a", "A"), ("b", "B")], json!({})),
        Recipe { check: Check::SameWithin(3e-4), ..r("erase", &[("in", "A"), ("erase", "B")], json!({})) },
        r("symmetrical_difference", &[("a", "A"), ("b", "B")], json!({})),
        r("dissolve", &[("in", "A")], json!({})),
        r("collect", &[("in", "A")], json!({})),
        r("explode", &[("in", "P")], json!({})),
        r("repair", &[("in", "A")], json!({})),
        r("delete_duplicates", &[("in", "P")], json!({})),
        r("feature_to_point", &[("in", "A")], json!({})),
        r("bounding_geometry", &[("in", "P")], json!({ "shape": "hull" })),
        r("simplify", &[("in", "A")], json!({ "tolerance": 50, "units": "m" })),
        r("polygons_to_lines", &[("in", "A")], json!({})),
        r("extract_vertices", &[("in", "A")], json!({})),
        r("calculate_geometry", &[("in", "A")], json!({ "field": "AREA", "property": "area", "area_units": "sqkm" })),
        r("calculate_geometry", &[("in", "L")], json!({ "field": "LEN", "property": "length", "length_units": "km" })),
        r("spatial_join", &[("target", "A"), ("join", "P")], json!({ "predicate": "intersects" })),
        r("select_by_location", &[("in", "P"), ("by", "A")], json!({ "relation": "intersects" })),
        r("summarize_within", &[("polys", "A"), ("items", "P")], json!({ "field": "val", "stat": "sum" })),
        r("near", &[("in", "P"), ("near", "L")], json!({ "radius": 20, "units": "km" })),
        r("join_nearest", &[("in", "P"), ("near", "L")], json!({ "radius": 20, "units": "km" })),
        r("line_intersections", &[("a", "L"), ("b", "L2")], json!({})),
        r("points_along_lines", &[("in", "L")], json!({ "distance": 5000, "units": "m" })),
        r("points_to_path", &[("in", "P")], json!({ "order": "val" })),
        Recipe { check: Check::SameWithin(1e-3), ..r("delaunay", &[("in", "P")], json!({})) },
        Recipe { check: Check::SameWithin(1e-3), ..r("voronoi", &[("in", "P")], json!({})) },
        Recipe { check: Check::SameWithin(1e-3), ..r("concave_hull", &[("in", "P")], json!({})) },
        r("fishnet", &[("in", "A")], json!({ "size": 5, "units": "km" })),
    ]
}

/// Tools that are not vector-in/vector-out shape tools, with the reason (and where they are tested instead).
const EXCLUDED: [(&str, &str); 45] = [
    ("input_layer", "reads a layer; no CRS logic of its own"),
    ("layer_output", "writes the result back; covered by every matrix run"),
    ("add_to_map", "a table sink"),
    ("export", "tested for CRS in crs_export.rs (R-E3) and engine.rs"),
    ("project", "the CRS tool itself: tested in crs_reference.rs and project tests"),
    ("filter", "attribute query; no geometry"),
    ("attribute_join", "tables only"),
    ("calculate_field", "attribute expressions; no CRS"),
    ("merge", "tables only"),
    ("summary_statistics", "tables only"),
    ("remove_null", "attribute filter"),
    ("lines_to_polygons", "needs closed line work; covered by polygons_to_lines round trip in tools tests"),
    ("random_points", "random positions: count only, in tools tests"),
    ("symbology", "styling only: passes the data through unchanged (engine.rs symbology_node_passes_contours_through_and_validates)"),
    ("contours", "raster tool: R5"),
    ("hillshade", "raster tool: R5"),
    ("slope", "raster tool: R5"),
    ("clip_raster", "raster tool: R5"),
    ("aspect", "raster tool: R5"),
    ("ruggedness", "raster tool: R5"),
    ("warp", "raster tool: R5"),
    ("polygonize", "raster tool: R5"),
    ("sample_raster", "raster tool: R5"),
    ("raster_calc", "raster tool: raster_nodes.rs and raster_ops.rs"),
    ("raster_calc2", "raster tool: raster_nodes.rs and raster_ops.rs"),
    ("reclassify", "raster tool: raster_nodes.rs and raster_ops.rs"),
    ("nodata", "raster tool: raster_nodes.rs and raster_ops.rs"),
    ("select_bands", "raster tool: raster_nodes.rs and raster_ops.rs"),
    ("resample_raster", "raster tool: raster_nodes.rs and raster_ops.rs"),
    ("align_raster", "raster tool: raster_nodes.rs (aligns across CRSs, raster_ops.rs)"),
    ("mosaic_rasters", "raster tool: raster_nodes.rs and raster_ops.rs"),
    ("rasterize", "vector to raster in the layer's CRS: raster_nodes.rs"),
    ("interpolate", "points to raster in the layer's CRS: raster_nodes.rs (H1)"),
    ("morans_i", "refuses geographic CRSs, otherwise planar in the layer's CRS: spatial_stats.rs (H3)"),
    ("hot_spots", "refuses geographic CRSs, otherwise planar in the layer's CRS: spatial_stats.rs (H3)"),
    ("define_vertical_datum", "metadata only: the raster, its grid and CRS are copied unchanged: raster_nodes.rs (C1)"),
    ("kriging", "refuses geographic CRSs, otherwise planar in the layer's CRS: kriging.rs (H2)"),
    ("fill_depressions", "raster tool run by WhiteboxTools, CRS restamped from the input: hydrology.rs (H4)"),
    ("flow_direction", "raster tool run by WhiteboxTools, CRS restamped from the input: hydrology.rs (H4)"),
    ("flow_accumulation", "raster tool run by WhiteboxTools, CRS restamped from the input: hydrology.rs (H4)"),
    ("basins", "raster tool run by WhiteboxTools, CRS restamped from the input: hydrology.rs (H4)"),
    ("extract_streams", "raster tool run by WhiteboxTools, CRS restamped from the input: hydrology.rs (H4)"),
    ("watersheds", "outlets are reprojected into the raster's CRS: hydrology.rs (H4)"),
    ("spectral_index", "raster tool, grid and CRS carried through: remote_sensing.rs, raster_nodes.rs (H5)"),
    ("classify_kmeans", "raster tool, grid and CRS carried through: remote_sensing.rs, raster_nodes.rs (H5)"),
];
// zonal_statistics is the 23rd raster tool; the list above is checked by the guard against the registry.

fn run(e: &Engine, tool: &str, layers: &[(&str, String)], params: &Value) -> Result<(engine::layers::LayerInfo, engine::graph::NodeResult), String> {
    let mut nodes = vec![];
    let mut edges = vec![];
    for (i, (port, id)) in layers.iter().enumerate() {
        nodes.push(json!({ "id": format!("in{i}"), "tool": "input_layer", "params": { "layer": id } }));
        edges.push(json!({ "source": format!("in{i}"), "target": "t", "targetHandle": port }));
    }
    nodes.push(json!({ "id": "t", "tool": tool, "params": params }));
    nodes.push(json!({ "id": "o", "tool": "layer_output", "params": { "layer": layers[0].1 } }));
    edges.push(json!({ "source": "t", "target": "o", "targetHandle": "in" }));
    let g: Graph = serde_json::from_value(json!({ "nodes": nodes, "edges": edges })).unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).map_err(|e| format!("{e:#}"))?;
    let t = res.iter().find(|r| r.id == "t").unwrap();
    if t.status != Status::Done && t.status != Status::Cached {
        return Err(t.error.clone().unwrap_or_else(|| format!("{:?}", t.status)));
    }
    let o = res.iter().find(|r| r.id == "o").unwrap().clone();
    match &o.layer {
        Some(l) => Ok((l.clone(), t.clone())),
        None => Err(format!("no result layer: {o:?}")),
    }
}

/// (feature count, total geodesic area m², total geodesic length m) of a result layer.
fn measures(e: &Engine, l: &engine::layers::LayerInfo) -> (i64, f64, f64) {
    let (mut area, mut len) = (0.0, 0.0);
    for fid in 1..=l.feature_count {
        if let Ok(m) = e.measure_feature(&l.id, fid) {
            area += m.area.unwrap_or(0.0);
            len += m.length.unwrap_or(0.0);
        }
    }
    (l.feature_count, area, len)
}

/// Run the recipe in every zoo CRS: {key → (count, area, length)} or the error.
fn matrix(rc: &Recipe) -> BTreeMap<&'static str, Result<(i64, f64, f64, Option<String>), String>> {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let mut out = BTreeMap::new();
    for (key, def) in ZOO {
        let layers: Vec<(&str, String)> = rc.inputs.iter().map(|(port, role)| (*port, layer(dir.path(), role, key, def))).collect();
        let r = run(e, rc.tool, &layers, &rc.params).map(|(l, _)| {
            let (n, a, len) = measures(e, &l);
            (n, a, len, l.crs.clone())
        });
        out.insert(key, r);
    }
    out
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b { 0.0 } else { (a - b).abs() / a.abs().max(b.abs()) }
}

#[test]
fn every_registered_tool_is_in_the_matrix_or_excluded_with_a_reason() {
    let covered: std::collections::BTreeSet<&str> = recipes().iter().map(|r| r.tool).chain(EXCLUDED.iter().map(|(t, _)| *t)).chain(["zonal_statistics"]).collect();
    let missing: Vec<String> = engine().tool_specs().into_iter().map(|s| s.id.to_string()).filter(|id| !covered.contains(id.as_str())).collect();
    assert!(missing.is_empty(), "add these tools to crs_matrix.rs (a recipe, or EXCLUDED with a reason): {missing:?}");
    for (t, why) in EXCLUDED {
        assert!(!why.is_empty(), "{t}");
    }
}

#[test]
fn r_f_every_tool_gives_the_same_answer_whatever_crs_its_input_is_stored_in() {
    let mut problems: Vec<String> = Vec::new();
    for rc in recipes() {
        let m = matrix(&rc);
        let label = format!("{} {:?}", rc.tool, rc.inputs);
        let Ok((n0, a0, l0, c0)) = m[REFERENCE].clone() else {
            problems.push(format!("{label}: fails in the reference CRS {REFERENCE}: {:?}", m[REFERENCE]));
            continue;
        };
        for (key, def) in ZOO {
            match &m[key] {
                Err(e) => problems.push(format!("{label} in {key}: error {e}")),
                Ok((n, a, l, crs)) => {
                    if let Check::RunsAndKeepsCrs = rc.check {
                        if *n == 0 || !(a.is_finite() && l.is_finite()) {
                            problems.push(format!("{label} in {key}: empty or non-finite result"));
                        }
                        if !crs.as_deref().is_some_and(|c| engine::crs::same(c, def)) {
                            problems.push(format!("{label} in {key}: the result's CRS is {crs:?}, not {def}"));
                        }
                        continue;
                    }
                    let area_tol = if let Check::SameWithin(t) = rc.check { t } else { crs_tol::AREA_REL };
                    if *n != n0 {
                        problems.push(format!("{label} in {key}: {n} features, {n0} in {REFERENCE}"));
                    }
                    if rel(*a, a0) > area_tol {
                        problems.push(format!("{label} in {key}: area {a:.1} m², {a0:.1} in {REFERENCE} (relative {:.2e})", rel(*a, a0)));
                    }
                    if rel(*l, l0) > crs_tol::LENGTH_REL * 10.0 {
                        problems.push(format!("{label} in {key}: length {l:.1} m, {l0:.1} in {REFERENCE} (relative {:.2e})", rel(*l, l0)));
                    }
                    // I8: the result keeps the first input's CRS.
                    if !crs.as_deref().is_some_and(|c| engine::crs::same(c, def)) {
                        problems.push(format!("{label} in {key}: the result's CRS is {crs:?}, not {def}"));
                    }
                }
            }
        }
        let _ = c0;
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}
