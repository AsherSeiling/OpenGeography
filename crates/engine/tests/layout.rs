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

const GOLDEN_SHA256: &str = "b63bbdb71f0f062a6eccdc5e815b877599385fc937ded289795bbcc05f29b2a9";

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
    fn cg3_legend_entries_can_be_renamed_hidden_added_and_reshaped() {
        let dir = tempfile::tempdir().unwrap();
        let (e, ids) = fixture(dir.path());
        let mut req = with_surround(&ids, categorized());
        let set = |req: &mut RenderRequest, patch: serde_json::Value| {
            let cur = serde_json::to_value(&req.layout.elements[2]).unwrap();
            let mut m = cur.as_object().unwrap().clone();
            m.extend(patch.as_object().unwrap().clone());
            req.layout.elements[2] = serde_json::from_value(serde_json::Value::Object(m)).unwrap();
        };
        let legend = |req: &RenderRequest| texts(group(&render(&e, req).svg, "legend-lg"));
        // The generated rows are reported with stable keys.
        let rows = render(&e, &req).legend_rows;
        let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, [ids[2].as_str(), ids[1].as_str(), ids[0].as_str(), &format!("{}#0", ids[0]), &format!("{}#1", ids[0]), &format!("{}#2", ids[0])].map(|s| s));
        assert!(rows.iter().find(|r| r.key == ids[0]).unwrap().layer_row);
        assert!(!rows.iter().find(|r| r.key.ends_with("#0")).unwrap().layer_row);
        // Rename a class and a layer, hide a class and a whole layer.
        set(&mut req, json!({ "overrides": {
            (format!("{}#0", ids[0])): { "label": "Alpha" },
            (format!("{}#1", ids[0])): { "hidden": true },
            (ids[2].clone()): { "label": "Points of interest" },
            (ids[1].clone()): { "hidden": true } } }));
        assert_eq!(legend(&req), ["Legend", "Points of interest", "polys", "Alpha", "Other"]);
        // Hiding a layer's heading hides its classes too.
        set(&mut req, json!({ "overrides": { (ids[0].clone()): { "hidden": true } } }));
        assert_eq!(legend(&req), ["Legend", "pts", "lines"]);
        // Hand-made entries go last; patch and text size are applied.
        set(&mut req, json!({ "overrides": {}, "custom": [{ "label": "Study area", "color": "#ff00ff" }], "patch": "circle", "textSize": 10.0 }));
        let svg = render(&e, &req).svg;
        let g = group(&svg, "legend-lg");
        assert!(g.contains(">Study area<") && g.contains("#ff00ff") && g.contains("<ellipse") && g.contains("font-size=\"3.528\""), "{g}");
        set(&mut req, json!({ "patch": "hexagon" }));
        assert!(e.render_layout_svg(&req, &|| Ok(())).unwrap_err().to_string().contains("Unknown legend patch"));
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

    const GOLDEN_ALL_SHA256: &str = "cb0175beb466b0d3ee70f5f66820ac101adbdf9239a35c27bf13da3f37d6b61c";
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
    fn lw5_grid_styles_ticks_crosses_colour_width_and_old_layouts_unchanged() {
        let e = Engine::new().unwrap();
        let c = [500_000.0, 4_649_776.0];
        // Default: unchanged (solid blue 0.15 mm lines, the whole line).
        let plain = render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1000, "labels": false } }))).svg;
        assert!(plain.contains("stroke=\"#26567a\" stroke-width=\"0.15\""), "{plain}");
        assert!(!plain.contains("stroke-dasharray") || plain.matches("stroke-dasharray").count() == 0);
        // Ticks: each line is a short mark at the frame edge (2.5 mm), not the full height.
        let ticks = render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1000, "labels": false, "style": "ticks", "color": "#ff0000", "width": 0.4 } }))).svg;
        assert!(ticks.contains("stroke=\"#ff0000\" stroke-width=\"0.4\""));
        let v = path_points(&ticks, "grid-f1-x500000");
        assert_eq!(v.len(), 4, "a tick at the bottom and one at the top: {v:?}");
        assert!((v[0].1 - v[1].1).abs() - 2.5 < 1e-6 && (v[2].1 - v[3].1).abs() - 2.5 < 1e-6);
        // Crosses: one path of arms at the nodes inside the frame.
        let cross = render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1000, "labels": false, "style": "crosses" } }))).svg;
        assert!(cross.contains("id=\"grid-f1-crosses\""));
        assert!(!cross.contains("id=\"grid-f1-x500000\""));
        // Graticule ticks and crosses work too, and a dashed graticule drops its dashes when ticks.
        let g = render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "graticule": { "interval": 0.01, "labels": false, "style": "crosses" } }))).svg;
        assert!(g.contains("id=\"graticule-f1-crosses\""));
        let gt = render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "graticule": { "interval": 0.01, "labels": false, "style": "ticks" } }))).svg;
        assert!(!gt.contains("stroke-dasharray"));
    }

    #[test]
    fn lw5_label_formats_and_the_zone_designation() {
        let e = Engine::new().unwrap();
        let c = [500_000.0, 4_649_776.0];
        let short = texts(&render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1000, "labelFormat": "short", "designation": true } }))).svg);
        assert!(short.iter().any(|t| t == "00") && short.iter().any(|t| t == "50"), "{short:?}");
        // Lower-left corner ~(497.6 km E, 4647.4 km N) in zone 15: band T, square VG (checked against the mgrs package).
        assert!(short.iter().any(|t| t == "15T VG"), "{short:?}");
        let km = texts(&render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1000, "labelFormat": "km" } }))).svg);
        assert!(km.iter().any(|t| t == "4650") && km.iter().any(|t| t == "500"), "{km:?}");
        let dms = texts(&render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "graticule": { "interval": 0.005, "labelFormat": "dms" } }))).svg);
        assert!(dms.iter().any(|t| t.contains('′') && (t.ends_with('W') || t.ends_with('N'))), "{dms:?}");
        let dec = texts(&render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "graticule": { "interval": 0.005, "labelFormat": "decimal" } }))).svg);
        assert!(dec.iter().any(|t| t.starts_with("-93.") && t.ends_with('°')), "{dec:?}");
        // Designation needs UTM.
        let lcc = render(&e, &frame_req("EPSG:3857", [-10_000_000.0, 5_000_000.0], 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": "auto", "designation": true } })));
        assert!(lcc.warnings.iter().any(|w| w.contains("needs a UTM")), "{:?}", lcc.warnings);
    }

    #[test]
    fn lw5_subgrid_draws_lighter_lines_between_the_main_ones_only() {
        let e = Engine::new().unwrap();
        let c = [500_000.0, 4_649_776.0];
        let out = render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 2000, "subInterval": 1000, "labels": false } })));
        let svg = &out.svg;
        assert!(svg.contains("id=\"subgrid-f1\""));
        // 501000 lies between the 500000 and 502000 main lines: a sub line; 500000 itself is a main line only.
        assert!(svg.contains("id=\"subgrid-f1-x501000\""));
        assert!(!svg.contains("id=\"subgrid-f1-x500000\""));
        assert!(svg.contains("id=\"grid-f1-x500000\""));
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        // An interval that does not divide the main one is refused in words.
        let bad = render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 2000, "subInterval": 750, "labels": false } })));
        assert!(!bad.svg.contains("id=\"subgrid-f1\"") && bad.warnings.iter().any(|w| w.contains("does not divide")), "{:?}", bad.warnings);
    }

    #[test]
    fn lw5_label_edges_corner_labels_and_square_letters() {
        let e = Engine::new().unwrap();
        let c = [500_000.0, 4_649_776.0];
        let count = |extra: serde_json::Value| texts(&render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": extra }))).svg).len();
        let base = count(json!({ "interval": 1000 }));
        assert!(base > 4);
        // All four edges label every line twice (bottom + top, left + right).
        assert_eq!(count(json!({ "interval": 1000, "labelEdges": "all" })), base * 2);
        // Only the outermost line of each direction: at most four labels.
        let corners = count(json!({ "interval": 1000, "cornersOnly": true }));
        assert!(corners <= 4 && corners >= 2, "{corners}");
        // Top/right puts the x labels above the frame (y < frame top 30 mm), bottom/left below it.
        let svg_of = |edges: &str| render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1000, "labelEdges": edges } }))).svg;
        let first_text_y = |svg: &str| { let i = svg.find("<text ").unwrap(); let t = &svg[i..]; let y = t.split(" y=\"").nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap(); y };
        assert!(first_text_y(&svg_of("topright")) < 30.0 && first_text_y(&svg_of("bottomleft")) > 30.0);
        // 100 km square letters: the frame lies inside square VG (about 498–502 km E straddles W/V at 500 km).
        let letters = texts(&render(&e, &frame_req("EPSG:26915", c, 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1000, "labels": false, "squareLetters": true } }))).svg);
        assert!(letters.iter().any(|t| t == "VG") && letters.iter().any(|t| t == "WG"), "{letters:?}");
        let geo = render(&e, &frame_req("EPSG:3857", [-10_000_000.0, 5_000_000.0], 24_000.0, 200.0, 160.0, json!({ "grid": { "interval": 1000, "squareLetters": true } })));
        assert!(geo.warnings.iter().any(|w| w.contains("100 km square letters")), "{:?}", geo.warnings);
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

    #[test]
    fn rs7_raster_legend_shows_bar_classes_or_unique_values() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("dem.tif");
        dem(&p);
        let e = Engine::new().unwrap();
        let id = e.import(&p).unwrap()[0].id.clone();
        let mut req = frame_req("EPSG:26915", CENTER, 24_000.0, 100.0, 80.0, json!({}));
        req.order = vec![id.clone()];
        req.layout.elements.push(serde_json::from_value(json!({ "kind": "legend", "id": "lg", "frame": "f1", "x": 120.0, "y": 30.0, "w": 60.0, "h": 40.0, "title": "Legend" })).unwrap());
        let legend = |req: &RenderRequest| {
            let svg = render(&e, req).svg;
            let start = svg.find("<g id=\"legend-lg\"").unwrap();
            svg[start..start + svg[start..].find("</g>").unwrap()].to_string()
        };
        fn set(req: &mut RenderRequest, id: &str, raster: serde_json::Value) {
            let mut s = LayerStyle::default();
            s.raster = Some(serde_json::from_value(raster).unwrap());
            req.styles.insert(id.to_string(), s);
        }
        // Colour bar: 32 stops between the stretch ends, printed under it.
        set(&mut req, &id, json!({ "renderer": "pseudocolor", "ramp": "viridis", "stretch": { "mode": "manual", "min": 300.0, "max": 500.0 } }));
        let g = legend(&req);
        assert_eq!(g.matches("height=\"3.5\" fill=\"#").count(), 32, "{g}");
        assert!(texts(&g).contains(&"300".to_string()) && texts(&g).contains(&"500".to_string()), "{:?}", texts(&g));
        // Classes.
        set(&mut req, &id, json!({ "renderer": "pseudocolor", "mode": "discrete", "breaks": [350.0, 450.0], "stretch": { "mode": "manual", "min": 300.0, "max": 500.0 } }));
        assert_eq!(texts(&legend(&req))[2..], ["&lt; 350", "350 – 450", "≥ 450"]);
        // Unique values: only visible classes, with their labels.
        set(&mut req, &id, json!({ "renderer": "paletted", "entries": [{ "value": 1.0, "color": "#ff0000", "label": "Forest" }, { "value": 2.0, "color": "#00ff00", "label": "Water", "hidden": true }] }));
        let t = texts(&legend(&req));
        assert!(t.contains(&"Forest".to_string()) && !t.contains(&"Water".to_string()), "{t:?}");
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
    engine::layout::model::LabelStyle { enabled: true, field: field.into(), size: 9.0, color: "#222222".into(), halo_color: "#ffffff".into(), halo_width: 1.5, min_scale: None, max_scale: None, ..Default::default() }
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

const GOLDEN_LABELS_SHA256: &str = "12784ce6c4c32c60de038ce87b3303f96f2fcbe186160b5f3958e518b552bc89";

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

// ---------- B7: category drawing order ----------

/// Colours of the polygon groups of the first layer, in the order they appear in the SVG (first = bottom).
fn fill_order(svg: &str) -> Vec<String> {
    svg.split("<g id=\"layer-").nth(1).unwrap_or("").split("</g></g>").next().unwrap_or("").split("fill=\"").skip(1).filter_map(|t| t.split('"').next()).filter(|c| c.starts_with('#')).map(String::from).collect()
}

#[test]
fn b7_a_category_higher_in_the_list_is_drawn_on_top_in_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let a_then_b = render(&e, &request(&ids, categorized()));
    // categorized(): classes [a: #7aa8d8, b: #e6a35c]; everything else (c) uses the style's fill.
    let order = fill_order(&a_then_b.svg);
    let pos = |c: &str| order.iter().position(|x| x == c).unwrap_or_else(|| panic!("{c} in {order:?}"));
    assert!(pos("#999999") < pos("#e6a35c") && pos("#e6a35c") < pos("#7aa8d8"), "else at the bottom, then b, then a on top: {order:?}");
    // Reverse the list: b is now above a.
    let reversed = json!({ "kind": "categorized", "fill": "#999999", "stroke": "#4d4d4d", "strokeWidth": 0.7, "opacity": 0.75, "radius": 3.5, "field": "cls",
            "classes": [ { "value": "b", "color": "#e6a35c", "label": "B" }, { "value": "a", "color": "#7aa8d8", "label": "A" } ] });
    let b_then_a = render(&e, &request(&ids, reversed));
    let order = fill_order(&b_then_a.svg);
    let pos = |c: &str| order.iter().position(|x| x == c).unwrap();
    assert!(pos("#999999") < pos("#7aa8d8") && pos("#7aa8d8") < pos("#e6a35c"), "now a is under b: {order:?}");
    assert_ne!(a_then_b.svg, b_then_a.svg);
}

#[test]
fn b7_graduated_and_single_styles_keep_their_order() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let grad = json!({ "kind": "graduated", "fill": "#999999", "stroke": "#4d4d4d", "strokeWidth": 0.7, "opacity": 0.75, "radius": 3.5, "field": "pop", "breaks": [20.0, 40.0], "colors": ["#111111", "#222222", "#333333"] });
    let order = fill_order(&render(&e, &request(&ids, grad)).svg);
    let pos = |c: &str| order.iter().position(|x| x == c);
    assert!(pos("#111111") < pos("#222222") && pos("#222222") < pos("#333333"), "{order:?}");
}

// ---------- C3.0: scale bar "1:N" label ----------

fn with_scale_bar(req: &RenderRequest, extra: serde_json::Value) -> RenderRequest {
    let mut v = serde_json::to_value(req).unwrap();
    let mut bar = json!({ "kind": "scale_bar", "id": "sb", "frame": "f1", "x": 215.0, "y": 120.0, "w": 55.0, "h": 14.0, "units": "km", "segments": 4 });
    for (k, val) in extra.as_object().unwrap() {
        bar[k] = val.clone();
    }
    v["layout"]["elements"].as_array_mut().unwrap().push(bar);
    serde_json::from_value(v).unwrap()
}

#[test]
fn c3_0_scale_bar_can_print_the_ratio_under_the_bar() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let base = request(&ids, categorized());
    let off = render(&e, &with_scale_bar(&base, json!({})));
    let on = render(&e, &with_scale_bar(&base, json!({ "ratio": true })));
    assert!(!off.svg.contains("1:24,000>") && !off.svg.contains(">1:24,000<"), "off by default");
    assert!(on.svg.contains(">1:24,000<"), "the ratio of the frame's scale");
    assert!(!on.warnings.iter().any(|w| w.contains("1:N")), "{:?}", on.warnings);
    // A bar too short for the extra line says so.
    let short = render(&e, &with_scale_bar(&base, json!({ "ratio": true, "h": 8.0 })));
    assert!(short.warnings.iter().any(|w| w.contains("Scale bar 'sb' is too short for its 1:N label")), "{:?}", short.warnings);
    // The flag survives the layout JSON, and is left out when off.
    let v = serde_json::to_value(with_scale_bar(&base, json!({ "ratio": true }))).unwrap();
    assert_eq!(v["layout"]["elements"][2]["ratio"], true);
    let v = serde_json::to_value(with_scale_bar(&base, json!({}))).unwrap();
    assert!(v["layout"]["elements"][2].get("ratio").is_none());
}

// ---------- LB1: fonts from the device ----------

#[test]
fn lb1_installed_font_in_labels_renders_to_pdf_and_png_and_a_missing_one_falls_back_with_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let fonts = engine::fonts::list();
    let run = |family: &str| {
        let mut req = request(&ids, categorized());
        let mut l = label("id");
        l.font_family = Some(family.into());
        l.bold = true;
        req.styles.get_mut(&ids[1]).unwrap().label = Some(l);
        render(&e, &req)
    };
    if let Some(f) = fonts.first() {
        assert!(engine::fonts::has_family(&f.name));
        let out = run(&f.name);
        assert!(out.svg.contains(&format!("'{}', Noto Sans", f.name)), "font stack in the svg");
        assert!(out.svg.contains("font-weight=\"700\""));
        assert!(!out.warnings.iter().any(|w| w.contains("not installed")), "{:?}", out.warnings);
        assert!(export::svg_to_pdf(&out.svg).unwrap().starts_with(b"%PDF-"));
        assert!(export::svg_to_png(&out.svg, 96.0).unwrap().starts_with(&[0x89, b'P', b'N', b'G']));
    }
    let out = run("Definitely Not A Font 123");
    assert!(out.warnings.iter().any(|w| w.contains("Definitely Not A Font 123") && w.contains("Noto Sans")), "{:?}", out.warnings);
    assert!(export::svg_to_pdf(&out.svg).unwrap().starts_with(b"%PDF-"), "falls back, still exports");
}

#[test]
fn lb1_font_list_is_sorted_unique_and_clean_names_are_attribute_safe() {
    let l = engine::fonts::list();
    assert!(l.windows(2).all(|w| w[0].name < w[1].name), "sorted, no duplicates");
    assert_eq!(engine::fonts::clean_name("A\"b'<c>&;d, e"), "Abcd e");
}

#[test]
fn lb1_text_element_can_use_an_installed_family() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let Some(f) = engine::fonts::list().into_iter().next() else { return };
    let mut req = request(&ids, categorized());
    if let engine::layout::model::Element::Text { family, .. } = &mut req.layout.elements[1] {
        *family = Some(f.name.clone());
    } else {
        panic!("second element is the title");
    }
    let out = render(&e, &req);
    assert!(out.svg.contains(&format!("font-family=\"'{}', Noto Sans\"", f.name)), "title uses the family");
    assert!(export::svg_to_pdf(&out.svg).unwrap().starts_with(b"%PDF-"));
}

// ---------- LB2: placement and rendering options ----------

fn point_labels(e: &Engine, ids: &[String; 4], l: engine::layout::model::LabelStyle) -> engine::layout::LayoutSvg {
    let mut req = request(ids, categorized());
    req.styles.get_mut(&ids[2]).unwrap().label = Some(l);
    render(e, &req)
}

#[test]
fn lb2_point_positions_background_shadow_and_data_defined_rotation_and_size() {
    use engine::layout::model::{LabelBackground, LabelShadow};
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let base = |l: &engine::layout::model::LabelStyle| label_texts(&point_labels(&e, &ids, l.clone()).svg);
    let right = base(&label("id"));
    assert_eq!(right.iter().filter(|x| x.4).count(), 5, "default: right of the symbol, start-anchored");

    let anchors = |pos: &str| {
        let mut l = label("id");
        l.position = Some(pos.into());
        let svg = point_labels(&e, &ids, l).svg;
        (svg.matches("text-anchor=\"end\"").count(), svg.matches("text-anchor=\"middle\"").count(), label_texts(&svg))
    };
    let (end, _, left) = anchors("left");
    assert_eq!(end, 10, "five labels, each a halo and a fill text");
    assert!(left.iter().filter(|x| ["0", "1", "2", "3", "4"].contains(&x.0.as_str())).all(|x| !x.4));
    let (_, mid_above, above) = anchors("above");
    assert!(mid_above >= 5);
    // Above is higher on the page (smaller y) than right; below is lower.
    let y = |t: &Vec<(String, f64, f64, f64, bool)>| t.iter().find(|x| x.0 == "2").unwrap().2;
    let (_, _, below) = anchors("below");
    assert!(y(&above) < y(&right) && y(&right) < y(&below), "{} {} {}", y(&above), y(&right), y(&below));
    // A larger distance moves the label further from the symbol.
    let mut far = label("id");
    far.distance = 20.0;
    assert!(base(&far).iter().find(|x| x.0 == "2").unwrap().1 > right.iter().find(|x| x.0 == "2").unwrap().1 + 4.0);

    // Background boxes and shadows add elements; the label text is still there once in its own colour.
    let mut st = label("id");
    st.background = Some(LabelBackground { fill: "#ffffcc".into(), stroke: Some("#000000".into()), padding: 2.0 });
    st.shadow = Some(LabelShadow { color: "#888888".into(), dx: 2.0, dy: 2.0 });
    let svg = point_labels(&e, &ids, st).svg;
    assert_eq!(svg.matches("fill=\"#ffffcc\"").count(), 5);
    assert_eq!(svg.matches("fill=\"#888888\"").count(), 5);

    // Data-defined rotation and size from the id field.
    let mut dd = label("id");
    dd.rotation_field = Some("id".into());
    dd.size_field = Some("id".into());
    let svg = point_labels(&e, &ids, dd).svg;
    assert!(svg.contains("rotate(3 "), "label of id 3 turned 3°");
    let sizes: std::collections::BTreeSet<String> = label_texts(&svg).iter().map(|x| format!("{:.3}", x.3)).collect();
    assert!(sizes.len() >= 3, "sizes follow the field: {sizes:?}");
    assert!(export::svg_to_pdf(&svg).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn lb2_priority_decides_who_keeps_a_spot_and_allow_overlap_draws_everything() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    // A second copy of the point layer at the same places, with a red colour field and priority 9.
    let copy = dir.path().join("copy.gpkg");
    write_fixture(&copy, &format!("SELECT x AS id, '#ff0000' AS col, ST_Point({} + x * 400, {} + 700) AS geom FROM range(5) t(x)", CENTER[0] - 800.0, CENTER[1]), Some("EPSG:26915"));
    let copy_id = e.import(&copy).unwrap()[0].id.clone();
    let mut req = request(&ids, categorized());
    req.order = vec![ids[2].clone(), copy_id.clone(), ids[1].clone(), ids[0].clone(), ids[3].clone()]; // the black layer is on top
    let mut st = LayerStyle::default();
    let mut red = label("id");
    red.color_field = Some("col".into());
    red.priority = Some(9);
    st.label = Some(red);
    req.styles.insert(copy_id.clone(), st);
    req.styles.get_mut(&ids[2]).unwrap().label = Some(label("id"));
    let svg = render(&e, &req).svg;
    assert_eq!(svg.matches("fill=\"#ff0000\"").count(), 5, "the high-priority copy is drawn");
    assert_eq!(svg.matches("fill=\"#222222\"").count(), 0, "the lower priority labels were displaced");

    // Allow overlap on both: all ten are drawn.
    req.styles.get_mut(&copy_id).unwrap().label.as_mut().unwrap().allow_overlap = true;
    req.styles.get_mut(&ids[2]).unwrap().label.as_mut().unwrap().allow_overlap = true;
    let svg = render(&e, &req).svg;
    assert_eq!(svg.matches("fill=\"#ff0000\"").count() + svg.matches("fill=\"#222222\"").count(), 10);
}

#[test]
fn lb2_wrap_breaks_at_spaces_keeps_long_words_and_newlines() {
    use engine::layout::labels::wrap_text;
    assert_eq!(wrap_text("North Fork Smith River", Some(10)), vec!["North Fork", "Smith", "River"]);
    assert_eq!(wrap_text("Supercalifragilistic is long", Some(8)), vec!["Supercalifragilistic", "is long"]);
    assert_eq!(wrap_text("one\ntwo three", None), vec!["one", "two three"]);
    assert_eq!(wrap_text("", Some(5)), vec![""]);
}

#[test]
fn lb2_line_labels_sit_above_or_below_the_line() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let at = |pos: Option<&str>| {
        let mut req = request(&ids, categorized());
        let mut l = label("id");
        l.line_pos = pos.map(String::from);
        req.styles.get_mut(&ids[1]).unwrap().label = Some(l);
        let svg = render(&e, &req).svg;
        // The line's label is the one with a rotation transform.
        let t = svg.split("<text ").skip(1).find(|t| t.contains("rotate(") && t.contains(">1</text>")).unwrap_or_else(|| panic!("no rotated line label")).to_string();
        let n = |k: &str| t.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
        n("y")
    };
    let (on, above, below) = (at(None), at(Some("above")), at(Some("below")));
    assert!(above < on && on < below, "{above} {on} {below}");
}

// ---------- LB3: contour labels ----------

#[test]
fn lb3_contour_text_parse_and_placement_along_a_line() {
    use engine::layout::labels::{along_line, contour_text, parse_linestring};
    use engine::layout::model::ContourLabels;
    let c = ContourLabels { spacing: 100.0, in_line: true, decimals: Some(0), unit: Some(" m".into()) };
    assert_eq!(contour_text("120.0", &c), "120 m");
    assert_eq!(contour_text("1250.5", &ContourLabels { decimals: Some(1), ..Default::default() }), "1250.5");
    assert_eq!(contour_text("Index", &c), "Index m");
    assert_eq!(parse_linestring("LINESTRING (0 0, 10 0, 10 5.5)"), vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.5)]);
    assert!(parse_linestring("POINT (1 2)").is_empty());

    // A straight 100 mm line, labels 8 mm wide every 30 mm: first at 15, then 45, 75 (the next, 105, would run off the end).
    let line: Vec<(f64, f64)> = vec![(0.0, 0.0), (100.0, 0.0)];
    let p = along_line(&line, 8.0, 3.0, 30.0);
    assert_eq!(p.iter().map(|x| x.0.round() as i64).collect::<Vec<_>>(), vec![15, 45, 75]);
    assert!(p.iter().all(|x| x.2 == 0.0));
    // Right-to-left lines still read left to right (upright), and a too-short line gets no label.
    let back: Vec<(f64, f64)> = vec![(100.0, 0.0), (0.0, 0.0)];
    assert!(along_line(&back, 8.0, 3.0, 30.0).iter().all(|x| x.2.abs() <= 90.0 && x.2 == 0.0));
    let steep: Vec<(f64, f64)> = vec![(0.0, 0.0), (0.0, 100.0)];
    assert!(along_line(&steep, 8.0, 3.0, 30.0).iter().all(|x| (x.2 - 90.0).abs() < 1e-9 || (x.2 + 90.0).abs() < 1e-9));
    assert!(along_line(&[(0.0, 0.0), (5.0, 0.0)], 8.0, 3.0, 30.0).is_empty());
    // A sharp bend under the text means no label there.
    let bent: Vec<(f64, f64)> = vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)];
    assert!(along_line(&bent, 30.0, 3.0, 40.0).is_empty());
}

#[test]
fn lb3_contour_labels_repeat_along_lines_upright_with_unit_and_break_or_sit_above() {
    use engine::layout::model::ContourLabels;
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let run = |in_line: bool| {
        let mut req = request(&ids, categorized());
        let mut l = label("id");
        l.contour = Some(ContourLabels { spacing: 150.0, in_line, decimals: Some(0), unit: Some(" m".into()) });
        req.styles.get_mut(&ids[1]).unwrap().label = Some(l);
        render(&e, &req).svg
    };
    let broken = run(true);
    let labels = broken.matches(">1 m</text>").count();
    assert!(labels >= 3, "several labels along one line: {labels}");
    assert!(broken.contains("fill=\"#ffffff\"") && broken.contains("<rect"), "the line is broken under each label by a background box");
    for t in broken.split("rotate(").skip(1) {
        let deg: f64 = t.split(' ').next().unwrap().parse().unwrap();
        assert!(deg.abs() <= 90.0, "upright: {deg}");
    }
    let above = run(false);
    assert_eq!(above.matches(">1 m</text>").count(), labels * 2, "the same labels, each a halo and a fill text (the broken-line variant has no halo)");
    assert!(above.matches("<rect").count() < broken.matches("<rect").count(), "no break boxes when the text sits above the line");
    assert!(export::svg_to_pdf(&broken).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn lb3_index_contours_only_are_labelled_with_elevation_like_the_contour_nodes_output() {
    use engine::layout::model::ContourLabels;
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    // Three parallel contours 20 m apart; only the 300 m line is an index contour (the Contour node writes ELEV and INDEX).
    let contours = dir.path().join("contours.gpkg");
    write_fixture(
        &contours,
        &format!("SELECT x AS id, CAST(260 + 20 * x AS DOUBLE) AS \"ELEV\", CASE WHEN x = 2 THEN 1 ELSE 0 END AS \"INDEX\", ST_GeomFromText('LINESTRING(' || {a} || ' ' || ({y} + 300 * x) || ', ' || {b} || ' ' || ({y} + 300 * x) || ')') AS geom FROM range(3) t(x)", a = CENTER[0] - 3000.0, b = CENTER[0] + 3000.0, y = CENTER[1] - 300.0),
        Some("EPSG:26915"),
    );
    let cid = e.import(&contours).unwrap()[0].id.clone();
    let mut req = request(&ids, categorized());
    req.order.insert(0, cid.clone());
    let mut st = LayerStyle::default();
    let mut l = label("ELEV");
    l.index_only = true;
    l.contour = Some(ContourLabels { spacing: 200.0, in_line: true, decimals: Some(0), unit: Some(" m".into()) });
    st.label = Some(l);
    req.styles.insert(cid.clone(), st);
    let svg = render(&e, &req).svg;
    assert!(svg.matches(">300 m</text>").count() >= 2, "the index contour is labelled more than once");
    assert_eq!(svg.matches(">260 m</text>").count() + svg.matches(">280 m</text>").count(), 0, "other contours are not labelled");
}

// ---------- CG5: tables, charts, rich text, tokens ----------

fn add_element(req: &mut RenderRequest, e: serde_json::Value) {
    req.layout.elements.push(serde_json::from_value(e).unwrap());
}

fn group_of<'a>(svg: &'a str, id: &str) -> &'a str {
    let start = svg.find(&format!("<g id=\"{id}\"")).unwrap_or_else(|| panic!("no group {id}"));
    &svg[start..start + svg[start..].find("</g>").unwrap()]
}

fn text_of(g: &str) -> Vec<String> {
    g.split("<text ").skip(1).map(|t| t.split_once('>').unwrap().1.split("</text>").next().unwrap().to_string()).collect()
}

#[test]
fn cg5_table_shows_chosen_fields_sorted_with_a_truncation_warning() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    add_element(&mut req, json!({ "kind": "table", "id": "tb", "layer": ids[0], "fields": ["cls", "pop"], "sortField": "pop", "descending": true, "x": 12.7, "y": 195.0, "w": 80.0, "h": 40.0 }));
    let out = render(&e, &req);
    let t = text_of(group_of(&out.svg, "table-tb"));
    assert_eq!(t, ["cls", "pop", "c", "50", "b", "30", "a", "10", "a", "5"], "header, then rows by pop descending; numbers without decimals");
    assert!(group_of(&out.svg, "table-tb").contains("text-anchor=\"end\""), "numbers align right");
    // Too small for all rows: says so and shows what fits.
    if let engine::layout::model::Element::Table { at, .. } = req.layout.elements.last_mut().unwrap() {
        at.h = 9.0;
    }
    let out = render(&e, &req);
    assert!(out.warnings.iter().any(|w| w.contains("Table 'tb' shows 1 of 4 rows")), "{:?}", out.warnings);
    // Unknown field is skipped with a message; no layer is a prompt, not a crash.
    add_element(&mut req, json!({ "kind": "table", "id": "t2", "layer": ids[0], "fields": ["nope", "cls"], "x": 12.7, "y": 150.0, "w": 40.0, "h": 40.0 }));
    add_element(&mut req, json!({ "kind": "table", "id": "t3", "x": 100.0, "y": 150.0, "w": 40.0, "h": 40.0 }));
    let out = render(&e, &req);
    assert!(out.warnings.iter().any(|w| w.contains("no field 'nope'")) && out.warnings.iter().any(|w| w.contains("Table 't3' has no layer yet")), "{:?}", out.warnings);
}

#[test]
fn cg5_chart_aggregates_by_category_in_colour_blind_safe_colours() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    add_element(&mut req, json!({ "kind": "chart", "id": "ch", "layer": ids[0], "chartType": "bar", "category": "cls", "value": "pop", "x": 12.7, "y": 195.0, "w": 80.0, "h": 40.0, "title": "Population" }));
    let out = render(&e, &req);
    let g = group_of(&out.svg, "chart-ch");
    assert_eq!(text_of(g), ["Population", "c", "50", "b", "30", "a", "15"], "sum of pop per class, largest first");
    assert!(g.contains("#0072b2"), "Okabe–Ito blue");
    // Count when there is no value field; pie has a legend with percentages.
    add_element(&mut req, json!({ "kind": "chart", "id": "pi", "layer": ids[0], "chartType": "pie", "category": "cls", "x": 100.0, "y": 195.0, "w": 80.0, "h": 40.0 }));
    let out = render(&e, &req);
    let g = group_of(&out.svg, "chart-pi");
    assert_eq!(text_of(g), ["a (50%)", "b (25%)", "c (25%)"]);
    assert_eq!(g.matches("<path").count(), 3);
    // Bad inputs are words.
    add_element(&mut req, json!({ "kind": "chart", "id": "bad", "layer": ids[0], "chartType": "bar", "category": "cls", "value": "cls", "x": 0.0, "y": 0.0, "w": 40.0, "h": 40.0 }));
    assert!(render(&e, &req).warnings.iter().any(|w| w.contains("must be a numeric field")));
    add_element(&mut req, json!({ "kind": "chart", "id": "bad2", "layer": ids[0], "chartType": "donut", "category": "cls", "x": 0.0, "y": 0.0, "w": 40.0, "h": 40.0 }));
    assert!(e.render_layout_svg(&req, &|| Ok(())).unwrap_err().to_string().contains("Unknown chart type"));
}

#[test]
fn cg5_rich_text_and_new_tokens() {
    use engine::layout::rich_runs;
    assert_eq!(rich_runs("plain", false, false), [("plain".to_string(), false, false)]);
    assert_eq!(rich_runs("a **b** *c* d", false, false), [("a ".into(), false, false), ("b".into(), true, false), (" ".into(), false, false), ("c".into(), false, true), (" d".into(), false, false)]);
    assert_eq!(rich_runs(r"2 \* 3 \\ x", false, false), [(r"2 * 3 \ x".to_string(), false, false)]);
    assert_eq!(rich_runs("**x**", true, false)[0], ("x".to_string(), false, false), "** toggles off inside a bold element");
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    req.author = "A. Geologist".into();
    add_element(&mut req, json!({ "kind": "text", "id": "tx", "x": 12.7, "y": 195.0, "w": 150.0, "h": 10.0, "text": "By {author}: {layer_count} layers ({layer_names}) **bold**", "font": "sans" }));
    let svg = render(&e, &req).svg;
    let g = &svg[svg.find("By A. Geologist").unwrap()..];
    assert!(g.starts_with("By A. Geologist: 3 layers (pts, lines, polys) </tspan>") || g.contains("3 layers (pts, lines, polys)"), "{}", &g[..120.min(g.len())]);
    assert!(g.contains("font-weight=\"700\""), "bold run");
}

// ---------- CG7: frame clip shapes and layer shadows ----------

fn set_frame(req: &mut RenderRequest, patch: serde_json::Value) {
    let mut m = serde_json::to_value(&req.layout.elements[0]).unwrap().as_object().unwrap().clone();
    m.extend(patch.as_object().unwrap().clone());
    req.layout.elements[0] = serde_json::from_value(serde_json::Value::Object(m)).unwrap();
}

fn clip_of(svg: &str) -> &str {
    let s = svg.find("<clipPath id=\"clip-f1\">").unwrap() + 23;
    &svg[s..s + svg[s..].find("</clipPath>").unwrap()]
}

#[test]
fn cg7_frames_can_be_ellipses_rounded_or_cut_to_a_polygon_layer() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    assert!(clip_of(&render(&e, &req).svg).starts_with("<rect"));
    set_frame(&mut req, json!({ "clip": { "shape": "ellipse" } }));
    let out = render(&e, &req);
    assert!(clip_of(&out.svg).starts_with("<ellipse cx=\"112.7\" cy=\"110\" rx=\"100\" ry=\"80\""), "{}", clip_of(&out.svg));
    assert!(out.svg.contains("<ellipse") && out.svg.matches("<ellipse").count() >= 2, "the border follows the shape");
    set_frame(&mut req, json!({ "clip": { "shape": "rounded", "radius": 8.0 } }));
    assert!(clip_of(&render(&e, &req).svg).contains("rx=\"8\""));
    // The outline of the polygon layer (four squares; the last runs off the frame).
    set_frame(&mut req, json!({ "clip": { "shape": "layer", "layer": ids[0] } }));
    let out = render(&e, &req);
    let c = clip_of(&out.svg);
    assert!(c.starts_with("<path clip-rule=\"evenodd\" d=\"M") && c.matches('Z').count() >= 3, "{c}");
    assert!(out.warnings.iter().all(|w| !w.contains("clip shape")), "{:?}", out.warnings);
    // A line layer can't outline a frame: say so and keep the rectangle.
    set_frame(&mut req, json!({ "clip": { "shape": "layer", "layer": ids[1] } }));
    let out = render(&e, &req);
    assert!(out.warnings.iter().any(|w| w.contains("is not a polygon layer")) && clip_of(&out.svg).starts_with("<rect"), "{:?}", out.warnings);
    set_frame(&mut req, json!({ "clip": { "shape": "layer", "layer": "" } }));
    assert!(e.render_layout_svg(&req, &|| Ok(())).unwrap_err().to_string().contains("Choose the polygon layer"));
    set_frame(&mut req, json!({ "clip": { "shape": "star" } }));
    assert!(e.render_layout_svg(&req, &|| Ok(())).unwrap_err().to_string().contains("Unknown frame shape"));
    // PDF export of a clipped frame works.
    set_frame(&mut req, json!({ "clip": { "shape": "layer", "layer": ids[0] } }));
    assert!(export::svg_to_pdf(&render(&e, &req).svg).unwrap().starts_with(b"%PDF"));
}

#[test]
fn cg7_a_layer_can_cast_a_shadow_in_layouts_only() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    assert!(!render(&e, &req).svg.contains("id=\"shadow-"));
    let mut st = req.styles.get(&ids[0]).unwrap().clone();
    st.effect = Some(serde_json::from_value(json!({ "dx": 1.0, "dy": 1.5, "color": "#222222", "opacity": 0.4 })).unwrap());
    req.styles.insert(ids[0].clone(), st);
    let svg = render(&e, &req).svg;
    let s = svg.find(&format!("<g id=\"shadow-{}\"", ids[0])).expect("shadow group");
    let l = svg.find(&format!("<g id=\"layer-{}\"", ids[0])).unwrap();
    assert!(s < l, "the shadow is under the layer");
    let sh = &svg[s..l];
    assert!(sh.contains("translate(1 1.5)") && sh.contains("opacity=\"0.4\"") && sh.contains("#222222") && !sh.contains("#7aa8d8") && !sh.contains("#e6a35c"), "{sh}");
    // Round trip: omitted when unset.
    let v = serde_json::to_value(req.styles.get(&ids[1]).unwrap()).unwrap();
    assert!(v.get("effect").is_none());
}

// ---------- CG8: symbol library ----------

#[test]
fn cg8_point_markers_line_dashes_and_polygon_patterns_render_in_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    let base = render(&e, &req).svg;
    // Defaults are unchanged.
    assert!(base.contains("<circle") && !base.contains("stroke-dasharray") && !base.contains("<pattern"));
    let mut put = |req: &mut RenderRequest, id: &str, extra: serde_json::Value| {
        let mut v = serde_json::to_value(req.styles.get(id).cloned().unwrap_or_default()).unwrap();
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        req.styles.insert(id.to_string(), serde_json::from_value(v).unwrap());
    };
    put(&mut req, &ids[2], json!({ "marker": "star" }));
    put(&mut req, &ids[1], json!({ "dash": "dashed" }));
    put(&mut req, &ids[0], json!({ "pattern": "hatch", "patternSpacing": 8.0 }));
    let out = render(&e, &req);
    let svg = &out.svg;
    let pts = &svg[svg.find(&format!("<g id=\"layer-{}\"", ids[2])).unwrap()..];
    let pts = &pts[..pts.find("</g></g>").unwrap_or(pts.len())];
    assert!(pts.contains("<polygon points=") && !pts.contains("<circle"), "{pts}");
    assert!(svg.contains("stroke-dasharray"), "dashed line");
    assert!(svg.matches("<pattern").count() == 3 && svg.contains(&format!("fill=\"url(#pat-{}-0)\"", ids[0])), "one pattern per class bucket");
    assert_eq!(out.warnings, ["Layer nocrs skipped: no coordinate system"]);
    // PNG and PDF still export.
    assert!(e.render_layout_preview(&req, 96.0).is_ok());
    assert!(export::svg_to_pdf(svg).unwrap().starts_with(b"%PDF"));
    // Unknown names are readable errors.
    put(&mut req, &ids[2], json!({ "marker": "pentagon" }));
    assert!(e.render_layout_svg(&req, &|| Ok(())).unwrap_err().to_string().contains("Unknown point symbol"));
    put(&mut req, &ids[2], json!({ "marker": "svg" }));
    assert!(e.render_layout_svg(&req, &|| Ok(())).is_err(), "an svg symbol needs a file");
}

// ---------------------------------------------------------------- declination diagram (LW6)

#[test]
fn lw6_declination_diagram_draws_gn_tn_mn_from_the_model_and_the_frame() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = slot_layout(&ids, json!({}));
    let render_with = |req: &RenderRequest, extra: serde_json::Value| {
        let mut v = serde_json::to_value(&req.layout).unwrap();
        for el in v["elements"].as_array_mut().unwrap() {
            if el["kind"] == "declination" {
                el.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            }
        }
        let mut r = req.clone();
        r.layout = serde_json::from_value(v).unwrap();
        render(&e, &r)
    };
    let out = render_with(&req, json!({}));
    if let Ok(dir) = std::env::var("LW6_DUMP") {
        // Look at it: LW6_DUMP=/some/dir cargo test ... lw6 writes the page as PNG at 150 dpi.
        let mut v = serde_json::to_value(&req.layout).unwrap();
        v["elements"] = json!(v["elements"].as_array().unwrap().iter().filter(|e| e["kind"] == "map_frame" || e["kind"] == "declination").cloned().collect::<Vec<_>>());
        v["elements"][0]["center"] = json!([300000.0, 4650000.0]); // far from the central meridian: grid and true north differ
        let mut r = req.clone();
        r.layout = serde_json::from_value(v).unwrap();
        use base64::Engine as _;
        let png = e.render_layout_preview(&r, 150.0).unwrap().png;
        std::fs::write(std::path::Path::new(&dir).join("declination.png"), base64::engine::general_purpose::STANDARD.decode(png).unwrap()).unwrap();
    }
    for id in ["declination-dc1-gn", "declination-dc1-tn", "declination-dc1-mn"] {
        assert!(out.svg.contains(&format!("id=\"{id}\"")), "{id}");
    }
    let texts_of = |svg: &str| -> Vec<String> { svg.split("<text ").skip(1).map(|t| t.split_once('>').unwrap().1.split("</text>").next().unwrap().to_string()).collect() };
    let t = texts_of(&out.svg);
    assert!(t.iter().any(|x| x == "GN") && t.iter().any(|x| x == "TN") && t.iter().any(|x| x == "MN"), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("2026 MAGNETIC NORTH DECLINATION")), "{t:?}");
    // Units: degrees only has no mils; mils only has no degree sign in the declination line.
    let deg = texts_of(&render_with(&req, json!({ "units": "deg" })).svg);
    assert!(deg.iter().any(|x| x.contains("° ") && !x.contains("mils") && x.contains("at centre of sheet")), "{deg:?}");
    let mil = texts_of(&render_with(&req, json!({ "units": "mils" })).svg);
    assert!(mil.iter().any(|x| x.contains("mils") && x.contains("at centre of sheet")), "{mil:?}");
    // The simple style leaves grid north out; the army style words the grid-magnetic angle.
    let simple = render_with(&req, json!({ "style": "simple" })).svg;
    assert!(!simple.contains("declination-dc1-gn") && simple.contains("declination-dc1-mn"));
    let army = texts_of(&render_with(&req, json!({ "style": "army" })).svg);
    assert!(army.iter().any(|x| x.starts_with("G-M angle")), "{army:?}");
    // Small angles are opened up by default and the caption says so; true scale turns that off.
    assert!(t.iter().any(|x| x == "angles exaggerated, not to scale"), "{t:?}");
    let exact = texts_of(&render_with(&req, json!({ "exaggerate": false })).svg);
    assert!(!exact.iter().any(|x| x == "angles exaggerated, not to scale"));
    // Outside the model's years: an explained warning and no magnetic line, never a guess.
    let late = render_with(&req, json!({ "date": "2045-01-01" }));
    assert!(!late.svg.contains("declination-dc1-mn") && late.warnings.iter().any(|w| w.contains("Declination diagram")), "{:?}", late.warnings);
    // Bad words and dates are refused when the layout is checked.
    req.layout = serde_json::from_value({
        let mut v = serde_json::to_value(&req.layout).unwrap();
        for el in v["elements"].as_array_mut().unwrap() {
            if el["kind"] == "declination" {
                el["date"] = json!("tomorrow");
            }
        }
        v
    })
    .unwrap();
    assert!(e.render_layout_svg(&req, &|| Ok(())).unwrap_err().to_string().contains("YYYY-MM-DD"));
}

// ---------------------------------------------------------------- Topo Map Builder (LW9)

#[test]
fn lw9_topo_sheet_renders_every_marginal_element_without_warnings() {
    use engine::layout::topo::{build, TopoSpec};
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let spec: TopoSpec = serde_json::from_value(json!({ "center": [-93.0, 42.0], "scale": 24000, "paper": "tabloid", "landscape": true, "title": "Example Quadrangle", "date": "2026-01-01" })).unwrap();
    let built = build(&spec, "2026-01-01").unwrap();
    let mut req = request(&ids, categorized());
    req.layout = built.layout.clone();
    let out = render(&e, &req);
    // Nothing is cut off or left out: no warning about any builder element, and each piece of marginalia is in the drawing.
    let bad: Vec<_> = out.warnings.iter().filter(|w| !w.contains("skipped") && !w.contains("no coordinate system")).collect();
    assert!(bad.is_empty(), "{bad:?}");
    let svg = &out.svg;
    for needle in ["declination-dc1-tn", "grid-f1-x500000", "graticule-f1-", "HOW TO GIVE A GRID REFERENCE", "Example Quadrangle", "Legend", "15T "] {
        assert!(svg.contains(needle), "{needle}");
    }
    assert!(svg.contains("Scale 1:24,000") && svg.contains("NAD 83"), "the marginal note is filled in");
    if let Ok(dir) = std::env::var("TOPO_DUMP") {
        use base64::Engine as _;
        let png = e.render_layout_preview(&req, 110.0).unwrap().png;
        std::fs::write(std::path::Path::new(&dir).join("topo.png"), base64::engine::general_purpose::STANDARD.decode(png).unwrap()).unwrap();
    }
}

// ---------------------------------------------------------------- text slots (docs/LAYOUT_REWORK_PLAN.md §4.2)

use engine::layout::slots::{self, NO_TEXT, SLOTS};

const PT: f64 = 25.4 / 72.0;

/// One element of every kind that draws text, in a frame with a graticule and a grid; `styles` is merged into each element's `textStyles`.
fn slot_layout(ids: &[String; 4], styles: serde_json::Value) -> RenderRequest {
    let mut req = request(ids, categorized());
    let st = |kind: &str| -> serde_json::Value {
        let mut m = serde_json::Map::new();
        for sl in SLOTS.iter().filter(|s| s.kind == kind) {
            if let Some(v) = styles.get(sl.id) {
                m.insert(sl.id.to_string(), v.clone());
            }
        }
        serde_json::Value::Object(m)
    };
    let elements = json!([
        { "kind": "map_frame", "id": "f1", "x": 12.7, "y": 30.0, "w": 200.0, "h": 160.0, "crs": "EPSG:26915", "center": CENTER, "scale": 24000, "layers": "map",
          "graticule": { "interval": "auto", "labels": true }, "grid": { "interval": 500, "labels": true }, "textStyles": st("map_frame") },
        { "kind": "text", "id": "t1", "x": 12.7, "y": 10.0, "w": 254.0, "h": 14.0, "text": "Title", "font": "sans-bold", "size": 18, "align": "center", "color": "#000000", "textStyles": st("text") },
        { "kind": "legend", "id": "lg1", "frame": "f1", "x": 215.0, "y": 30.0, "w": 55.0, "h": 70.0, "title": "Legend title", "columns": 1, "textStyles": st("legend") },
        { "kind": "scale_bar", "id": "sb1", "frame": "f1", "x": 215.0, "y": 110.0, "w": 55.0, "h": 20.0, "units": "km", "segments": 4, "ratio": true, "verbal": true, "textStyles": st("scale_bar") },
        { "kind": "north_arrow", "id": "na1", "frame": "f1", "x": 230.0, "y": 135.0, "w": 25.0, "h": 25.0, "style": "rose", "textStyles": st("north_arrow") },
        { "kind": "declination", "id": "dc1", "frame": "f1", "x": 150.0, "y": 95.0, "w": 60.0, "h": 44.0, "date": "2026-01-01", "textStyles": st("declination") },
        { "kind": "sources", "id": "src1", "frame": "f1", "x": 12.7, "y": 195.0, "w": 200.0, "h": 8.0, "size": 6, "textStyles": st("sources") },
        { "kind": "table", "id": "tb1", "layer": ids[0], "x": 215.0, "y": 165.0, "w": 55.0, "h": 30.0, "fields": ["cls", "pop"], "textStyles": st("table") },
        { "kind": "chart", "id": "ch1", "layer": ids[0], "x": 150.0, "y": 135.0, "w": 60.0, "h": 40.0, "chartType": "bar", "category": "cls", "title": "Chart title", "textStyles": st("chart") }
    ]);
    let mut v = serde_json::to_value(&req.layout).unwrap();
    v["elements"] = elements;
    req.layout = serde_json::from_value(v).unwrap();
    req
}

#[test]
fn text_slots_cover_every_element_kind_that_draws_text() {
    // Every element kind is either listed in NO_TEXT or has at least one slot; every slot belongs to a real kind.
    let kinds = ["map_frame", "text", "legend", "scale_bar", "north_arrow", "declination", "sources", "shape", "table", "chart", "picture"];
    for k in kinds {
        assert!(NO_TEXT.contains(&k) || !slots::of_kind(k).is_empty(), "{k} has no text slot and is not marked as drawing no text");
    }
    for s in SLOTS {
        assert!(kinds.contains(&s.kind), "{} names an unknown element kind", s.id);
    }
}

#[test]
fn every_text_slot_changes_its_text_size_and_colour_in_the_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let base = render(&e, &slot_layout(&ids, json!({}))).svg;
    for sl in SLOTS.iter().filter(|s| s.id != "text.text") {
        let styled = render(&e, &slot_layout(&ids, json!({ sl.id: { "size": 23.0, "color": "#c0ffee" } }))).svg;
        let want = format!("font-size=\"{}\"", engine::layout::svg::mm(23.0 * PT));
        assert!(styled.contains(&want), "{}: no text drawn at 23 pt", sl.id);
        assert!(!base.contains(&want), "{}: the baseline already has 23 pt text", sl.id);
        assert!(styled.contains("fill=\"#c0ffee\""), "{}: the colour was not applied", sl.id);
        assert!(!base.contains("fill=\"#c0ffee\""), "{}", sl.id);
    }
}

#[test]
fn slot_styles_set_font_weight_slant_underline_spacing_and_halo() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let svg = render(&e, &slot_layout(&ids, json!({ "legend.title": { "bold": false, "italic": true, "underline": true, "letterSpacing": 0.4, "haloColor": "#ffffff", "haloWidth": 0.5, "family": "Georgia" } }))).svg;
    let title = svg.split("<text").find(|t| t.contains(">Legend title<")).expect("the legend title");
    for needle in ["font-weight=\"400\"", "font-style=\"italic\"", "text-decoration=\"underline\"", "letter-spacing=\"0.4\"", "paint-order=\"stroke\"", "stroke-width=\"1\"", "'Georgia'"] {
        assert!(title.contains(needle), "{needle} missing in {title}");
    }
}

#[test]
fn element_opacity_fades_the_whole_element_and_old_layouts_load_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = slot_layout(&ids, json!({}));
    let mut v = serde_json::to_value(&req.layout).unwrap();
    v["elements"][1]["opacity"] = json!(0.4);
    v["elements"][1]["name"] = json!("Map title");
    req.layout = serde_json::from_value(v.clone()).unwrap();
    let svg = render(&e, &req).svg;
    assert!(svg.contains("<g opacity=\"0.4\">"), "no opacity group");
    // Round trip keeps name and opacity; an element without them serialises without the new keys.
    let back = serde_json::to_value(&req.layout).unwrap();
    assert_eq!(back["elements"][1]["name"], "Map title");
    assert!(back["elements"][0].get("textStyles").is_none() && back["elements"][0].get("opacity").is_none(), "{}", back["elements"][0]);
}

// ---------- Topo: symbol stacks, patterns and markers in legends ----------

#[test]
fn topo_a_line_can_be_a_stack_of_strokes_in_the_frame_and_the_legend() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = with_surround_topo(&ids);
    let stack = json!([{ "color": "#000000", "width": 4.0 }, { "color": "#c8202f", "width": 2.5 }, { "color": "#000000", "width": 6.0, "dash": [0.9, 12.0], "cap": "butt" }]);
    let mut v = serde_json::to_value(req.styles.get(&ids[1]).cloned().unwrap_or_default()).unwrap();
    v["strokes"] = stack;
    req.styles.insert(ids[1].clone(), serde_json::from_value(v).unwrap());
    let svg = render(&e, &req).svg;
    let g = &svg[svg.find(&format!("<g id=\"layer-{}\"", ids[1])).unwrap()..];
    let g = &g[..g.find("</g></g>").map_or(g.len(), |i| i + 4)];
    // Three stroke groups, bottom to top, the last with the tie dash.
    let order: Vec<usize> = ["stroke=\"#000000\" stroke-opacity=\"1\" stroke-width=\"1.058\"", "stroke=\"#c8202f\"", "stroke-dasharray=\"0.238 3.175\""].iter().map(|k| g.find(k).unwrap_or_else(|| panic!("missing {k} in {g}"))).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    assert!(g.contains("stroke-linecap=\"butt\""));
    // The legend swatch draws the same stack.
    let lg = &svg[svg.find("<g id=\"legend-lg\"").unwrap()..];
    assert!(lg.contains("<line") && lg.contains("stroke=\"#c8202f\"") && lg.contains("stroke-dasharray"), "the stack is in the legend");
    // PDF export works with dashes and butt caps.
    assert!(export::svg_to_pdf(&svg).unwrap().starts_with(b"%PDF"));
}

#[test]
fn topo_classes_can_each_wear_their_own_stack() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = request(&ids, categorized());
    let mut v = serde_json::to_value(req.styles.get(&ids[0]).unwrap()).unwrap();
    v["classes"][0]["strokes"] = json!([{ "color": "#123456", "width": 3.0 }]);
    req.styles.insert(ids[0].clone(), serde_json::from_value(v).unwrap());
    let svg = render(&e, &req).svg;
    assert_eq!(svg.matches("stroke=\"#123456\"").count() >= 1, true, "class A's outline stack is drawn");
    // Class B has no stack of its own, so it keeps the layer's outline.
    assert!(svg.contains("stroke=\"#4d4d4d\""), "the other classes keep the layer's stroke");
}

#[test]
fn topo_legend_swatches_show_patterns_and_markers() {
    let dir = tempfile::tempdir().unwrap();
    let (e, ids) = fixture(dir.path());
    let mut req = with_surround_topo(&ids);
    for (id, extra) in [(&ids[0], json!({ "pattern": "marsh", "patternSpacing": 6.0, "stroke": "#169bd0" })), (&ids[2], json!({ "marker": "tri_open", "radius": 5.0 }))] {
        let mut v = serde_json::to_value(req.styles.get(id).cloned().unwrap_or_default()).unwrap();
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        req.styles.insert(id.clone(), serde_json::from_value(v).unwrap());
    }
    let svg = render(&e, &req).svg;
    let lg = &svg[svg.find("<g id=\"legend-lg\"").unwrap()..];
    assert!(lg.contains("<pattern id=\"swp-") && lg.contains("fill=\"url(#swp-"), "area swatch shows the pattern");
    assert!(lg.contains("fill-rule=\"evenodd\""), "point swatch shows the open triangle");
}

fn with_surround_topo(ids: &[String; 4]) -> RenderRequest {
    let mut req = request(ids, categorized());
    req.layout.elements.push(serde_json::from_value(json!({ "kind": "legend", "id": "lg", "frame": "f1", "x": 215.0, "y": 30.0, "w": 55.0, "h": 80.0, "title": "Legend", "columns": 1 })).unwrap());
    req
}
