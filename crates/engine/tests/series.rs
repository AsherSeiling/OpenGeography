//! Map series (C3.2, docs/LAYOUTS_PLAN.md): P21, P22 and the pieces under them.

use engine::layout::model::{Layout, RenderRequest};
use engine::layout::ExportFormat;
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn write_fixture(path: &Path, select: &str, srs: &str) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS '{srs}', USE_TMP_FILE false)", path.display())).unwrap();
}

/// `n` squares in UTM 15N in a row, 2 km wide, 1 km apart, named by `name_sql`, with a population and an id.
fn counties(e: &Engine, dir: &Path, n: usize, name_sql: &str) -> String {
    let p = dir.join("counties.gpkg");
    write_fixture(
        &p,
        &format!("SELECT x AS id, {name_sql} AS name, (x * 7) % 13 AS pop, ST_MakeEnvelope(400000 + x * 3000, 4600000 + (x % 3) * 500, 402000 + x * 3000 + (x % 4) * 200, 4602000 + (x % 3) * 500 + (x % 2) * 300) AS geom FROM range({n}) t(x)"),
        "EPSG:26915",
    );
    e.import(&p).unwrap()[0].id.clone()
}

fn layout(index: &str, series: Value) -> Layout {
    serde_json::from_value(json!({
        "id": "S1", "name": "Counties", "version": 1, "body": "earth",
        "page": { "width": 279.4, "height": 215.9, "units": "mm", "margin": 12.7, "background": "#ffffff" },
        "elements": [
            { "kind": "text", "id": "t1", "x": 12.7, "y": 10.0, "w": 254.0, "h": 14.0, "text": "{page_name} ({page_number}/{page_count}) pop {page_pop}", "font": "sans-bold", "size": 18, "align": "center", "color": "#000000" },
            { "kind": "map_frame", "id": "f1", "x": 12.7, "y": 30.0, "w": 200.0, "h": 160.0, "crs": "EPSG:26915", "center": [0, 0], "scale": 1000000, "layers": "map", "followsSeries": true },
            { "kind": "scale_bar", "id": "sb", "frame": "f1", "x": 215.0, "y": 120.0, "w": 50.0, "h": 12.0, "units": "km", "segments": 2 }
        ],
        "series": series
    }))
    .map(|mut l: Layout| { let _ = index; l.version = 1; l })
    .unwrap()
}

fn spec(index: &str) -> Value {
    json!({ "indexLayer": index, "nameField": "name", "extent": "fit", "marginPct": 10.0 })
}

fn request(l: &Layout, index: &str) -> RenderRequest {
    serde_json::from_value(json!({
        "layout": l,
        "styles": { index: { "kind": "single", "fill": "#7aa8d8", "stroke": "#4d4d4d", "strokeWidth": 0.7, "opacity": 0.75, "radius": 3.5 } },
        "order": [index], "visible": {}, "project": "Demo", "date": "2026-01-01"
    }))
    .unwrap()
}

fn frame_of(r: &RenderRequest) -> (f64, [f64; 2], f64, f64) {
    match &r.layout.elements[1] {
        engine::layout::model::Element::MapFrame { at, center, scale, .. } => (*scale, *center, at.w, at.h),
        _ => unreachable!(),
    }
}

#[test]
fn series_pages_follow_the_sort_field_then_the_feature_id() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path(), 6, "CASE WHEN x = 5 THEN NULL ELSE ['Zeta','alpha','Beta','alpha','delta'][x + 1] END");
    let pages = e.series_pages(&layout(&id, spec(&id))).unwrap();
    // Text sorts without regard to case: alpha, alpha (by id), Beta, delta, Zeta, then the NULL name last.
    assert_eq!(pages.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["alpha", "alpha", "Beta", "delta", "Zeta", ""]);
    assert_eq!(pages.iter().map(|p| p.index).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
    assert!(pages[0].fid < pages[1].fid, "ties keep the feature order");
    // A numeric sort field sorts as numbers (10 after 9), another field than the name.
    let mut s = spec(&id);
    s["sortField"] = json!("pop");
    let by_pop = e.series_pages(&layout(&id, s)).unwrap();
    let pops: Vec<i64> = by_pop.iter().map(|p| p.attrs["pop"].parse().unwrap()).collect();
    assert!(pops.windows(2).all(|w| w[0] <= w[1]), "{pops:?}");
    // The definition query limits the pages.
    e.set_definition_query(&id, Some("id < 3")).unwrap();
    assert_eq!(e.series_pages(&layout(&id, spec(&id))).unwrap().len(), 3);
}

#[test]
fn series_errors_are_readable() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path(), 3, "'c' || x");
    let mut l = layout(&id, spec(&id));
    l.series = None;
    assert!(e.series_pages(&l).unwrap_err().to_string().contains("no map series"));
    assert!(e.series_pages(&layout("nope", spec("nope"))).unwrap_err().to_string().contains("no longer in the project"));
    let mut s = spec(&id);
    s["nameField"] = json!("missing");
    assert!(e.series_pages(&layout(&id, s)).unwrap_err().to_string().contains("no field called 'missing'"));
    e.set_definition_query(&id, Some("id > 100")).unwrap();
    assert!(e.series_pages(&layout(&id, spec(&id))).unwrap_err().to_string().contains("no features"));
    // The layout JSON refuses a broken series.
    for bad in [json!({ "indexLayer": "", "nameField": "n" }), json!({ "indexLayer": "a", "nameField": "n", "extent": "zoom" }), json!({ "indexLayer": "a", "nameField": "n", "extent": "fixed_scale" }), json!({ "indexLayer": "a", "nameField": "n", "marginPct": 80 })] {
        let mut v = serde_json::to_value(layout(&id, spec(&id))).unwrap();
        v["series"] = bad.clone();
        assert!(Layout::from_json(&v.to_string()).is_err(), "{bad}");
    }
    // A series with no frame that follows it says what to tick.
    let mut v = serde_json::to_value(layout(&id, spec(&id))).unwrap();
    v["elements"][1]["followsSeries"] = json!(false);
    let l2: Layout = serde_json::from_value(v).unwrap();
    e.set_definition_query(&id, None).unwrap();
    let err = e.apply_series_page(&request(&l2, &id), 0).unwrap_err().to_string();
    assert!(err.contains("Follows the map series"), "{err}");
    assert!(e.apply_series_page(&request(&layout(&id, spec(&id)), &id), 99).unwrap_err().to_string().contains("no page 100"));
}

#[test]
fn a_followed_frame_is_centred_on_its_feature_and_fits_it_with_the_margin() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path(), 4, "'c' || x");
    let req = request(&layout(&id, spec(&id)), &id);
    let pages = e.series_pages(&req.layout).unwrap();
    for p in &pages {
        let (r, page, count) = e.apply_series_page(&req, p.index).unwrap();
        assert_eq!((count, page.fid), (4, p.fid));
        let (scale, center, w, h) = frame_of(&r);
        // The feature's box, straight from the fixture formula.
        let x = p.fid - 1; // feature ids start at 1; the fixture's x starts at 0
        let bbox = [400000.0 + x as f64 * 3000.0, 4600000.0 + (x % 3) as f64 * 500.0, 402000.0 + x as f64 * 3000.0 + (x % 4) as f64 * 200.0, 4602000.0 + (x % 3) as f64 * 500.0 + (x % 2) as f64 * 300.0];
        let c = [(bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0];
        assert!((center[0] - c[0]).abs() < 1.0 && (center[1] - c[1]).abs() < 1.0, "page {x}: {center:?} vs {c:?}");
        // The frame shows 10 % more than the feature on each side, in the tighter direction.
        let (fw, fh) = (w / 1000.0 * scale, h / 1000.0 * scale);
        let (bw, bh) = (bbox[2] - bbox[0], bbox[3] - bbox[1]);
        assert!(fw >= bw * 1.2 - 1e-6 && fh >= bh * 1.2 - 1e-6, "feature plus margin fits: {fw}x{fh} for {bw}x{bh}");
        assert!((fw - bw * 1.2).abs() < 0.01 * fw || (fh - bh * 1.2).abs() < 0.01 * fh, "and no looser than needed (scale rounded up to a whole number): {fw}x{fh} for {bw}x{bh}");
        assert_eq!(scale.fract(), 0.0);
        assert_eq!(r.page.as_ref().unwrap().number, p.index + 1);
        assert!(r.series_page.is_none());
    }
    // Fixed scale: every page at the same scale, still centred.
    let mut s = spec(&id);
    s["extent"] = json!("fixed_scale");
    s["scale"] = json!(25000);
    let fixed = request(&layout(&id, s), &id);
    for i in 0..4 {
        assert_eq!(frame_of(&e.apply_series_page(&fixed, i).unwrap().0).0, 25000.0);
    }
}

#[test]
fn tokens_name_the_page_its_number_and_the_index_features_fields() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path(), 3, "'County ' || x");
    let mut req = request(&layout(&id, spec(&id)), &id);
    req.series_page = Some(1);
    let out = e.render_layout_svg(&req, &|| Ok(())).unwrap();
    assert!(out.svg.contains("County 1 (2/3) pop 7"), "{}", out.svg.split("<text").nth(1).unwrap_or(""));
    // Without a series page the tokens keep their old meaning.
    req.series_page = None;
    let plain = e.render_layout_svg(&req, &|| Ok(())).unwrap();
    assert!(plain.svg.contains("Counties (1/1) pop {page_pop}"));
}

fn pdf_count(pdf: &[u8]) -> usize {
    let s = String::from_utf8_lossy(pdf);
    s.split("/Count ").nth(1).and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse().ok()).unwrap_or(0)
}

#[test]
fn p21_ten_counties_make_a_ten_page_pdf_in_sort_order_each_frame_holds_its_county() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let names = ["Yuma", "Adams", "Polk", "Cass", "Wake", "Dane", "Lane", "Kent", "Bell", "Ford"];
    let list = names.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ");
    let id = counties(&e, dir.path(), 10, &format!("[{list}][x + 1]"));
    let req = request(&layout(&id, spec(&id)), &id);
    let out = dir.path().join("series.pdf");
    let r = e.export_series(&req, ExportFormat::Pdf, 150.0, &out).unwrap();
    assert_eq!(r.pages, 10);
    let pdf = std::fs::read(&out).unwrap();
    assert!(pdf.starts_with(b"%PDF-") && pdf_count(&pdf) == 10, "a valid PDF with ten pages");
    assert_eq!(String::from_utf8_lossy(&pdf).matches("/MediaBox [0 0 792 611.9").count(), 10, "every page is Letter landscape");
    assert!(String::from_utf8_lossy(&pdf).contains("/ToUnicode"), "fonts embedded");
    assert!(!out.with_extension("pdf.part").exists());

    // Page i of the series: the pages are in name order and the title shows the county.
    let mut sorted = names.to_vec();
    sorted.sort_by_key(|n| n.to_lowercase());
    for (i, name) in sorted.iter().enumerate() {
        let mut one = req.clone();
        one.series_page = Some(i);
        let svg = e.render_layout_svg(&one, &|| Ok(())).unwrap().svg;
        assert!(svg.contains(&format!("{name} ({}/10)", i + 1)), "page {i}: {name}");
        // The frame contains its county (the filled square is drawn inside the frame's clip: it has a path).
        let (r, p, _) = e.apply_series_page(&req, i).unwrap();
        assert_eq!(&p.name, name);
        let (scale, center, w, h) = frame_of(&r);
        let x = names.iter().position(|n| n == name).unwrap() as f64;
        let (x0, x1) = (400000.0 + x * 3000.0, 402000.0 + x * 3000.0);
        assert!(center[0] - w / 2000.0 * scale <= x0 && center[0] + w / 2000.0 * scale >= x1, "{name}: frame spans the county east–west");
        let _ = h;
    }
    assert!(r.warnings.iter().all(|w| w.starts_with("Page ")), "{:?}", r.warnings);
}

#[test]
fn png_export_writes_one_file_per_page_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path(), 4, "'c' || x");
    let req = request(&layout(&id, spec(&id)), &id);
    let out = dir.path().join("pngs");
    let r = e.export_series(&req, ExportFormat::Png, 72.0, &out).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&out).unwrap().map(|f| f.unwrap().file_name().to_string_lossy().to_string()).collect();
    names.sort();
    assert_eq!(names, ["Counties_001_c0.png", "Counties_002_c1.png", "Counties_003_c2.png", "Counties_004_c3.png"]);
    assert_eq!(r.files.len(), 4);
    for f in &r.files {
        assert_eq!(&std::fs::read(f).unwrap()[1..4], b"PNG");
    }
    // SVG isn't a series format.
    assert!(e.export_series(&req, ExportFormat::Svg, 72.0, &out).unwrap_err().to_string().contains("PDF or as PNG"));
}

/// Cancel the series task named after the layout as soon as it reports page `after` or a later one (a poll can miss a page on a busy
/// runner); true when it was seen running. On failure the message says what the watcher last saw.
fn cancel_at_page(label: &str, after: usize) -> std::thread::JoinHandle<Result<(), String>> {
    let label = label.to_string();
    std::thread::spawn(move || {
        let mut last = String::from("no task with that name was ever listed");
        for _ in 0..20_000 {
            std::thread::sleep(std::time::Duration::from_millis(1));
            if let Some(t) = engine::progress::snapshot().into_iter().find(|t| t.label.contains(&label)) {
                last = format!("{} / {}", t.label, t.detail);
                let page: Option<usize> = t.detail.strip_prefix("Page ").and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok());
                if page.is_some_and(|p| p >= after) {
                    if !t.cancellable {
                        return Err(format!("task {last} is not cancellable"));
                    }
                    engine::progress::cancel(t.id);
                    return Ok(());
                }
            }
        }
        Err(format!("never reached page {after}; last seen: {last}"))
    })
}

/// Enough pages that the export is still running when the watcher's cancel lands, even on a fast release build (CI run 40780933485:
/// 40 pages finished before the cancel arrived on macOS and Windows).
const PAGES: usize = 400;

#[test]
fn p22_cancel_after_page_three_leaves_no_partial_file() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path(), PAGES, "'c' || x");
    let mut l = layout(&id, spec(&id));
    l.name = "CancelPdf".into();
    let req = request(&l, &id);
    let pdf = dir.path().join("cancelled.pdf");
    let watcher = cancel_at_page("CancelPdf", 3);
    let err = e.export_series(&req, ExportFormat::Pdf, 72.0, &pdf).unwrap_err().to_string();
    watcher.join().unwrap().expect("the watcher cancelled the task");
    assert!(err.to_lowercase().contains("cancel"), "{err}");
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|f| f.unwrap().file_name().to_string_lossy().to_string()).filter(|n| n.contains("cancelled")).collect();
    assert!(left.is_empty(), "no PDF and no .part file: {left:?}");

    // PNG files: the folder holds nothing afterwards, not even the staging folder.
    let mut l = layout(&id, spec(&id));
    l.name = "CancelPng".into();
    let out = dir.path().join("pngs");
    let watcher = cancel_at_page("CancelPng", 3);
    assert!(e.export_series(&request(&l, &id), ExportFormat::Png, 72.0, &out).is_err());
    watcher.join().unwrap().expect("the watcher cancelled the task");
    assert_eq!(std::fs::read_dir(&out).map(|d| d.count()).unwrap_or(0), 0, "an empty folder");
    assert!(engine::progress::snapshot().iter().all(|t| !t.label.contains("CancelP")), "task removed");

    // The engine still works.
    assert_eq!(e.series_pages(&req.layout).unwrap().len(), PAGES);
}
