//! ST6: Ripley's K tool reports an L chart with a Monte Carlo envelope and the seed it used.

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::json;

fn import_text(e: &Engine, dir: &std::path::Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    e.import(&p).unwrap().remove(0).id
}

#[test]
fn ripley_tool_reports_an_l_chart_with_envelope_and_a_visible_seed() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let crs = json!({"type": "name", "properties": {"name": "EPSG:32614"}});
    // Two tight clusters of points in a 1000 m square.
    let mut f = vec![];
    for c in [(500200.0, 4000200.0), (500800.0, 4000800.0)] {
        for k in 0..15 {
            let (dx, dy) = (((k * 37) % 11) as f64 * 4.0, ((k * 53) % 13) as f64 * 4.0);
            f.push(json!({"type": "Feature", "properties": {}, "geometry": {"type": "Point", "coordinates": [c.0 + dx, c.1 + dy]}}));
        }
    }
    let pts = import_text(&e, dir.path(), "pts.geojson", &json!({"type": "FeatureCollection", "crs": crs, "features": f}).to_string());
    let sq = json!({"type": "FeatureCollection", "crs": crs, "features": [{"type": "Feature", "properties": {}, "geometry": {"type": "Polygon", "coordinates": [[[500000.0, 4000000.0], [501000.0, 4000000.0], [501000.0, 4001000.0], [500000.0, 4001000.0], [500000.0, 4000000.0]]]}}]}).to_string();
    let area = import_text(&e, dir.path(), "area.geojson", &sq);
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "p", "tool": "input_layer", "params": {"layer": pts}}, {"id": "a", "tool": "input_layer", "params": {"layer": area}},
                  {"id": "t", "tool": "ripleys_k", "params": {"distances": "50, 100, 200", "edge": "none", "permutations": 39, "seed": 9}}],
        "edges": [{"source": "p", "target": "t", "targetHandle": "in"}, {"source": "a", "target": "t", "targetHandle": "area"}]
    }))
    .unwrap();
    let r = e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap();
    assert!(matches!(r.status, Status::Done), "{:?}", r.error);
    let st = r.stat.as_ref().unwrap();
    assert_eq!(st.seed, Some(9));
    assert_eq!(st.charts.len(), 1);
    assert_eq!(st.charts[0].series.len(), 4, "observed, two envelope lines, CSR line");
    // Clustered: the observed curve is above the envelope at 100 m.
    let obs = st.charts[0].series[0].marks[1].y.unwrap();
    let hi = st.charts[0].series[2].marks[1].y.unwrap();
    assert!(obs > hi, "obs {obs} hi {hi}");
    assert!(st.notes.iter().any(|n| n.contains("seed 9")));
}
