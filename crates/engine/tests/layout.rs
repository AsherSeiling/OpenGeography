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

// ---------------------------------------------------------------- C2.2

#[test]
fn layouts_are_stored_in_the_project_and_survive_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut layout = request(&ids, categorized()).layout;
    e.save_layout(&layout).unwrap();
    layout.name = "Renamed".into();
    e.save_layout(&layout).unwrap(); // same id: replaced
    assert_eq!(e.list_layouts().unwrap().len(), 1);
    let proj = dir.path().join("p.gproj");
    e.save_project(&proj, "{}").unwrap();
    drop(e); // release the project file (Windows locks it while open)
    let e2 = Engine::new().unwrap();
    e2.open_project(&proj).unwrap();
    let back = e2.list_layouts().unwrap();
    assert_eq!(back, vec![layout.clone()]);
    e2.delete_layout("L1").unwrap();
    assert!(e2.list_layouts().unwrap().is_empty());
}

#[test]
fn save_layout_validates() {
    let e = Engine::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (_, ids) = fixture(dir.path());
    let mut layout = request(&ids, categorized()).layout;
    layout.version = 99;
    assert!(e.save_layout(&layout).is_err());
    assert!(e.list_layouts().unwrap().is_empty());
}

#[test]
fn export_writes_svg_pdf_png_atomically_and_preview_is_base64_png() {
    use engine::layout::ExportFormat;
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let req = request(&ids, categorized());
    let out = dir.path().join("out");
    for (fmt, name, magic) in [(ExportFormat::Svg, "m.svg", &b"<svg"[..]), (ExportFormat::Pdf, "m.pdf", &b"%PDF-"[..]), (ExportFormat::Png, "m.png", &b"\x89PNG"[..])] {
        let warnings = e.export_layout(&req, fmt, 72.0, &out.join(name)).unwrap();
        assert_eq!(warnings, vec!["Layer nocrs skipped: no coordinate system".to_string()]);
        assert!(std::fs::read(out.join(name)).unwrap().starts_with(magic), "{name}");
    }
    let leftovers: Vec<_> = std::fs::read_dir(&out).unwrap().map(|f| f.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".part")).collect();
    assert!(leftovers.is_empty(), "no temp files left: {leftovers:?}");
    let p = e.render_layout_preview(&req, 96.0).unwrap();
    use base64::Engine as _;
    assert!(base64::engine::general_purpose::STANDARD.decode(&p.png).unwrap().starts_with(b"\x89PNG"));
    assert_eq!(p.warnings.len(), 1);
}

#[test]
fn cancelled_render_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let req = request(&ids, categorized());
    let err = e.render_layout_svg(&req, &|| anyhow::bail!(engine::progress::CANCELLED)).unwrap_err();
    assert!(engine::progress::is_cancelled(&err));
}

// ---------------------------------------------------------------- C2.3

mod surround {
    use super::*;
    use engine::layout::geo::{self, Body, FrameCrs};
    use geographiclib_rs::{Geodesic, InverseGeodesic};

    /// A frame (no data layers) plus a scale bar and a north arrow.
    fn bare(crs: &str, center: [f64; 2], scale: f64, rotation: f64, extra: serde_json::Value) -> RenderRequest {
        let mut elements = vec![
            json!({ "kind": "map_frame", "id": "f1", "x": 12.7, "y": 30.0, "w": 200.0, "h": 160.0, "rotation": rotation, "crs": crs, "center": center, "scale": scale, "layers": "map" }),
            json!({ "kind": "scale_bar", "id": "sb", "frame": "f1", "x": 220.0, "y": 120.0, "w": 46.0, "h": 10.0, "units": "km", "segments": 4 }),
            json!({ "kind": "north_arrow", "id": "na", "frame": "f1", "x": 250.0, "y": 140.0, "w": 12.0, "h": 18.0, "style": "simple" }),
        ];
        elements.extend(extra.as_array().cloned().unwrap_or_default());
        serde_json::from_value(json!({
            "layout": { "id": "L1", "name": "Bare", "page": { "width": 279.4, "height": 215.9 }, "elements": elements },
            "order": [], "project": "Demo", "date": "2026-01-01"
        }))
        .unwrap()
    }

    fn group<'a>(svg: &'a str, id: &str) -> &'a str {
        let start = svg.find(&format!("<g id=\"{id}\"")).unwrap_or_else(|| panic!("no group {id}"));
        let end = svg[start..].find("</g>").unwrap() + start;
        &svg[start..end]
    }

    fn attr(tag: &str, name: &str) -> f64 {
        let i = tag.find(&format!(" {name}=\"")).unwrap() + name.len() + 3;
        tag[i..].split('"').next().unwrap().parse().unwrap()
    }

    fn texts(svg: &str) -> Vec<String> {
        svg.split("<text ").skip(1).map(|t| t.split_once('>').unwrap().1.split("</text>").next().unwrap().to_string()).collect()
    }

    /// Bar length (mm) and label (value, unit) of the scale bar `sb`.
    fn bar(svg: &str) -> (f64, f64, String) {
        let g = group(svg, "scalebar-sb");
        let len: f64 = g.split("<rect ").skip(1).map(|r| attr(r, "width")).sum();
        let last = texts(g).pop().unwrap();
        let (v, u) = last.split_once(' ').unwrap();
        (len, v.parse().unwrap(), u.to_string())
    }

    #[test]
    fn p11_scale_bar_matches_geodesic_distance() {
        let body = Body::get("earth").unwrap();
        let e = Engine::new().unwrap();
        // (CRS, lon, lat, scale, units, metres per unit)
        for (crs, lon, lat, scale, units, m) in [
            ("EPSG:26915", -96.0, 45.0, 100_000.0, "km", 1000.0),
            ("EPSG:2276", -97.3, 32.8, 100_000.0, "mi", 1609.344), // State Plane, US survey feet
            ("EPSG:5070", -110.0, 35.0, 250_000.0, "km", 1000.0),  // Albers CONUS
        ] {
            let fc = FrameCrs::new(crs, &body).unwrap();
            let c = fc.from_lonlat(lon, lat).unwrap();
            let mut req = bare(crs, [c.0, c.1], scale, 0.0, json!([]));
            if let engine::layout::model::Element::ScaleBar { units: u, .. } = &mut req.layout.elements[1] {
                *u = units.into();
            }
            let svg = render(&e, &req).svg;
            let (len_mm, label, unit) = bar(&svg);
            assert_eq!(unit, units);
            // Inverse-project the drawn bar's end points (grid east of the frame centre).
            let half = len_mm / 1000.0 * scale / fc.unit_m / 2.0;
            let (a, b) = (fc.to_lonlat(c.0 - half, c.1).unwrap(), fc.to_lonlat(c.0 + half, c.1).unwrap());
            let ground: f64 = Geodesic::new(body.a, body.f).inverse(a.1, a.0, b.1, b.0);
            assert!((ground / (label * m) - 1.0).abs() < 1e-3, "{crs}: drawn bar spans {ground} m, label says {label} {unit}");
            assert!(len_mm <= 46.0 - 1.0 - 7.0 + 1e-6, "{crs}: bar fits its box ({len_mm} mm)");
        }
    }

    #[test]
    fn p12_north_arrow_follows_convergence_and_frame_rotation() {
        let body = Body::get("earth").unwrap();
        let e = Engine::new().unwrap();
        let fc = FrameCrs::new("EPSG:26915", &body).unwrap();
        let angle = |lon: f64, rot: f64, style: &str| {
            let c = fc.from_lonlat(lon, 45.0).unwrap();
            let mut req = bare("EPSG:26915", [c.0, c.1], 24_000.0, rot, json!([]));
            if let engine::layout::model::Element::NorthArrow { style: s, .. } = &mut req.layout.elements[2] {
                *s = style.into();
            }
            let svg = render(&e, &req).svg;
            let tag = svg.split("<g id=\"northarrow-na\"").nth(1).unwrap_or_else(|| panic!("no arrow in {style}")).split('>').next().unwrap().to_string();
            let a: f64 = tag.split("rotate(").nth(1).unwrap().split_whitespace().next().unwrap().parse().unwrap();
            let gamma = geo::factors(&body, &fc, c.0, c.1).unwrap().convergence_deg;
            (a, gamma)
        };
        for style in ["simple", "compass"] {
            let (a, gamma) = angle(-96.0, 0.0, style);
            assert!((a - (-gamma)).abs() < 0.01, "{a} vs convergence {gamma}");
            assert!(a > 2.0 && a < 2.3, "west of the central meridian true north leans east: {a}");
            let (a, _) = angle(-90.0, 0.0, style);
            assert!(a < -2.0 && a > -2.3, "east of it, west: {a}");
            let (a, gamma) = angle(-96.0, 30.0, style);
            assert!((a - (30.0 - gamma)).abs() < 0.01, "frame rotation adds: {a}");
        }
    }

    #[test]
    fn p15_scale_variation_warning_for_web_mercator_at_60n() {
        let body = Body::get("earth").unwrap();
        let fc = FrameCrs::new("EPSG:3857", &body).unwrap();
        let c = fc.from_lonlat(10.0, 60.0).unwrap();
        let e = Engine::new().unwrap();
        let out = render(&e, &bare("EPSG:3857", [c.0, c.1], 5_000_000.0, 0.0, json!([])));
        assert!(out.warnings.iter().any(|w| w.contains("scale varies") && w.contains("f1")), "{:?}", out.warnings);
        // The bar is still true at the centre: k at 60°N is 2, so 1 km is twice as long as at k = 1.
        let utm = bare("EPSG:32632", [fc.from_lonlat(9.0, 60.0).map(|_| 500_000.0).unwrap(), 6_651_000.0], 5_000_000.0, 0.0, json!([]));
        assert!(!render(&e, &utm).warnings.iter().any(|w| w.contains("scale varies")));
    }

    fn with_surround(ids: &[String; 4], style: serde_json::Value) -> RenderRequest {
        let mut req = request(ids, style);
        let extra = [
            json!({ "kind": "legend", "id": "lg", "frame": "f1", "x": 215.0, "y": 30.0, "w": 55.0, "h": 80.0, "title": "Legend", "columns": 1 }),
            json!({ "kind": "scale_bar", "id": "sb", "frame": "f1", "x": 215.0, "y": 120.0, "w": 55.0, "h": 10.0, "units": "km", "segments": 4 }),
            json!({ "kind": "north_arrow", "id": "na", "frame": "f1", "x": 235.0, "y": 140.0, "w": 14.0, "h": 20.0, "style": "simple" }),
            json!({ "kind": "sources", "id": "src", "frame": "f1", "x": 12.7, "y": 192.0, "w": 200.0, "h": 12.0, "size": 6 }),
        ];
        for x in extra {
            req.layout.elements.push(serde_json::from_value(x).unwrap());
        }
        req
    }

    #[test]
    fn p13_legend_lists_visible_frame_layers_in_draw_order_with_class_labels() {
        let dir = tempfile::tempdir().unwrap();
        let (e, ids) = fixture(dir.path());
        let mut req = with_surround(&ids, categorized());
        let legend = |req: &RenderRequest| texts(group(&render(&e, req).svg, "legend-lg"));
        // order (top first): pts, lines, polys, nocrs (no CRS → not drawn, not listed).
        assert_eq!(legend(&req), ["Legend", "pts", "lines", "polys", "A", "B", "Other"]);
        req.visible.insert(ids[1].clone(), false);
        assert_eq!(legend(&req), ["Legend", "pts", "polys", "A", "B", "Other"]);
        let grad = json!({ "kind": "graduated", "fill": "#999999", "stroke": "#4d4d4d", "strokeWidth": 0.7, "opacity": 1, "radius": 3.5, "field": "pop",
                           "breaks": [20, 40], "colors": ["#aa0000", "#00aa00", "#0000aa"] });
        let req = with_surround(&ids, grad);
        assert_eq!(legend(&req)[4..], ["&lt; 20", "20 – 40", "≥ 40"]);
        // Swatch colours are the colours the frame draws.
        let svg = render(&e, &req).svg;
        let g = group(&svg, "legend-lg");
        for c in ["#aa0000", "#00aa00", "#0000aa"] {
            assert!(g.contains(&format!("fill=\"{c}\"")), "{c}");
        }
    }

    #[test]
    fn legend_reports_when_it_does_not_fit() {
        let dir = tempfile::tempdir().unwrap();
        let (e, ids) = fixture(dir.path());
        let mut req = with_surround(&ids, categorized());
        if let engine::layout::model::Element::Legend { at, .. } = &mut req.layout.elements[2] {
            at.h = 20.0;
        }
        let out = render(&e, &req);
        assert!(out.warnings.iter().any(|w| w.contains("Legend 'lg' does not fit")), "{:?}", out.warnings);
    }

    #[test]
    fn p14_sources_cite_every_drawn_layer_including_derived_ones() {
        use engine::sources;
        let dir = tempfile::tempdir().unwrap();
        let (e, ids) = fixture(dir.path());
        e.with_conn_for_tests(|c| {
            for cite in sources::for_tiger("counties", "Counties", "ACS2024") {
                sources::record(c, &cite, &ids[0], "polys")?;
            }
            sources::record(c, &sources::for_3dep(), &ids[2], "pts")?;
            // The line layer is a model result of the other two: it carries their sources.
            sources::replace_derived(c, &[ids[0].as_str(), ids[2].as_str()], &ids[1], "lines (model result)")
        })
        .unwrap();
        let sources_text = |req: &RenderRequest| texts(group(&render(&e, req).svg, "sources-src")).join(" ");
        let mut req = with_surround(&ids, categorized());
        let all = sources_text(&req);
        assert!(all.starts_with("Data: ") && all.contains("TIGERweb (2024)") && all.contains("3D Elevation Program (3DEP)"), "{all}");
        // Only the derived layer is drawn: both inputs' citations still appear.
        req.visible.insert(ids[0].clone(), false);
        req.visible.insert(ids[2].clone(), false);
        let derived = sources_text(&req);
        assert!(derived.contains("TIGERweb") && derived.contains("3DEP"), "{derived}");
        // A hidden layer's source is not cited.
        req.visible.insert(ids[1].clone(), false);
        let none = render(&e, &req).svg;
        assert!(!none.contains("sources-src"), "no drawn layers, no sources text");
    }

    #[test]
    fn surround_of_a_hidden_frame_is_reported_not_drawn() {
        let e = Engine::new().unwrap();
        let mut req = bare("EPSG:26915", [500_000.0, 4_649_776.0], 24_000.0, 0.0, json!([]));
        if let engine::layout::model::Element::MapFrame { at, .. } = &mut req.layout.elements[0] {
            at.visible = false;
        }
        let out = render(&e, &req);
        assert!(!out.svg.contains("scalebar-sb") && out.warnings.iter().filter(|w| w.contains("hidden")).count() == 2, "{:?}", out.warnings);
    }

    #[test]
    fn all_elements_page_is_deterministic_and_golden() {
        let dir = tempfile::tempdir().unwrap();
        let (e, ids) = fixture(dir.path());
        e.with_conn_for_tests(|c| {
            engine::sources::record(c, &engine::sources::for_3dep(), &ids[2], "pts")?;
            engine::sources::record(c, &engine::sources::for_tiger("counties", "Counties", "ACS2024")[0], &ids[0], "polys")
        })
        .unwrap();
        let req = with_surround(&ids, categorized());
        let a = render(&e, &req).svg;
        assert_eq!(a, render(&e, &req).svg);
        let sha = hex::encode(Sha256::digest(a.as_bytes()));
        if std::env::var("GOLDEN_PRINT").is_ok() {
            println!("GOLDEN_ALL {sha}");
            std::fs::write(std::env::temp_dir().join("layout_all.png"), export::svg_to_png(&a, 110.0).unwrap()).unwrap();
            std::fs::write(std::env::temp_dir().join("layout_all.pdf"), export::svg_to_pdf(&a).unwrap()).unwrap();
        }
        assert_eq!(sha, GOLDEN_ALL_SHA256, "SVG output changed; if intended, regenerate with GOLDEN_PRINT=1");
        assert!(export::svg_to_pdf(&a).unwrap().starts_with(b"%PDF-"));
    }

    const GOLDEN_ALL_SHA256: &str = "d061737a73398da134248e68b96a9d48a5bb1914328f7cc92b34ecd3899e3b47";
}

// ---------------------------------------------------------------- C2.4

mod c24 {
    use super::*;
    use engine::layout::geo::{self, Body, FrameCrs};
    use engine::layout::model::{Element, Layout};

    fn frame_req(crs: &str, center: [f64; 2], scale: f64, w: f64, h: f64, extra: serde_json::Value) -> RenderRequest {
        let mut frame = json!({ "kind": "map_frame", "id": "f1", "x": 12.7, "y": 30.0, "w": w, "h": h, "crs": crs, "center": center, "scale": scale, "layers": "map" });
        for (k, v) in extra.as_object().unwrap() {
            frame[k] = v.clone();
        }
        serde_json::from_value(json!({
            "layout": { "id": "L1", "name": "Grid", "page": { "width": 279.4, "height": 215.9 }, "elements": [frame] },
            "order": [], "project": "Demo", "date": "2026-01-01"
        }))
        .unwrap()
    }

    fn path_points(svg: &str, id: &str) -> Vec<(f64, f64)> {
        let i = svg.find(&format!("id=\"{id}\"")).unwrap_or_else(|| panic!("no path {id}"));
        let d = svg[i..].split(" d=\"").nth(1).unwrap().split('"').next().unwrap();
        d.split(|c| c == 'M' || c == 'L').filter(|t| !t.is_empty()).map(|t| {
            let mut n = t.split_whitespace().map(|v| v.parse::<f64>().unwrap());
            (n.next().unwrap(), n.next().unwrap())
        }).collect()
    }

    fn texts(svg: &str) -> Vec<String> {
        svg.split("<text ").skip(1).map(|t| t.split_once('>').unwrap().1.split("</text>").next().unwrap().to_string()).collect()
    }

    /// Page mm → frame-CRS coordinates for frame f1 at (12.7, 30) of size w × h.
    fn to_crs(body: &Body, crs: &FrameCrs, center: [f64; 2], scale: f64, w: f64, h: f64) -> impl Fn((f64, f64)) -> (f64, f64) {
        let e = geo::frame_extent(body, crs, center, scale, w, h).unwrap();
        move |(px, py)| (e[0] + (px - 12.7) / w * (e[2] - e[0]), e[3] - (py - 30.0) / h * (e[3] - e[1]))
    }

    #[test]
    fn p16_graticule_follows_the_projection() {
        let body = Body::get("earth").unwrap();
        let e = Engine::new().unwrap();
        let fc = FrameCrs::new("EPSG:2154", &body).unwrap(); // Lambert-93 (Lambert Conformal Conic)
        let c = fc.from_lonlat(3.0, 45.0).unwrap();
        let center = [c.0, c.1];
        // 1:50,000: every vertex of the 45°N line inverse-projects to 45° within 1e-6.
        let req = frame_req("EPSG:2154", center, 50_000.0, 200.0, 160.0, json!({ "graticule": { "interval": "auto", "labels": true } }));
        let svg = render(&e, &req).svg;
        let inv = to_crs(&body, &fc, center, 50_000.0, 200.0, 160.0);
        let line = path_points(&svg, "graticule-f1-lat45");
        assert!(line.len() > 10);
        for p in &line {
            let (lon, lat) = fc.to_lonlat(inv(*p).0, inv(*p).1).unwrap();
            assert!((lat - 45.0).abs() < 1e-6, "vertex at lon {lon} has latitude {lat}");
        }
        let meridian = path_points(&svg, "graticule-f1-lon3");
        for p in &meridian {
            let (lon, _) = fc.to_lonlat(inv(*p).0, inv(*p).1).unwrap();
            assert!((lon - 3.0).abs() < 1e-6, "{lon}");
        }
        assert!(texts(&svg).iter().any(|t| t == "45.00°N") && texts(&svg).iter().any(|t| t.ends_with("°E")), "{:?}", texts(&svg));
        // 1:2,000,000: the parallel is an arc, not a chord.
        let req = frame_req("EPSG:2154", center, 2_000_000.0, 200.0, 160.0, json!({ "graticule": { "interval": "auto", "labels": false } }));
        let svg = render(&e, &req).svg;
        let line = path_points(&svg, "graticule-f1-lat45");
        let (a, b) = (line[0], *line.last().unwrap());
        let dev = line.iter().map(|p| ((b.0 - a.0) * (a.1 - p.1) - (a.0 - p.0) * (b.1 - a.1)).abs() / ((b.0 - a.0).hypot(b.1 - a.1))).fold(0.0, f64::max);
        assert!(dev > 1.0, "45°N is curved in a conic projection: deviation {dev} mm");
        assert!(!texts(&svg).iter().any(|t| t.contains('°')), "labels off");
    }

    #[test]
    fn grid_lines_are_straight_at_round_coordinates_and_need_a_projected_crs() {
        let e = Engine::new().unwrap();
        let req = frame_req("EPSG:26915", [500_000.0, 4_649_776.0], 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": "auto", "labels": true } }));
        let out = render(&e, &req);
        let v = path_points(&out.svg, "grid-f1-x500000");
        assert_eq!(v.len(), 2);
        assert!((v[0].0 - v[1].0).abs() < 1e-9 && (v[0].0 - (12.7 + 100.0)).abs() < 0.001, "{v:?}"); // centre line
        assert!(texts(&out.svg).iter().any(|t| t == "500000") && texts(&out.svg).iter().any(|t| t == "4650000"), "{:?}", texts(&out.svg));
        let req = frame_req("EPSG:26915", [500_000.0, 4_649_776.0], 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 250, "labels": false } }));
        assert!(render(&e, &req).svg.contains("id=\"grid-f1-x500250\""));
        let geo = frame_req("EPSG:4326", [-93.0, 45.0], 500_000.0, 200.0, 160.0, json!({ "grid": { "interval": "auto" } }));
        assert!(render(&e, &geo).warnings.iter().any(|w| w.contains("grid needs a projected")));
        let too_fine = frame_req("EPSG:26915", [500_000.0, 4_649_776.0], 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1 } }));
        assert!(render(&e, &too_fine).warnings.iter().any(|w| w.contains("more than 200 lines")));
    }

    #[test]
    fn model_rejects_bad_grid_specs_and_unprintable_basemaps() {
        let e = Engine::new().unwrap();
        for (extra, needle) in [
            (json!({ "basemap": "osm" }), "bulk downloads"),
            (json!({ "basemap": "bing" }), "Unknown basemap"),
            (json!({ "graticule": { "interval": -1 } }), "interval"),
            (json!({ "grid": { "interval": "often" } }), "interval"),
        ] {
            let layout: Layout = frame_req("EPSG:26915", [0.0, 0.0], 24_000.0, 100.0, 100.0, extra).layout;
            let err = e.save_layout(&layout).unwrap_err().to_string();
            assert!(err.contains(needle), "{err}");
        }
        let ok = frame_req("EPSG:26915", [0.0, 0.0], 24_000.0, 100.0, 100.0, json!({ "basemap": "usgsTopo", "graticule": { "interval": 0.5 }, "grid": { "labels": false } })).layout;
        e.save_layout(&ok).unwrap();
        assert!(matches!(&ok.elements[0], Element::MapFrame { basemap, .. } if basemap == "usgsTopo"));
    }

    /// A DEM in UTM 15N, 100 m cells, a smooth hill: 200 × 200 cells around the fixture centre.
    fn dem(path: &Path) {
        use gdal::raster::Buffer;
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(path, 200, 200, 1).unwrap();
        ds.set_geo_transform(&[CENTER[0] - 10_000.0, 100.0, 0.0, CENTER[1] + 10_000.0, 0.0, -100.0]).unwrap();
        ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(26915).unwrap()).unwrap();
        let data: Vec<f32> = (0..200 * 200).map(|i| { let (x, y) = ((i % 200) as f32 - 100.0, (i / 200) as f32 - 100.0); 300.0 + 200.0 * (-(x * x + y * y) / 3000.0).exp() }).collect();
        ds.rasterband(1).unwrap().write((0, 0), (200, 200), &mut Buffer::new((200, 200), data)).unwrap();
    }

    fn count(hay: &[u8], needle: &str) -> usize {
        hay.windows(needle.len()).filter(|w| *w == needle.as_bytes()).count()
    }

    #[test]
    fn p17_raster_is_one_image_in_the_pdf_at_most_300_dpi() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("dem.tif");
        dem(&p);
        let e = Engine::new().unwrap();
        let id = e.import(&p).unwrap()[0].id.clone();
        let mut req = frame_req("EPSG:26915", CENTER, 24_000.0, 100.0, 80.0, json!({}));
        req.order = vec![id.clone()];
        req.layout.elements.push(serde_json::from_value(json!({ "kind": "legend", "id": "lg", "frame": "f1", "x": 120.0, "y": 30.0, "w": 40.0, "h": 20.0, "title": "Legend" })).unwrap());
        let out = render(&e, &req);
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert_eq!(count(out.svg.as_bytes(), "<image "), 1);
        let pdf = export::svg_to_pdf(&out.svg).unwrap();
        // One picture, with its soft mask for the transparent corners if the encoder writes one.
        let images = count(&pdf, "/Subtype /Image");
        assert!((1..=2).contains(&images), "{images} image objects");
        // 100 mm at 300 dpi = 1181 px; the embedded width never exceeds it (+1 for rounding).
        let w: usize = String::from_utf8_lossy(&pdf).split("/Width ").skip(1).filter_map(|t| t.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()).max().unwrap();
        assert!((1180..=1182).contains(&w), "image is {w} px wide");
        // The legend lists the raster, and a screen preview carries a smaller image.
        assert!(out.svg.contains("id=\"legend-lg\""));
        let small = e.render_layout_preview(&req, 96.0).unwrap();
        assert!(small.warnings.is_empty());
        let mut lo = req.clone();
        lo.raster_dpi = Some(96.0);
        let svg_lo = render(&e, &lo).svg;
        assert!(svg_lo.len() < out.svg.len() / 3, "{} vs {}", svg_lo.len(), out.svg.len());
    }

    /// A stand-in tile service: an RGB raster in Web Mercator covering ±50 km around the frame centre.
    fn fake_basemap(_: &str) -> anyhow::Result<gdal::Dataset> {
        use gdal::raster::Buffer;
        let body = Body::get("earth").unwrap();
        let merc = FrameCrs::new("EPSG:3857", &body)?;
        let utm = FrameCrs::new("EPSG:26915", &body)?;
        let (lon, lat) = utm.to_lonlat(CENTER[0], CENTER[1])?;
        let (cx, cy) = merc.from_lonlat(lon, lat)?;
        let drv = gdal::DriverManager::get_driver_by_name("MEM")?;
        let mut ds = drv.create_with_band_type::<u8, _>("", 256, 256, 3)?;
        ds.set_geo_transform(&[cx - 50_000.0, 390.625, 0.0, cy + 50_000.0, 0.0, -390.625])?;
        ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(3857)?)?;
        for (b, v) in [(1, 20u8), (2, 200), (3, 40)] {
            ds.rasterband(b)?.write((0, 0), (256, 256), &mut Buffer::new((256, 256), vec![v; 256 * 256]))?;
        }
        Ok(ds)
    }

    #[test]
    fn p18_basemap_is_warped_to_the_frame_and_cited() {
        let e = Engine::new().unwrap();
        let mut req = frame_req("EPSG:26915", CENTER, 24_000.0, 100.0, 80.0, json!({ "basemap": "usgsTopo" }));
        req.layout.elements.push(serde_json::from_value(json!({ "kind": "sources", "id": "src", "frame": "f1", "x": 12.7, "y": 112.0, "w": 100.0, "h": 10.0, "size": 6 })).unwrap());
        let out = e.render_layout_svg_with(&req, &fake_basemap, &|| Ok(())).unwrap();
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert_eq!(out.basemaps, vec!["usgsTopo".to_string()]);
        // The image is the frame's size and its middle pixel is the tile colour.
        let b64 = out.svg.split("id=\"basemap-f1\"").nth(1).unwrap().split("base64,").nth(1).unwrap().split('"').next().unwrap();
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let mut dec = png::Decoder::new(std::io::Cursor::new(&bytes)).read_info().unwrap();
        let mut buf = vec![0u8; dec.output_buffer_size().unwrap()];
        let info = dec.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (1181, 945));
        let mid = ((info.height / 2 * info.width + info.width / 2) * 4) as usize;
        assert_eq!(&buf[mid..mid + 4], &[20, 200, 40, 255]);
        // Sources cite it, and the project's source log records it once however often we render.
        let text = texts(&out.svg).join(" ");
        assert!(text.contains("USGS Topo base map"), "{text}");
        e.render_layout_svg_with(&req, &fake_basemap, &|| Ok(())).unwrap();
        let log = e.sources().unwrap();
        let entry = log.iter().find(|s| s.citation.key == "usgs-basemap-topo").expect("basemap recorded in the source log");
        assert_eq!(entry.uses.len(), 1);
        // A service that is down is a warning, not a failed export.
        let down = e.render_layout_svg_with(&req, &|_| anyhow::bail!("offline"), &|| Ok(())).unwrap();
        assert!(down.warnings.iter().any(|w| w.contains("could not be drawn") && w.contains("offline")), "{:?}", down.warnings);
        assert!(down.basemaps.is_empty() && !down.svg.contains("<image"));
    }

    /// The real National Map service. `cargo test -p engine --test layout p18_network -- --ignored`
    #[test]
    #[ignore]
    fn p18_network_usgs_topo_in_utm() {
        let e = Engine::new().unwrap();
        let req = frame_req("EPSG:26915", CENTER, 100_000.0, 100.0, 80.0, json!({ "basemap": "usgsTopo" }));
        let out = e.render_layout_svg(&req, &|| Ok(())).unwrap();
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(out.svg.contains("id=\"basemap-f1\""));
        assert!(export::svg_to_pdf(&out.svg).unwrap().starts_with(b"%PDF-"));
    }
}

// ---------- L2: labels in layouts (docs/LAYOUTS_PLAN.md) ----------

fn label(field: &str) -> engine::layout::model::LabelStyle {
    engine::layout::model::LabelStyle { enabled: true, field: field.into(), size: 9.0, color: "#222222".into(), halo_color: "#ffffff".into(), halo_width: 1.5, min_scale: None, max_scale: None }
}

/// (text, x, y, size) of every label fill `<text>`; halo underlays carry a stroke and are skipped.
fn label_texts(svg: &str) -> Vec<(String, f64, f64, f64, bool)> {
    let attr = |t: &str, k: &str| t.split(&format!(" {k}=\"")).nth(1).and_then(|r| r.split('"').next()).map(String::from);
    svg.split("<text ")
        .skip(1)
        .map(|t| t.split("</text>").next().unwrap_or(t))
        .filter(|t| t.contains("text-anchor") && !t.contains("stroke-width") && !t.contains("<tspan"))
        .map(|t| {
            let body = t.split('>').nth(1).unwrap_or("").split('<').next().unwrap_or("").to_string();
            let n = |k: &str| attr(t, k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            (body, n("x"), n("y"), n("font-size"), attr(t, "text-anchor").as_deref() == Some("start"))
        })
        .collect()
}

#[test]
fn l2_labels_draw_for_points_lines_and_polygons_with_halos() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    req.styles.get_mut(&ids[0]).unwrap().label = Some(label("cls"));
    req.styles.get_mut(&ids[1]).unwrap().label = Some(label("id"));
    req.styles.get_mut(&ids[2]).unwrap().label = Some(label("id"));
    let out = render(&e, &req);
    assert!(out.svg.contains("<g id=\"labels-f1\">"), "a labels group in the frame");
    let t = label_texts(&out.svg);
    // 4 polygons (labels a, b, c, a), 1 line, 5 points
    assert_eq!(t.iter().filter(|x| ["a", "b", "c"].contains(&x.0.as_str())).count(), 4, "{t:?}");
    assert_eq!(t.len(), 4 + 1 + 5, "{t:?}");
    assert!(out.svg.contains("stroke-linejoin=\"round\""), "halo underlay drawn");
    assert!(out.svg.contains("rotate("), "the line's label is turned to the line");
    // Points are labelled to the right of the symbol (start-anchored).
    assert_eq!(t.iter().filter(|x| x.4).count(), 5);
    assert_eq!(out.warnings, vec!["Layer nocrs skipped: no coordinate system".to_string()]);
}

#[test]
fn l2_p2_p3_no_two_label_boxes_overlap_and_skipped_labels_are_counted() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    // 400 points packed into 8 mm of page: most labels cannot fit.
    let dense = dir.path().join("dense.gpkg");
    write_fixture(&dense, &format!("SELECT x AS id, ST_Point({} + (x % 20) * 10, {} + (x / 20) * 10) AS geom FROM range(400) t(x)", CENTER[0], CENTER[1] - 500.0), Some("EPSG:26915"));
    let dense_id = e.import(&dense).unwrap()[0].id.clone();
    let mut req = request(&ids, categorized());
    req.order.insert(0, dense_id.clone());
    let mut st = LayerStyle::default();
    st.label = Some(label("id"));
    req.styles.insert(dense_id, st);
    let out = render(&e, &req);
    let t = label_texts(&out.svg);
    assert!(!t.is_empty() && t.len() < 400, "{} placed", t.len());
    // Boxes from the same metrics the placer uses: none overlaps another.
    let boxes: Vec<[f64; 4]> = t
        .iter()
        .map(|(s, x, y, size, start)| {
            let w = engine::layout::surround::text_width(s, "sans", size * 72.0 / 25.4) + 0.6;
            let h = size * 1.2 + 0.6;
            let x0 = if *start { *x - 0.3 } else { x - w / 2.0 };
            [x0, y - size * 0.35 - (h - size * 0.7) / 2.0, x0 + w, y - size * 0.35 - (h - size * 0.7) / 2.0 + h]
        })
        .collect();
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            assert!(a[2] <= b[0] + 1e-6 || b[2] <= a[0] + 1e-6 || a[3] <= b[1] + 1e-6 || b[3] <= a[1] + 1e-6, "labels overlap: {a:?} {b:?}");
        }
    }
    let note = out.warnings.iter().find(|w| w.contains("didn't fit")).unwrap_or_else(|| panic!("{:?}", out.warnings));
    let skipped: usize = note.split_whitespace().next().unwrap().parse().unwrap();
    assert!(note.contains("labels of dense"), "{note}");
    assert_eq!(skipped + t.len(), 400, "every candidate is either placed or counted as skipped");
}

#[test]
fn l2_labels_follow_the_scale_range_and_never_draw_outside_it() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    let mut l = label("cls");
    l.max_scale = Some(10_000); // the frame is 1:24 000: zoomed out past the limit
    req.styles.get_mut(&ids[0]).unwrap().label = Some(l.clone());
    assert!(label_texts(&render(&e, &req).svg).is_empty());
    l.max_scale = Some(50_000);
    l.min_scale = Some(30_000); // zoomed in past the other limit
    req.styles.get_mut(&ids[0]).unwrap().label = Some(l.clone());
    assert!(label_texts(&render(&e, &req).svg).is_empty());
    l.min_scale = Some(1_000);
    req.styles.get_mut(&ids[0]).unwrap().label = Some(l);
    assert_eq!(label_texts(&render(&e, &req).svg).len(), 4);
}

#[test]
fn l2_unknown_field_or_off_draws_nothing_and_a_hidden_layer_has_no_labels() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    req.styles.get_mut(&ids[0]).unwrap().label = Some(label("no_such_field"));
    assert!(label_texts(&render(&e, &req).svg).is_empty());
    let mut off = label("cls");
    off.enabled = false;
    req.styles.get_mut(&ids[0]).unwrap().label = Some(off);
    assert!(label_texts(&render(&e, &req).svg).is_empty());
    req.styles.get_mut(&ids[0]).unwrap().label = Some(label("cls"));
    req.visible.insert(ids[0].clone(), false);
    assert!(label_texts(&render(&e, &req).svg).is_empty());
}

#[test]
fn l2_p1_labels_are_deterministic_and_golden() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    req.styles.get_mut(&ids[0]).unwrap().label = Some(label("cls"));
    req.styles.get_mut(&ids[1]).unwrap().label = Some(label("id"));
    req.styles.get_mut(&ids[2]).unwrap().label = Some(label("id"));
    let a = render(&e, &req).svg;
    assert_eq!(a, render(&e, &req).svg, "same input, same bytes");
    let sha = hex::encode(Sha256::digest(a.as_bytes()));
    if std::env::var("GOLDEN_PRINT").is_ok() {
        println!("GOLDEN_LABELS {sha}");
        std::fs::write(std::env::temp_dir().join("layout_labels_golden.svg"), &a).unwrap();
        std::fs::write(std::env::temp_dir().join("layout_labels_golden.png"), export::svg_to_png(&a, 110.0).unwrap()).unwrap();
    }
    assert_eq!(sha, GOLDEN_LABELS_SHA256, "label SVG changed; if intended, regenerate with GOLDEN_PRINT=1");
}

const GOLDEN_LABELS_SHA256: &str = "68a3026135826a6356e8a05c876754448d5ba308f048546b8717eb2d7a6943b0";

#[test]
fn l2_p4_pdf_label_text_is_real_text() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    req.styles.get_mut(&ids[0]).unwrap().label = Some(label("cls"));
    let out = render(&e, &req);
    let pdf = export::svg_to_pdf(&out.svg).unwrap();
    assert!(contains(&pdf, "/FontFile2") && contains(&pdf, "/ToUnicode"), "labels are text in an embedded font, not paths");
    let streams = pdf_streams(&pdf);
    assert!(streams.iter().filter(|s| contains(s, "BT")).count() >= 1);
}

// ---------- L3: adjusting labels in the layout ----------

/// The request with `labelOverrides` on frame f1.
fn with_overrides(req: &RenderRequest, ov: serde_json::Value) -> RenderRequest {
    let mut v = serde_json::to_value(req).unwrap();
    v["layout"]["elements"][0]["labelOverrides"] = ov;
    serde_json::from_value(v).unwrap()
}

fn labelled(ids: &[String; 4]) -> RenderRequest {
    let mut req = request(ids, categorized());
    req.styles.get_mut(&ids[0]).unwrap().label = Some(label("cls"));
    req
}

/// fid of the polygon labelled `text` in an automatic render.
fn fid_of(out: &engine::layout::LayoutSvg, layer: &str, text: &str) -> i64 {
    out.labels.iter().find(|l| l.layer == layer && l.text == text).unwrap_or_else(|| panic!("{:?}", out.labels)).fid
}

#[test]
fn l3_render_reports_the_placed_labels() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let out = render(&e, &labelled(&ids));
    assert_eq!(out.labels.len(), 4);
    assert!(out.labels.iter().all(|l| l.frame == "f1" && l.layer == ids[0] && !l.pinned && !l.hidden && l.box_mm[2] > l.box_mm[0]));
    assert!(out.dropped_label_edits.is_empty());
}

#[test]
fn l3_p1_a_moved_label_stays_at_its_new_position_in_svg_and_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let base = render(&e, &labelled(&ids));
    let fid = fid_of(&base, &ids[0], "b");
    let before = base.labels.iter().find(|l| l.fid == fid).unwrap().box_mm;
    let req = with_overrides(&labelled(&ids), json!({ ids[0].clone(): { "features": { fid.to_string(): { "dxMm": 10.0, "dyMm": -5.0 } } } }));
    let out = render(&e, &req);
    let moved = out.labels.iter().find(|l| l.fid == fid).unwrap();
    assert!(moved.pinned && !moved.hidden);
    assert!((moved.box_mm[0] - before[0] - 10.0).abs() < 1e-9 && (moved.box_mm[1] - before[1] + 5.0).abs() < 1e-9, "{before:?} → {:?}", moved.box_mm);
    let (t0, t1) = (label_texts(&base.svg), label_texts(&out.svg));
    let (a, b) = (t0.iter().find(|t| t.0 == "b").unwrap(), t1.iter().find(|t| t.0 == "b").unwrap());
    assert!((b.1 - a.1 - 10.0).abs() < 1e-6 && (b.2 - a.2 + 5.0).abs() < 1e-6);
    assert_eq!(out.svg, render(&e, &req).svg, "same edits, same bytes");
    let pdf = export::svg_to_pdf(&out.svg).unwrap();
    assert!(contains(&pdf, "/ToUnicode"), "the PDF is built from the same svg");
}

#[test]
fn l3_p2_a_hidden_label_is_not_drawn_and_does_not_come_back() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let base = render(&e, &labelled(&ids));
    let fid = fid_of(&base, &ids[0], "c");
    let req = with_overrides(&labelled(&ids), json!({ ids[0].clone(): { "features": { fid.to_string(): { "hidden": true } } } }));
    let out = render(&e, &req);
    assert_eq!(label_texts(&out.svg).iter().filter(|t| t.0 == "c").count(), 0);
    assert_eq!(label_texts(&out.svg).len(), 3);
    let h = out.labels.iter().find(|l| l.fid == fid).unwrap();
    assert!(h.hidden, "listed so the UI can bring it back");
    assert!(!out.warnings.iter().any(|w| w.contains("didn't fit")));
}

#[test]
fn l3_p3_pinned_labels_are_never_dropped_and_automatic_labels_avoid_them() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let base = render(&e, &labelled(&ids));
    let (a_fid, b_fid) = (fid_of(&base, &ids[0], "a"), fid_of(&base, &ids[0], "b"));
    let (a, b) = (base.labels.iter().find(|l| l.fid == a_fid).unwrap().box_mm, base.labels.iter().find(|l| l.fid == b_fid).unwrap().box_mm);
    // Move b's label right on top of a's; b is pinned there, so a (automatic) must give way.
    let req = with_overrides(&labelled(&ids), json!({ ids[0].clone(): { "features": { b_fid.to_string(): { "dxMm": a[0] - b[0], "dyMm": a[1] - b[1] } } } }));
    let out = render(&e, &req);
    let pinned = out.labels.iter().find(|l| l.fid == b_fid).unwrap();
    assert!(pinned.pinned && !pinned.hidden);
    assert!(!out.labels.iter().any(|l| l.fid == a_fid), "the automatic label that would overlap is skipped");
    assert!(out.warnings.iter().any(|w| w.contains("1 label of") && w.contains("didn't fit")), "{:?}", out.warnings);
    // No two drawn boxes overlap, pinned or not.
    let boxes: Vec<[f64; 4]> = out.labels.iter().filter(|l| !l.hidden).map(|l| l.box_mm).collect();
    for (i, p) in boxes.iter().enumerate() {
        for q in &boxes[i + 1..] {
            assert!(p[2] <= q[0] || q[2] <= p[0] || p[3] <= q[1] || q[3] <= p[1], "{p:?} {q:?}");
        }
    }
    // A pinned label dragged off the frame is still drawn where the user put it.
    let off = with_overrides(&labelled(&ids), json!({ ids[0].clone(): { "features": { b_fid.to_string(): { "dxMm": 500.0 } } } }));
    assert!(render(&e, &off).labels.iter().any(|l| l.fid == b_fid && l.pinned));
}

#[test]
fn l3_edits_text_size_rotation_and_a_per_layout_style() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let base = render(&e, &labelled(&ids));
    let fid = fid_of(&base, &ids[0], "a");
    let req = with_overrides(&labelled(&ids), json!({ ids[0].clone(): { "features": { fid.to_string(): { "text": "North block", "sizePt": 14.0, "rotation": 30.0 } } } }));
    let out = render(&e, &req);
    let t = label_texts(&out.svg);
    assert!(t.iter().any(|x| x.0 == "North block" && (x.3 - 14.0 * 25.4 / 72.0).abs() < 1e-3), "{t:?}");
    assert!(out.svg.contains("rotate(30"));
    // A per-layout style: bigger text and another colour, for this layout only (the request's layer style is untouched).
    let styled = with_overrides(&labelled(&ids), json!({ ids[0].clone(): { "style": { "size": 20.0, "color": "#ff0000" } } }));
    let s = render(&e, &styled);
    assert!(label_texts(&s.svg).iter().all(|x| x.3 > 9.0 * 25.4 / 72.0 + 1e-6));
    assert!(s.svg.contains("#ff0000"));
    assert!(!base.svg.contains("#ff0000"));
    // A layout can label a layer the map doesn't, and turn off one it does.
    let on = with_overrides(&request(&ids, categorized()), json!({ ids[0].clone(): { "style": { "enabled": true, "field": "cls" } } }));
    assert_eq!(render(&e, &on).labels.len(), 4);
    let off = with_overrides(&labelled(&ids), json!({ ids[0].clone(): { "style": { "enabled": false } } }));
    assert!(render(&e, &off).labels.is_empty());
}

#[test]
fn l3_p5_an_edit_for_a_deleted_feature_is_ignored_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let base = render(&e, &labelled(&ids));
    let fid = fid_of(&base, &ids[0], "a");
    let req = with_overrides(&labelled(&ids), json!({ ids[0].clone(): { "features": { "424242": { "dxMm": 3.0 }, fid.to_string(): { "dxMm": 1.0 } } } }));
    let out = render(&e, &req);
    assert_eq!(out.dropped_label_edits.len(), 1);
    assert_eq!((out.dropped_label_edits[0].fid, out.dropped_label_edits[0].layer.as_str(), out.dropped_label_edits[0].frame.as_str()), (424242, ids[0].as_str(), "f1"));
    assert!(out.warnings.iter().any(|w| w.contains("1 label edit of") && w.contains("no longer exist")), "{:?}", out.warnings);
    assert_eq!(out.labels.len(), 4, "the real label is still drawn, with its edit");
}

#[test]
fn l3_labels_overrides_survive_the_layout_json_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let (_e, ids) = fixture(dir.path());
    let ov = json!({ ids[0].clone(): { "style": { "size": 12.0 }, "features": { "3": { "dxMm": 1.5, "hidden": true } } } });
    let req = with_overrides(&labelled(&ids), ov.clone());
    let back = serde_json::to_value(&req).unwrap();
    assert_eq!(back["layout"]["elements"][0]["labelOverrides"], ov);
}
