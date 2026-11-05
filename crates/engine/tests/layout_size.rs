//! UR-5: how large a layout page can really be exported. `probe_*` prints measurements (run with --ignored --nocapture);
//! the plain tests pin the limits the app now states.

use engine::layout::model::RenderRequest;
use engine::layout::ExportFormat;
use engine::Engine;
use serde_json::json;
use std::path::Path;
use std::time::Instant;

fn engine_with_layer(dir: &Path) -> (Engine, String) {
    let p = dir.join("polys.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY (SELECT 1 AS id, ST_MakeEnvelope(499000, 4649000, 501000, 4650500) AS geom UNION ALL SELECT 2, ST_MakeEnvelope(501500, 4649000, 503000, 4651000)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:26915', USE_TMP_FILE false)", p.display())).unwrap();
    let e = Engine::new().unwrap();
    let id = e.import(&p).unwrap()[0].id.clone();
    (e, id)
}

fn request(id: &str, w: f64, h: f64) -> RenderRequest {
    serde_json::from_value(json!({
        "layout": { "id": "L1", "name": "Big page", "version": 1, "body": "earth",
            "page": { "width": w, "height": h, "units": "mm", "margin": 10.0, "background": "#ffffff" },
            "elements": [
                { "kind": "map_frame", "id": "f1", "x": 10.0, "y": 30.0, "w": w - 20.0, "h": h - 40.0, "crs": "EPSG:26915", "center": [501000.0, 4650000.0], "scale": 10000, "layers": "map", "border": { "width": 0.35, "color": "#000000" } },
                { "kind": "text", "id": "t1", "x": 10.0, "y": 5.0, "w": w - 20.0, "h": 20.0, "text": "{layout_name}", "font": "sans-bold", "size": 24, "align": "center", "color": "#000000" } ] },
        "styles": { id: { "kind": "single", "fill": "#7aa8d8", "stroke": "#333333", "strokeWidth": 1.0, "opacity": 1, "radius": 3.5 } },
        "order": [id], "visible": {}, "project": "Demo", "date": "2026-01-01" }))
    .unwrap()
}

fn go(e: &Engine, id: &str, w: f64, h: f64, fmt: ExportFormat, dpi: f64, out: &Path) -> Result<(f64, u64), String> {
    let t = Instant::now();
    e.export_layout(&request(id, w, h), fmt, dpi, out).map_err(|e| e.to_string())?;
    Ok((t.elapsed().as_secs_f64(), std::fs::metadata(out).map(|m| m.len()).unwrap_or(0)))
}

// Measured 2026-11-05 on a 16 GB Mac: PDF export of a 1 000 000 mm page takes milliseconds (a PDF page has no pixels); a PNG of 610
// million pixels takes 2.4 s and 4.4 GB at peak; one of 39 billion pixels (a 20 m page at 300 dpi) was killed by the system.

#[test]
fn a_very_large_page_exports_to_pdf_and_svg() {
    let dir = tempfile::tempdir().unwrap();
    let (e, id) = engine_with_layer(dir.path());
    for side in [20_000.0, 100_000.0] {
        let pdf = dir.path().join("big.pdf");
        e.export_layout(&request(&id, side, side * 0.7), ExportFormat::Pdf, 300.0, &pdf).unwrap();
        let text = String::from_utf8_lossy(&std::fs::read(&pdf).unwrap()).into_owned();
        let line = text.lines().find(|l| l.contains("/MediaBox")).expect("a page size");
        let nums: Vec<f64> = line.split(|c: char| c == '[' || c == ']' || c == ' ').filter_map(|t| t.parse().ok()).collect();
        assert!((nums[2] - side / 25.4 * 72.0).abs() < 0.1, "{side}: {line}"); // 72 PDF units to the inch
        let svg = dir.path().join("big.svg");
        e.export_layout(&request(&id, side, side * 0.7), ExportFormat::Svg, 300.0, &svg).unwrap();
        assert!(std::fs::metadata(&svg).unwrap().len() > 100);
    }
}

#[test]
fn a_png_too_large_to_make_is_refused_in_words_and_a_smaller_dpi_works() {
    let dir = tempfile::tempdir().unwrap();
    let (e, id) = engine_with_layer(dir.path());
    let out = dir.path().join("big.png");
    // A 20 m page at 300 dpi would be 39 billion pixels.
    let err = e.export_layout(&request(&id, 20_000.0, 14_000.0), ExportFormat::Png, 300.0, &out).unwrap_err().to_string();
    assert!(err.contains("20000 × 14000 mm") && err.contains("million") && err.contains("PDF or SVG"), "{err}");
    assert!(err.contains("Use 37 dpi or less"), "{err}");
    assert!(!out.exists(), "nothing is written");
    // A 100 m page cannot be a PNG at any resolution.
    let err = e.export_layout(&request(&id, 100_000.0, 70_000.0), ExportFormat::Png, 20.0, &out).unwrap_err().to_string();
    assert!(err.contains("Even at the lowest resolution"), "{err}");
    // The dpi it names does work (a 2.4 m page keeps the test fast: 100 dpi = 9 400 x 6 600 px).
    e.export_layout(&request(&id, 2400.0, 1700.0), ExportFormat::Png, 100.0, &out).unwrap();
    assert!(out.exists());
}

#[test]
fn the_screen_preview_of_a_huge_page_lowers_its_resolution_instead_of_failing() {
    let dir = tempfile::tempdir().unwrap();
    let (e, id) = engine_with_layer(dir.path());
    // 20 m x 14 m at the preview's 96 dpi would be 4 billion pixels.
    let p = e.render_layout_preview(&request(&id, 20_000.0, 14_000.0), 96.0).unwrap();
    assert!(p.png_bytes > 0 && p.png_bytes < 60_000_000);
    assert!(p.dpi < 20.0 && p.dpi > 1.0, "the dpi actually drawn is reported: {}", p.dpi);
}

#[test]
#[ignore]
fn probe_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let (e, id) = engine_with_layer(dir.path());
    for side in [5_000.0, 20_000.0, 50_000.0, 200_000.0, 1_000_000.0] {
        let out = dir.path().join("o.pdf");
        let r = go(&e, &id, side, side * 0.7, ExportFormat::Pdf, 300.0, &out);
        let head = std::fs::read(&out).map(|b| String::from_utf8_lossy(&b).lines().find(|l| l.contains("MediaBox")).unwrap_or("").to_string()).unwrap_or_default();
        println!("{side:>9} mm PDF: {r:?} {head}");
    }
}

#[test]
#[ignore]
fn probe_png() {
    let dir = tempfile::tempdir().unwrap();
    let (e, id) = engine_with_layer(dir.path());
    let (w, h, dpi): (f64, f64, f64) = std::env::var("PNG").unwrap().split(',').map(|v| v.parse().unwrap()).collect::<Vec<f64>>().try_into().map(|v: [f64; 3]| (v[0], v[1], v[2])).unwrap();
    let r = go(&e, &id, w, h, ExportFormat::Png, dpi, &dir.path().join("o.png"));
    println!("{w}x{h} mm at {dpi} dpi ({:.0} Mpx): {r:?}", w / 25.4 * dpi * h / 25.4 * dpi / 1e6);
}
