//! Sharing a statistics sheet (ST7): CSV of a table, SVG and PNG of a chart, a card on a layout page (drawn from the card, out of date said),
//! and the sheet report with methods and citations. Old layouts still load.

use engine::layout::model::RenderRequest;
use engine::stats::result::*;
use engine::stats::share::{card_state, cell_text, table_csv, CardState};
use engine::stats::{CardInput, StatCard, StatSheet};
use engine::Engine;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn layer_from(e: &Engine, dir: &std::path::Path, select: &str) -> String {
    let p = dir.join("t.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)", p.display())).unwrap();
    e.import(&p).unwrap().remove(0).id
}

fn card(id: &str, tool: &str, layer: &str, options: Value) -> StatCard {
    StatCard {
        id: id.into(),
        tool: tool.into(),
        title: None,
        inputs: BTreeMap::from([("in".to_string(), CardInput { layer: layer.into(), fids: None })]),
        options: options.as_object().cloned().unwrap_or_default(),
        result: None,
    }
}

/// An engine with a layer of 60 points, a saved sheet "S1" holding a run histogram card "h" and a card "n" that was never run.
fn setup(dir: &std::path::Path) -> (Engine, String, StatSheet) {
    let e = Engine::new().unwrap();
    let l = layer_from(&e, dir, "SELECT i AS id, (i * 1.5)::DOUBLE AS v, ST_Point(i, i) AS geom FROM range(1, 61) t(i)");
    let mut h = card("h", "chart_histogram", &l, json!({"field": "v", "bins": 4}));
    h.title = Some("Heights, a \"test\"".into());
    h.result = Some(e.run_stat_card(&h).unwrap());
    let n = card("n", "chart_box", &l, json!({"field": "v"}));
    let sheet = StatSheet { id: "S1".into(), name: "Sheet one".into(), version: 1, cards: vec![h, n] };
    e.save_stat_sheet(&sheet).unwrap();
    (e, l, sheet)
}

fn first_fid(e: &Engine, layer: &str) -> i64 {
    e.rows(layer, &engine::table::RowsQuery { limit: 1, ..Default::default() }).unwrap().fids[0]
}

fn page(elements: Value) -> RenderRequest {
    serde_json::from_value(json!({
        "layout": {
            "id": "L1", "name": "Stats page", "version": 1, "body": "earth",
            "page": { "width": 279.4, "height": 215.9, "units": "mm", "margin": 12.7, "background": "#ffffff" },
            "elements": elements
        },
        "styles": {}, "order": [], "visible": {}, "project": "Demo", "date": "2026-01-01"
    }))
    .unwrap()
}

#[test]
fn csv_quotes_keeps_full_precision_and_writes_units_in_the_heading() {
    let t = StatTable {
        title: "t".into(),
        columns: vec![
            StatColumn { name: "name, with comma".into(), unit: None, kind: ColumnKind::Text },
            StatColumn { name: "length".into(), unit: Some("m".into()), kind: ColumnKind::Number },
            StatColumn { name: "n".into(), unit: None, kind: ColumnKind::Integer },
        ],
        rows: vec![vec![json!("say \"hi\""), json!(0.1 + 0.2), json!(3)], vec![json!("plain"), Value::Null, json!(-4)]],
    };
    let csv = table_csv(&t);
    assert_eq!(csv, "\"name, with comma\",length (m),n\r\n\"say \"\"hi\"\"\",0.30000000000000004,3\r\nplain,,-4\r\n");
    // for people: four significant digits, whole numbers as they are
    assert_eq!(cell_text(&json!(1234.5678)), "1235");
    assert_eq!(cell_text(&json!(12.3456)), "12.35");
    assert_eq!(cell_text(&json!(0.012345)), "0.01235");
    assert_eq!(cell_text(&json!(42)), "42");
    assert_eq!(cell_text(&json!(2.0)), "2");
    assert_eq!(cell_text(&Value::Null), "");
}

#[test]
fn a_card_is_out_of_date_when_a_layer_changed_and_missing_when_it_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l, sheet) = setup(dir.path());
    let h = &sheet.cards[0];
    let lookup = |id: &str| e.layer(id).ok().map(|x| (x.name, x.version));
    assert_eq!(card_state(h, &lookup), CardState::Current);
    assert_eq!(card_state(&sheet.cards[1], &lookup), CardState::NotRun);
    let fid = first_fid(&e, &l);
    e.update_cell(&l, fid, "v", Some("999")).unwrap();
    let lookup = |id: &str| e.layer(id).ok().map(|x| (x.name, x.version));
    assert!(matches!(card_state(h, &lookup), CardState::OutOfDate(_)));
    assert!(card_state(h, &lookup).sentence("It").unwrap().contains("out of date"));
    assert!(matches!(card_state(h, &|_| None), CardState::Missing(_)));
}

#[test]
fn a_chart_card_on_a_page_is_the_sheets_chart_and_a_table_card_is_its_table() {
    let dir = tempfile::tempdir().unwrap();
    let (e, _l, _s) = setup(dir.path());
    let req = page(json!([
        { "kind": "stat_card", "id": "s1", "x": 10.0, "y": 10.0, "w": 120.0, "h": 70.0, "sheet": "S1", "card": "h" },
        { "kind": "stat_card", "id": "s2", "x": 10.0, "y": 100.0, "w": 120.0, "h": 40.0, "sheet": "S1", "card": "h", "part": "table" }
    ]));
    let out = e.render_layout_svg(&req, &|| Ok(())).unwrap();
    assert!(out.svg.contains("data-kind=\"histogram\""), "the chart is not on the page");
    assert!(out.svg.matches("class=\"mark\"").count() >= 4, "one mark per bar");
    assert!(out.svg.contains("class=\"title\""), "chart title");
    assert!(out.svg.to_lowercase().contains(">count<") || out.svg.to_lowercase().contains(">n<"), "the table's headings");
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    // the page is valid SVG for the PDF and PNG paths, fonts included
    engine::layout::export::parse(&out.svg).unwrap();
    let png = engine::layout::export::svg_to_png(&out.svg, 72.0).unwrap();
    assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
}

#[test]
fn the_page_says_when_the_card_is_not_run_out_of_date_or_gone_and_never_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l, _s) = setup(dir.path());
    let req = page(json!([
        { "kind": "stat_card", "id": "a", "x": 10.0, "y": 10.0, "w": 100.0, "h": 60.0, "sheet": "S1", "card": "n" },
        { "kind": "stat_card", "id": "b", "x": 10.0, "y": 80.0, "w": 100.0, "h": 60.0, "sheet": "S1", "card": "nope" },
        { "kind": "stat_card", "id": "c", "x": 10.0, "y": 150.0, "w": 100.0, "h": 40.0, "sheet": "gone", "card": "h" },
        { "kind": "stat_card", "id": "d", "x": 120.0, "y": 10.0, "w": 100.0, "h": 60.0 },
        { "kind": "stat_card", "id": "e", "x": 120.0, "y": 80.0, "w": 100.0, "h": 60.0, "sheet": "S1", "card": "h", "index": 3 }
    ]));
    let w = e.render_layout_svg(&req, &|| Ok(())).unwrap().warnings.join(" | ");
    for want in ["has not been run", "no longer on sheet", "is gone", "has no card yet", "no chart number 4"] {
        assert!(w.contains(want), "{want} missing in {w}");
    }
    // change the layer: the histogram is out of date and still drawn
    let fid = first_fid(&e, &l);
    e.update_cell(&l, fid, "v", Some("999")).unwrap();
    let req = page(json!([{ "kind": "stat_card", "id": "s1", "x": 10.0, "y": 10.0, "w": 120.0, "h": 70.0, "sheet": "S1", "card": "h" }]));
    let out = e.render_layout_svg(&req, &|| Ok(())).unwrap();
    assert!(out.warnings.iter().any(|w| w.contains("out of date")), "{:?}", out.warnings);
    assert!(out.svg.contains("data-kind=\"histogram\""));
}

#[test]
fn layouts_without_statistics_cards_are_unchanged_and_a_bad_part_is_refused() {
    let l = engine::layout::model::Layout::from_json(
        &json!({ "id": "L", "name": "Old", "version": 1, "body": "earth", "page": { "width": 100.0, "height": 100.0, "units": "mm", "margin": 5.0, "background": "#ffffff" },
                 "elements": [ { "kind": "text", "id": "t", "x": 1.0, "y": 1.0, "w": 50.0, "h": 10.0, "text": "Hi" } ] })
        .to_string(),
    )
    .unwrap();
    let back = serde_json::to_value(&l).unwrap();
    assert!(back["elements"][0].get("sheet").is_none());
    let bad = json!({ "id": "L", "name": "B", "version": 1, "body": "earth", "page": { "width": 100.0, "height": 100.0, "units": "mm", "margin": 5.0, "background": "#ffffff" },
                      "elements": [ { "kind": "stat_card", "id": "s", "x": 1.0, "y": 1.0, "w": 50.0, "h": 10.0, "sheet": "S", "card": "c", "part": "pie" } ] });
    assert!(engine::layout::model::Layout::from_json(&bad.to_string()).unwrap_err().to_string().contains("chart or table"));
    // a stat_card round-trips with its fields and without the defaults
    let ok = json!({ "id": "L", "name": "B", "version": 1, "body": "earth", "page": { "width": 100.0, "height": 100.0, "units": "mm", "margin": 5.0, "background": "#ffffff" },
                     "elements": [ { "kind": "stat_card", "id": "s", "x": 1.0, "y": 1.0, "w": 50.0, "h": 10.0, "sheet": "S", "card": "c", "part": "table", "index": 1 } ] });
    let back = serde_json::to_value(engine::layout::model::Layout::from_json(&ok.to_string()).unwrap()).unwrap();
    assert_eq!((back["elements"][0]["part"].as_str(), back["elements"][0]["index"].as_u64()), (Some("table"), Some(1)));
}

#[test]
fn export_writes_csv_svg_png_and_a_report_with_methods_and_citations() {
    let dir = tempfile::tempdir().unwrap();
    let (e, _l, mut sheet) = setup(dir.path());
    let h = &sheet.cards[0];
    let p = |n: &str| dir.path().join(n).to_string_lossy().to_string();
    e.export_stat_table(h, 0, &p("t.csv")).unwrap();
    let csv = std::fs::read_to_string(p("t.csv")).unwrap();
    assert!(csv.lines().count() >= 5 && csv.contains("\r\n"), "{csv}");
    e.export_stat_chart(h, 0, "svg", 96.0, &p("c.svg")).unwrap();
    let svg = std::fs::read_to_string(p("c.svg")).unwrap();
    assert!(svg.starts_with("<?xml") && svg.contains("data-kind=\"histogram\"") && svg.contains("#ffffff") && !svg.contains("currentColor"));
    e.export_stat_chart(h, 0, "png", 192.0, &p("c.png")).unwrap();
    let png = std::fs::read(p("c.png")).unwrap();
    assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
    // 720 x 400 CSS px at 192 dpi is 1440 x 800
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1440);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 800);
    // sentences for what cannot be exported
    assert!(e.export_stat_chart(h, 0, "gif", 96.0, &p("x")).unwrap_err().to_string().contains("Unknown chart format"));
    assert!(e.export_stat_chart(h, 5, "svg", 96.0, &p("x")).unwrap_err().to_string().contains("does not exist"));
    assert!(e.export_stat_table(&sheet.cards[1], 0, &p("x")).unwrap_err().to_string().contains("not been run"));
    assert!(e.export_stat_table(h, 0, "/no/such/dir/x.csv").unwrap_err().to_string().contains("could not be written"));
    // report: titles escaped, method, rows used, notes, citations once, the card never run named in the warnings
    sheet.cards[0].result.as_mut().unwrap().citations = vec!["Freedman and Diaconis 1981".into()];
    let w = e.export_stat_report(&sheet, "2026-11-12", &p("r.html")).unwrap();
    assert_eq!(w.len(), 1);
    assert!(w[0].contains("has not been run"));
    let html = std::fs::read_to_string(p("r.html")).unwrap();
    assert!(html.contains("<h1>Sheet one</h1>") && html.contains("Heights, a &quot;test&quot;"), "title escaped");
    assert!(html.contains("Rows used: 60.") && html.contains("Methods and citations") && html.contains("Freedman and Diaconis 1981"));
    assert!(html.contains("data-kind=\"histogram\"") && html.contains("<table>") && html.contains("2026-11-12"));
    assert!(!html.contains("<script"));
}
