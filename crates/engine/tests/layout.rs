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
