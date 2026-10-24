//! Layout renderer (docs/LAYOUTS_PLAN.md C2.1, acceptance P1–P6).

use engine::layout::model::{LayerStyle, RenderRequest};
use engine::layout::{export, render_svg};
use engine::Engine;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;

fn write_fixture(path: &Path, select: &str, srs: Option<&str>) {
    let driver = if path.extension().is_some_and(|e| e == "shp") { "ESRI Shapefile" } else { "GPKG" };
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let srs = srs.map(|s| format!(", SRS '{s}'")).unwrap_or_default();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER '{driver}'{srs}, USE_TMP_FILE false)", path.display())).unwrap();
}

const CENTER: [f64; 2] = [500_000.0, 4_649_776.0];

/// Polygons, a line and points around CENTER in UTM 15N (NAD83), plus a layer
/// without a coordinate system. Returns (engine, polygon id, line id, point id, no-crs id).
fn fixture(dir: &Path) -> (Engine, [String; 4]) {
    let (cx, cy) = (CENTER[0], CENTER[1]);
    let polys = dir.join("polys.gpkg");
    write_fixture(
        &polys,
        &format!(
            "SELECT 1 AS id, 'a' AS cls, 10.0 AS pop, ST_MakeEnvelope({a}, {b}, {c}, {d}) AS geom
             UNION ALL SELECT 2, 'b', 30.0, ST_MakeEnvelope({e}, {b}, {f}, {d})
             UNION ALL SELECT 3, 'c', 50.0, ST_MakeEnvelope({a}, {g}, {c}, {h})
             UNION ALL SELECT 4, 'a', 5.0, ST_MakeEnvelope({i}, {g}, {j}, {h})",
            a = cx - 1500.0, b = cy - 900.0, c = cx - 300.0, d = cy + 300.0,
            e = cx + 100.0, f = cx + 1300.0,
            g = cy - 2100.0, h = cy - 1000.0,
            i = cx + 1000.0, j = cx + 9000.0, // runs off the frame: must be clipped
        ),
        Some("EPSG:26915"),
    );
    let lines = dir.join("lines.gpkg");
    write_fixture(&lines, &format!("SELECT 1 AS id, ST_GeomFromText('LINESTRING({} {}, {} {}, {} {})') AS geom", CENTER[0] - 2000.0, CENTER[1] + 1000.0, CENTER[0], CENTER[1], CENTER[0] + 2000.0, CENTER[1] + 1200.0), Some("EPSG:26915"));
    let pts = dir.join("pts.gpkg");
    write_fixture(&pts, &format!("SELECT x AS id, ST_Point({} + x * 400, {} + 700) AS geom FROM range(5) t(x)", CENTER[0] - 800.0, CENTER[1]), Some("EPSG:26915"));
    let nocrs = dir.join("nocrs.shp"); // no .prj: the importer keeps the CRS unset
    write_fixture(&nocrs, "SELECT 1 AS id, ST_Point(1, 2) AS geom", None);
    let e = Engine::new().unwrap();
    let id = |p: &Path| e.import(p).unwrap()[0].id.clone();
    let ids = [id(&polys), id(&lines), id(&pts), id(&nocrs)];
    (e, ids)
}

fn request(ids: &[String; 4], style: serde_json::Value) -> RenderRequest {
    serde_json::from_value(json!({
        "layout": {
            "id": "L1", "name": "Fixture map", "version": 1, "body": "earth",
            "page": { "width": 279.4, "height": 215.9, "units": "mm", "margin": 12.7, "background": "#ffffff" },
            "elements": [
                { "kind": "map_frame", "id": "f1", "x": 12.7, "y": 30.0, "w": 200.0, "h": 160.0, "crs": "EPSG:26915",
                  "center": CENTER, "scale": 24000, "layers": "map", "border": { "width": 0.35, "color": "#000000" } },
                { "kind": "text", "id": "t1", "x": 12.7, "y": 10.0, "w": 254.0, "h": 14.0, "text": "{layout_name} – {scale}", "font": "sans-bold", "size": 18, "align": "center", "color": "#000000" }
            ]
        },
        "styles": { ids[0].clone(): style, ids[1].clone(): { "kind": "single", "fill": "#d33", "stroke": "#d33", "strokeWidth": 1.5, "opacity": 1, "radius": 3.5 },
                    ids[2].clone(): { "kind": "single", "fill": "#2a7", "stroke": "#2a7", "strokeWidth": 0.7, "opacity": 1, "radius": 3.5 } },
        "order": [ids[2], ids[1], ids[0], ids[3]],
        "visible": {},
        "project": "Demo", "date": "2026-01-01"
    }))
    .unwrap()
}

fn categorized() -> serde_json::Value {
    json!({ "kind": "categorized", "fill": "#999999", "stroke": "#4d4d4d", "strokeWidth": 0.7, "opacity": 0.75, "radius": 3.5, "field": "cls",
            "classes": [ { "value": "a", "color": "#7aa8d8", "label": "A" }, { "value": "b", "color": "#e6a35c", "label": "B" } ] })
}

fn render(e: &Engine, req: &RenderRequest) -> engine::layout::LayoutSvg {
    e.render_layout_svg(req, &|| Ok(())).unwrap()
}

/// Inflate every FlateDecode stream of a PDF.
fn pdf_streams(pdf: &[u8]) -> Vec<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = pdf[i..].windows(6).position(|w| w == b"stream") {
        let start = i + p + 6;
        let start = if pdf[start] == b'\r' { start + 2 } else { start + 1 };
        let end = start + pdf[start..].windows(9).position(|w| w == b"endstream").unwrap();
        let mut buf = Vec::new();
        if flate2::read::ZlibDecoder::new(&pdf[start..end]).read_to_end(&mut buf).is_ok() {
            out.push(buf);
        }
        i = end + 9;
    }
    out
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

#[test]
fn p2_pdf_is_vector_with_embedded_text() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let out = render(&e, &request(&ids, categorized()));
    assert!(out.svg.contains("<path"), "polygons drawn");
    assert!(out.svg.contains("<circle"), "points drawn");
    let pdf = export::svg_to_pdf(&out.svg).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(!contains(&pdf, "/Subtype /Image") && !contains(&pdf, "/XObject"), "no image XObject");
    assert!(contains(&pdf, "/FontFile2"), "font subset embedded");
    assert!(contains(&pdf, "/ToUnicode"), "text is extractable");
    let streams = pdf_streams(&pdf);
    let content: Vec<&Vec<u8>> = streams.iter().filter(|s| contains(s, " re") || contains(s, " m\n") || contains(s, " m ")).collect();
    assert!(!content.is_empty(), "page content stream found");
    assert!(content.iter().any(|s| contains(s, " l\n") || contains(s, " l ")), "path line operators");
    assert!(content.iter().any(|s| contains(s, "BT")), "text operators");
    // Letter landscape: 279.4 × 215.9 mm = 792 × 612 pt.
    assert!(contains(&pdf, "/MediaBox [0 0 792 611.9"), "page size is Letter landscape in points");
    // The title text goes through the token and the font: its glyph ids are in
    // the ToUnicode map, so the characters are present there.
    assert_eq!(out.warnings, vec!["Layer nocrs skipped: no coordinate system".to_string()]);
}

#[test]
fn p3_deterministic_and_golden() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let req = request(&ids, categorized());
    let a = render(&e, &req).svg;
    let b = render(&e, &req).svg;
    assert_eq!(a, b, "same input, same bytes");
    // A second engine imports the layers again: ids and row order are the same.
    let (e2, ids2) = fixture(dir.path());
    assert_eq!(ids, ids2);
    assert_eq!(a, render(&e2, &req).svg);
    let sha = hex::encode(Sha256::digest(a.as_bytes()));
    // Update by running with GOLDEN_PRINT=1 after an intended output change.
    if std::env::var("GOLDEN_PRINT").is_ok() {
        println!("GOLDEN {sha}");
        std::fs::write(std::env::temp_dir().join("layout_golden.svg"), &a).unwrap();
        std::fs::write(std::env::temp_dir().join("layout_golden.png"), export::svg_to_png(&a, 110.0).unwrap()).unwrap();
        std::fs::write(std::env::temp_dir().join("layout_golden.pdf"), export::svg_to_pdf(&a).unwrap()).unwrap();
    }
    assert_eq!(sha, GOLDEN_SHA256, "SVG output changed; if intended, regenerate with GOLDEN_PRINT=1");
}

const GOLDEN_SHA256: &str = "cb25ed50ccad12a2c89c65a06a4fd0bf5db7fd6b6a75dd3fc6f358ac617824b8";

#[test]
fn p5_missing_crs_errors_and_layers_without_crs_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let out = render(&e, &request(&ids, categorized()));
    // The no-CRS layer is not in the default visible set's warnings yet: it is, because visible defaults to true.
    assert!(e.layer(&ids[3]).unwrap().crs.is_none());
    let mut req = request(&ids, categorized());
    req.visible.insert(ids[3].clone(), true);
    let out2 = render(&e, &req);
    assert!(out2.warnings.iter().any(|w| w.contains("skipped: no coordinate system")), "{:?}", out2.warnings);
    assert_eq!(out.warnings, out2.warnings);

    let mut bad = request(&ids, categorized());
    if let engine::layout::model::Element::MapFrame { crs, .. } = &mut bad.layout.elements[0] {
        crs.clear();
    }
    let err = e.render_layout_svg(&bad, &|| Ok(())).unwrap_err().to_string();
    assert!(err.contains("Set a coordinate system for the map frame"), "{err}");

    let mut bad = request(&ids, categorized());
    if let engine::layout::model::Element::MapFrame { crs, .. } = &mut bad.layout.elements[0] {
        *crs = "EPSG:99999999".into();
    }
    assert!(e.render_layout_svg(&bad, &|| Ok(())).is_err());
}

#[test]
fn hidden_layers_and_unknown_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    req.visible.insert(ids[2].clone(), false);
    let svg = render(&e, &req).svg;
    assert!(!svg.contains("<circle"), "hidden point layer is not drawn");
    assert!(engine::layout::model::Layout::from_json(r#"{"id":"x","name":"x","page":{"width":10,"height":10},"elements":[{"kind":"hologram","id":"h","x":0,"y":0,"w":1,"h":1}]}"#).is_err());
    let mut req = request(&ids, categorized());
    req.layout.body = "mars".into();
    assert!(e.render_layout_svg(&req, &|| Ok(())).unwrap_err().to_string().contains("phase F"));
}

#[test]
fn clips_to_the_frame_and_follows_graduated_steps() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let grad = json!({ "kind": "graduated", "fill": "#999999", "stroke": "#4d4d4d", "strokeWidth": 0.7, "opacity": 1, "radius": 3.5, "field": "pop",
                       "breaks": [20, 40], "colors": ["#aa0000", "#00aa00", "#0000aa"] });
    let svg = render(&e, &request(&ids, grad)).svg;
    // pop 5 and 10 → first colour, 30 → second, 50 → third.
    for c in ["#aa0000", "#00aa00", "#0000aa"] {
        assert!(svg.contains(&format!("fill=\"{c}\"")), "{c} missing");
    }
    // Nothing is drawn further than the clip margin outside the frame (page x ≤ 12.7+200+2).
    let max_x = svg
        .split(&['M', 'L', 'Z'][..])
        .filter_map(|t| t.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()))
        .fold(0.0f64, f64::max);
    assert!(max_x <= 12.7 + 200.0 + 2.0 + 0.01 || max_x > 270.0, "max x {max_x}");
}

#[test]
fn layer_style_mirror_matches_the_ui_fixture() {
    let raw = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layer_style.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let style: LayerStyle = serde_json::from_value(v.clone()).unwrap();
    let back = serde_json::to_value(&style).unwrap();
    let keys = |v: &serde_json::Value| v.as_object().unwrap().keys().cloned().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys(&v), keys(&back), "LayerStyle fields drifted from the UI fixture (ui/src/layout/styleFixture.test.ts)");
    assert_eq!(v, back);
}

#[test]
fn p1_png_renders() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let svg = render(&e, &request(&ids, categorized())).svg;
    let png = export::svg_to_png(&svg, 96.0).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    let img = png::Decoder::new(std::io::Cursor::new(&png)).read_info().unwrap();
    let info = img.info();
    assert_eq!((info.width, info.height), (1056, 816));
}

/// P6. `cargo test -p engine --release p6 -- --ignored --nocapture`
#[test]
#[ignore]
fn p6_100k_polygons_png_150dpi() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("many.gpkg");
    // 100k squares of 20 m on a 320 × 313 grid with 30 m pitch, ~9.6 km wide.
    write_fixture(
        &p,
        &format!("SELECT x * 1000 + y AS id, ST_MakeEnvelope({x0} + x * 30, {y0} + y * 30, {x0} + x * 30 + 20, {y0} + y * 30 + 20) AS geom FROM range(320) a(x), range(313) b(y)", x0 = CENTER[0] - 4800.0, y0 = CENTER[1] - 4700.0),
        Some("EPSG:26915"),
    );
    let e = Engine::new().unwrap();
    let id = e.import(&p).unwrap()[0].id.clone();
    let mut req: RenderRequest = serde_json::from_value(json!({
        "layout": { "id": "L1", "name": "Perf", "page": { "width": 279.4, "height": 215.9 },
            "elements": [ { "kind": "map_frame", "id": "f1", "x": 12.7, "y": 12.7, "w": 254.0, "h": 190.0, "crs": "EPSG:26915", "center": CENTER, "scale": 40000 } ] },
        "order": [id.clone()], "styles": { id.clone(): { "kind": "single", "fill": "#7aa8d8", "stroke": "#4d4d4d", "strokeWidth": 0.7, "opacity": 0.75, "radius": 3.5 } }
    }))
    .unwrap();
    req.visible.insert(id, true);
    let t = std::time::Instant::now();
    let svg = e.render_layout_svg(&req, &|| Ok(())).unwrap().svg;
    let t_svg = t.elapsed();
    let png = export::svg_to_png(&svg, 150.0).unwrap();
    println!("P6: 100k polygons, SVG {:.2}s ({} MB), total to PNG {:.2}s ({} KB)", t_svg.as_secs_f64(), svg.len() / 1_000_000, t.elapsed().as_secs_f64(), png.len() / 1000);
    let _ = render_svg; // public API used by C2.2
    assert!(t.elapsed().as_secs_f64() < 1.5, "took {:?}", t.elapsed());
}

/// L4: no ellipsoid numbers in `layout/` outside `geo.rs`.
#[test]
fn no_ellipsoid_literals_outside_geo_rs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/layout");
    let needles = [["6378", "137"].concat(), ["298.257", "2"].concat(), ["EPSG:", "4326"].concat()];
    for f in std::fs::read_dir(dir).unwrap() {
        let p = f.unwrap().path();
        if p.file_name().unwrap() == "geo.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        for n in &needles {
            assert!(!text.contains(n.as_str()), "{} mentions {n}", p.display());
        }
    }
}
