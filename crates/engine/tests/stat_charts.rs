//! Statistics charts (docs/STATISTICS_WORKSPACE_PLAN.md, ST3): the numbers behind each chart against hand-worked values, the SVG structure (every
//! mark has its data-mark index, valid SVG, no scripts, Okabe-Ito colours), density binning above the point limit and resolving a bin to features,
//! dropped rows counted, and the chart tools as nodes.

use engine::stats::chart::{chart_svg, nice_ticks, render_svg, ChartStyle};
use engine::stats::charts::{self, qnorm, quantile7};
use engine::stats::result::*;
use engine::stats::{CardInput, StatCard};
use engine::Engine;
use serde_json::{json, Map};
use std::collections::BTreeMap;

fn card(tool: &str, layer: &str, options: serde_json::Value, fids: Option<Vec<i64>>) -> StatCard {
    StatCard {
        id: "c".into(),
        tool: tool.into(),
        title: None,
        inputs: BTreeMap::from([("in".to_string(), CardInput { layer: layer.into(), fids })]),
        options: options.as_object().cloned().unwrap_or_default(),
        result: None,
    }
}

fn layer_from(e: &Engine, dir: &std::path::Path, select: &str) -> String {
    let p = dir.join("t.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)", p.display())).unwrap();
    e.import(&p).unwrap().remove(0).id
}

fn marks(spec: &ChartSpec) -> usize {
    spec.series.iter().filter(|s| s.style != SeriesStyle::Line).map(|s| s.marks.len()).sum()
}

#[test]
fn quantiles_and_the_normal_quantile_match_reference_values() {
    // R: quantile(c(1,2,3,4,5,6,7,8,9,100), c(.25,.5,.75)) = 3.25 5.5 7.75 (type 7)
    let v = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 100.0];
    assert!((quantile7(&v, 0.25) - 3.25).abs() < 1e-12);
    assert!((quantile7(&v, 0.5) - 5.5).abs() < 1e-12);
    assert!((quantile7(&v, 0.75) - 7.75).abs() < 1e-12);
    // R: qnorm(c(.5, .975, .8413447460685429, .001, .999)) = 0 1.959963984540054 1 -3.090232306167813 3.090232306167813
    for (p, z) in [(0.5, 0.0), (0.975, 1.959963984540054), (0.8413447460685429, 1.0), (0.001, -3.090232306167813), (0.999, 3.090232306167813), (0.02, -2.053748910631823)] {
        assert!((qnorm(p) - z).abs() < 1e-7, "qnorm({p}) = {} want {z}", qnorm(p));
    }
}

#[test]
fn histogram_uses_freedman_diaconis_and_counts_every_value() {
    let vals: Vec<f64> = (1..=100).map(|x| x as f64).collect();
    let fids: Vec<i64> = (1..=100).collect();
    let b = charts::histogram(&fids, &vals, None, "x", usize::MAX).unwrap();
    // IQR 49.5, width = 2 * 49.5 * 100^(-1/3) = 21.33, ceil(99 / 21.33) = 5 bins
    let m = &b.chart.series[0].marks;
    assert_eq!(m.len(), 5);
    assert_eq!(m.iter().map(|k| k.y.unwrap() as usize).sum::<usize>(), 100);
    assert_eq!(m.iter().map(|k| k.fids.len()).sum::<usize>(), 100);
    assert!(b.notes.iter().any(|n| n.contains("Freedman-Diaconis")));
    // bins you set
    let b = charts::histogram(&fids, &vals, Some(10), "x", usize::MAX).unwrap();
    assert_eq!(b.chart.series[0].marks.len(), 10);
    assert!(b.chart.series[0].marks.iter().all(|k| k.y == Some(10.0)));
    // one value, and too many ids to list: bins keep their key
    assert_eq!(charts::histogram(&[1], &[5.0], None, "x", 0).unwrap().chart.series[0].marks.len(), 1);
    let capped = charts::histogram(&fids, &vals, Some(4), "x", 10).unwrap();
    assert!(capped.chart.series[0].marks.iter().all(|k| k.fids.is_empty() && k.bin.is_some()));
}

#[test]
fn box_plot_has_tukey_whiskers_and_outliers() {
    let items: Vec<(i64, f64)> = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 100.0].iter().enumerate().map(|(i, v)| (i as i64 + 1, *v)).collect();
    let b = charts::boxplot(vec![("All".into(), items)], "x", None, usize::MAX).unwrap();
    let m = &b.chart.series[0].marks;
    let bx = &m[0];
    assert_eq!((bx.y, bx.y2, bx.median), (Some(3.25), Some(7.75), Some(5.5)));
    assert_eq!((bx.lo, bx.hi), (Some(1.0), Some(9.0))); // 1.5 * IQR fence is 14.5, so 100 is an outlier
    assert_eq!(m.len(), 2);
    assert_eq!((m[1].y, m[1].fids.clone()), (Some(100.0), vec![10]));
}

#[test]
fn scatter_trend_is_least_squares_and_qq_line_passes_the_quartiles() {
    let pts: Vec<(i64, f64, f64)> = (0..50).map(|i| (i as i64, i as f64, 3.0 + 2.0 * i as f64)).collect();
    let b = charts::scatter(&pts, "x", "y", true, usize::MAX).unwrap();
    let t = &b.chart.series[1];
    assert_eq!(t.style, SeriesStyle::Line);
    assert_eq!((t.marks[0].y, t.marks[1].y), (Some(3.0), Some(101.0)));
    assert!(b.notes.iter().any(|n| n.contains("R-squared 1")));
    let items: Vec<(i64, f64)> = (0..200).map(|i| (i as i64, qnorm((i as f64 + 0.5) / 200.0) * 2.0 + 10.0)).collect();
    let q = charts::qq(&items, "x").unwrap();
    let line = &q.chart.series[1].marks;
    let slope = (line[1].y.unwrap() - line[0].y.unwrap()) / (line[1].x.unwrap() - line[0].x.unwrap());
    assert!((slope - 2.0).abs() < 0.02, "slope {slope}");
}

#[test]
fn correlation_matches_the_hand_value_and_refuses_a_constant_column() {
    let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let b = vec![2.0, 4.0, 5.0, 4.0, 5.0];
    // R: cor(1:5, c(2,4,5,4,5)) = 0.7745967
    assert!((charts::pearson(&a, &b).unwrap() - 0.7745966692414834).abs() < 1e-12);
    let h = charts::correlation_heatmap(&[1, 2, 3, 4, 5], &[a.clone(), b], &["a".into(), "b".into()], 2).unwrap();
    assert_eq!(h.chart.series[0].marks.len(), 4);
    assert_eq!(h.dropped[0].count, 2);
    assert!(charts::correlation_heatmap(&[1, 2, 3], &[a[..3].to_vec(), vec![1.0, 1.0, 1.0]], &["a".into(), "b".into()], 0).unwrap_err().to_string().contains("no variation"));
}

#[test]
fn svg_has_a_data_mark_per_selectable_mark_is_valid_and_script_free() {
    let vals: Vec<f64> = (1..=100).map(|x| x as f64).collect();
    let fids: Vec<i64> = (1..=100).collect();
    let pts: Vec<(i64, f64, f64)> = (0..60).map(|i| (i, i as f64, (i * i) as f64)).collect();
    let cols = vec![(0..30).map(|i| i as f64).collect::<Vec<_>>(), (0..30).map(|i| (i * 2 % 7) as f64).collect(), (0..30).map(|i| (i % 5) as f64).collect()];
    let f30: Vec<i64> = (0..30).collect();
    let names: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
    let all = vec![
        charts::histogram(&fids, &vals, None, "x <&> \"q\"", usize::MAX).unwrap(),
        charts::boxplot(vec![("a".into(), fids.iter().zip(&vals).map(|(f, v)| (*f, *v)).collect()), ("b".into(), vec![(1, 5.0), (2, 6.0), (3, 50.0)])], "x", Some("g"), usize::MAX).unwrap(),
        charts::scatter(&pts, "x", "y", true, usize::MAX).unwrap(),
        charts::qq(&fids.iter().zip(&vals).map(|(f, v)| (*f, *v)).collect::<Vec<_>>(), "x").unwrap(),
        charts::bar(&[(1, "a".into(), None), (2, "b".into(), None), (3, "a".into(), None)], "count", "cat", None, usize::MAX).unwrap(),
        charts::line(&pts, "x", "y").unwrap(),
        charts::scatter_matrix(&f30, &cols, &names, 0).unwrap(),
        charts::correlation_heatmap(&f30, &cols, &names, 0).unwrap(),
    ];
    for b in all {
        for style in [ChartStyle::default(), ChartStyle::paper()] {
            let svg = render_svg(&b.chart, 640.0, 400.0, &style);
            assert!(!svg.contains("<script") && !svg.contains("onclick") && !svg.contains("javascript:"), "{:?}", b.chart.kind);
            engine::layout::export::parse(&svg).unwrap_or_else(|e| panic!("{:?} is not valid SVG: {e}\n{svg}", b.chart.kind));
            let n = svg.matches("data-mark=\"").count();
            assert_eq!(n, marks(&b.chart), "{:?}: one data-mark per selectable mark", b.chart.kind);
            for i in [0, n.saturating_sub(1)] {
                assert!(svg.contains(&format!("data-mark=\"{i}\"")));
            }
            let lower = svg.to_lowercase();
            assert!(lower.contains("#0072b2") || lower.contains("#e69f00") || lower.contains("#d55e00"), "Okabe-Ito colours");
        }
        assert!(chart_svg(&b.chart, 640.0, 400.0).contains("&lt;&amp;&gt;") || b.chart.kind != ChartKind::Histogram, "titles are escaped");
    }
    let t = nice_ticks(0.0, 97.0, 6);
    assert_eq!(t, vec![0.0, 20.0, 40.0, 60.0, 80.0]);
}

#[test]
fn a_card_reports_n_used_and_every_dropped_row_and_depends_on_the_layer_version() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = layer_from(
        &e,
        dir.path(),
        "SELECT i AS id, CASE WHEN i % 10 = 0 THEN NULL ELSE (i * 1.5)::DOUBLE END AS v, CASE WHEN i % 2 = 0 THEN 'even' ELSE 'odd' END AS g, ST_Point(i, i) AS geom FROM range(1, 101) t(i)",
    );
    assert!(e.is_stat_tool("chart_histogram"));
    let r = e.run_stat_card(&card("chart_histogram", &l, json!({"field": "v"}), None)).unwrap();
    assert_eq!(r.n_used, Some(90));
    assert_eq!(r.dropped.iter().map(|d| d.count).sum::<u64>(), 10);
    assert!(r.dropped[0].reason.contains("missing or non-numeric"));
    assert_eq!(r.charts.len(), 1);
    assert_eq!(r.tables[0].title, "Histogram bins");
    assert_eq!(r.data_version.get(&l), Some(&e.layer(&l).unwrap().version));
    // box by group: two groups with their own boxes
    let b = e.run_stat_card(&card("chart_box", &l, json!({"field": "v", "group": "g"}), None)).unwrap();
    assert_eq!(b.charts[0].x.categories, vec!["even".to_string(), "odd".to_string()]);
    // selected features only
    let some = e.run_stat_card(&card("chart_histogram", &l, json!({"field": "v"}), Some(vec![1, 2, 3, 4]))).unwrap();
    assert_eq!(some.n_used, Some(4));
    // refusals are sentences
    assert!(e.run_stat_card(&card("chart_histogram", &l, json!({}), None)).unwrap_err().to_string().contains("Choose a field"));
    assert!(e.run_stat_card(&card("chart_bar", &l, json!({"category": "g", "agg": "sum"}), None)).unwrap_err().to_string().contains("value field"));
    assert!(e.run_stat_card(&card("chart_heatmap", &l, json!({"f1": "v", "f2": "v"}), None)).unwrap_err().to_string().contains("twice"));
}

#[test]
fn a_large_scatter_becomes_a_density_grid_that_says_so_and_a_cell_resolves_to_its_features() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = layer_from(&e, dir.path(), "SELECT i AS id, (i % 997)::DOUBLE AS a, (i % 13)::DOUBLE AS b, ST_Point(i % 100, i % 50) AS geom FROM range(1, 6501) t(i)");
    let c = card("chart_scatter", &l, json!({"x": "a", "y": "b", "trend": false}), None);
    let r = e.run_stat_card(&c).unwrap();
    assert_eq!(r.n_used, Some(6500));
    assert!(r.notes.iter().any(|n| n.contains("density grid")), "{:?}", r.notes);
    let cells = &r.charts[0].series[0].marks;
    assert!(cells.len() <= 2500 && cells.iter().all(|m| m.bin.is_some() && m.value.is_some()));
    assert_eq!(cells.iter().map(|m| m.value.unwrap() as usize).sum::<usize>(), 6500);
    let first = &cells[0];
    let ids = e.stat_chart_bin_fids(&c, first.bin.as_ref().unwrap()).unwrap();
    assert_eq!(ids.len(), first.value.unwrap() as usize);
    assert!(e.stat_chart_bin_fids(&c, "sc:999,999").is_err());
    // 5,000 points or fewer are drawn one by one
    let few = e.run_stat_card(&card("chart_scatter", &l, json!({"x": "a", "y": "b"}), Some((1..=100).collect()))).unwrap();
    assert_eq!(few.charts[0].series[0].marks.len(), 100);
    assert_eq!(few.charts[0].series[0].marks[0].fids.len(), 1);
    assert!(few.charts[0].series.iter().any(|s| s.name == "Trend"));
}

#[test]
fn the_chart_tools_also_work_as_nodes_and_give_their_numbers_as_a_table() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = layer_from(&e, dir.path(), "SELECT i AS id, i::DOUBLE AS v, ST_Point(i, i) AS geom FROM range(1, 51) t(i)");
    let g: engine::graph::Graph = serde_json::from_value(json!({
        "nodes": [{"id": "a", "tool": "input_layer", "params": {"layer": l}}, {"id": "h", "tool": "chart_histogram", "params": {"field": "v", "bins": 5}}],
        "edges": [{"source": "a", "target": "h", "targetHandle": "in"}]
    }))
    .unwrap();
    let res = e.run_graph(&g, Some("h"), &mut |_| {}).unwrap();
    let r = res.iter().find(|r| r.id == "h").unwrap();
    assert_eq!(r.status, engine::graph::Status::Done, "{:?}", r.error);
    assert_eq!(r.rows, Some(5));
    // every chart tool is a Statistics > Charts ToolSpec
    let specs = e.tool_specs();
    for id in charts::CHART_TOOLS {
        let s = specs.iter().find(|s| s.spec.id == id).unwrap_or_else(|| panic!("{id} is registered"));
        assert_eq!(s.spec.category, "Statistics › Charts");
    }
}

#[test]
fn old_chart_specs_without_the_new_fields_still_read() {
    let old = json!({"kind": "histogram", "title": "t", "x": {"label": "x"}, "y": {"label": "y"}, "series": [{"name": "s", "marks": [{"x": 1.0, "y": 2.0}]}], "notes": []});
    let c: ChartSpec = serde_json::from_value(old).unwrap();
    assert_eq!(c.series[0].style, SeriesStyle::Marks);
    assert!(c.x.categories.is_empty());
    let _ = Map::<String, serde_json::Value>::new();
}
