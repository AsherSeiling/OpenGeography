//! Georeferencing control points (GR1): storage, project round trip, old projects, CSV and QGIS files.

use engine::georef::{parse_control_file, to_csv, ControlPoint, ControlSet};
use engine::Engine;

fn pt(id: u32, pixel: f64, line: f64, x: f64, y: f64) -> ControlPoint {
    ControlPoint { id, pixel, line, x, y, enabled: true }
}

fn set(name: &str) -> ControlSet {
    ControlSet {
        id: format!("cs-{name}"),
        name: name.into(),
        target_crs: "EPSG:26914".into(),
        image: Some("/tmp/scan.png".into()),
        points: vec![pt(1, 0.0, 0.0, 600000.0, 3900000.0), pt(2, 100.5, 0.25, 600300.5, 3900000.0), pt(3, 0.0, 100.0, 600000.0, 3899700.0)],
    }
}

#[test]
fn set_round_trips_through_the_project_and_old_projects_still_open() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("p.gproj");
    let e = Engine::new().unwrap();
    let mut s = set("Scan 1950");
    s.points[1].enabled = false;
    e.save_control_set(&s).unwrap();
    e.save_project(&proj, "{\"version\":2}").unwrap();

    let e2 = Engine::new().unwrap();
    e2.open_project(&proj).unwrap();
    assert_eq!(e2.list_control_sets().unwrap(), vec![s.clone()]);
    assert_eq!(e2.control_set_by_name(" scan 1950 ").unwrap(), s);

    // An older project has no control point table at all: it opens, lists nothing, and takes a new set.
    let old = dir.path().join("old.gproj");
    let e3 = Engine::new().unwrap();
    e3.with_conn_for_tests(|c| Ok(c.execute_batch("DROP TABLE __control_sets")?)).unwrap();
    e3.save_project(&old, "{\"version\":2}").unwrap();
    let e4 = Engine::new().unwrap();
    e4.open_project(&old).unwrap();
    assert!(e4.list_control_sets().unwrap().is_empty());
    e4.save_control_set(&s).unwrap();
    assert_eq!(e4.list_control_sets().unwrap().len(), 1);
}

#[test]
fn validation_reads_as_sentences() {
    let e = Engine::new().unwrap();
    let mut s = set("a");
    s.target_crs = "  ".into();
    assert!(e.save_control_set(&s).unwrap_err().to_string().contains("never guessed"));
    s.target_crs = "EPSG:not-a-code".into();
    assert!(e.save_control_set(&s).unwrap_err().to_string().contains("can't be read"));
    let mut s = set("a");
    s.points[1].id = 1;
    assert!(e.save_control_set(&s).unwrap_err().to_string().contains("share the id 1"));
    let mut s = set("a");
    s.points[0].x = f64::NAN;
    assert!(e.save_control_set(&s).unwrap_err().to_string().contains("not a number"));
    // Names are unique ignoring case; a missing name lists what exists.
    e.save_control_set(&set("Alpha")).unwrap();
    let mut dup = set("alpha");
    dup.id = "other".into();
    assert!(e.save_control_set(&dup).unwrap_err().to_string().contains("already exists"));
    let err = e.control_set_by_name("Beta").unwrap_err().to_string();
    assert!(err.contains("no control point set named 'Beta'") && err.contains("Alpha"), "{err}");
    e.delete_control_set("cs-Alpha").unwrap();
    assert!(e.control_set_by_name("Alpha").unwrap_err().to_string().contains("has none yet"));
}

#[test]
fn csv_states_the_crs_and_round_trips() {
    let mut s = set("csv");
    s.points[2].enabled = false;
    s.points[1].pixel = 100.123456789012;
    let text = to_csv(&s);
    assert!(text.contains("# crs: EPSG:26914"), "{text}");
    assert!(text.contains("top-left corner of the top-left pixel"));
    let back = parse_control_file(&text).unwrap();
    assert_eq!(back.format, "csv");
    assert_eq!(back.crs.as_deref(), Some("EPSG:26914"));
    assert_eq!(back.points, s.points);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pts.csv");
    let e = Engine::new().unwrap();
    e.export_control_set(&s, &path).unwrap();
    assert_eq!(e.read_control_file(&path, None).unwrap().points, s.points);
    assert!(!dir.path().join("pts.csv.saving").exists());
}

#[test]
fn csv_without_a_crs_reports_none_instead_of_guessing() {
    let p = parse_control_file("id,pixel,line,x,y,enabled\n1,0,0,10,20,1\n2,5,5,15,25,0\n").unwrap();
    assert_eq!(p.crs, None);
    assert_eq!(p.points.len(), 2);
    assert!(!p.points[1].enabled);
    assert!(parse_control_file("id,pixel,line,x,y\n1,a,0,10,20\n").unwrap_err().to_string().contains("Line 2"));
    assert!(parse_control_file("# crs: EPSG:4326\n").unwrap_err().to_string().contains("no control points"));
    assert!(parse_control_file("foo,bar\n1,2\n").unwrap_err().to_string().contains("not one this version reads"));
}

#[test]
fn qgis_points_are_converted_explicitly() {
    let text = "#CRS: EPSG:4326\nmapX,mapY,sourceX,sourceY,enable,dX,dY,residual\n-100.5,40.25,12.5,-30,1,0,0,0\n-100.0,40.0,200,-400.75,0,0,0,0\n";
    let p = parse_control_file(text).unwrap();
    assert_eq!(p.format, "qgis");
    assert_eq!(p.crs.as_deref(), Some("EPSG:4326"));
    assert_eq!(p.points, vec![pt(1, 12.5, 30.0, -100.5, 40.25), ControlPoint { enabled: false, ..pt(2, 200.0, 400.75, -100.0, 40.0) }]);
    assert!(p.notes[0].contains("-sourceY"));
    // Without a "#CRS:" line the file names no CRS and the caller has to ask.
    let p = parse_control_file("mapX,mapY,sourceX,sourceY,enable,dX,dY,residual\n1,2,3,-4,1,0,0,0\n").unwrap();
    assert_eq!(p.crs, None);
    // Positive sourceY is not QGIS's image convention: refused rather than flipped silently.
    assert!(parse_control_file("#CRS: EPSG:4326\nmapX,mapY,sourceX,sourceY,enable\n1,2,3,4,1\n").unwrap_err().to_string().contains("positive sourceY"));
}

#[test]
fn a_qgis_file_with_positions_outside_the_image_is_refused() {
    // Made on an already georeferenced raster: sourceX/sourceY hold map coordinates (southern hemisphere: sourceY is negative too).
    let text = "#CRS: EPSG:32614\nmapX,mapY,sourceX,sourceY,enable,dX,dY,residual\n600100,3900100,600000,-3900000,1,0,0,0\n";
    assert!(parse_control_file(text).is_ok(), "no image size known: the note warns instead");
    let p = engine::georef::parse_control_file_for(text, Some((800.0, 600.0)));
    let e = p.unwrap_err().to_string();
    assert!(e.contains("map coordinates") && e.contains("800 by 600"), "{e}");
    assert!(engine::georef::parse_control_file_for("mapX,mapY,sourceX,sourceY,enable\n1,2,10,-20,1\n", Some((800.0, 600.0))).is_ok());
    let note = &parse_control_file(text).unwrap().notes[0];
    assert!(note.contains("already georeferenced raster") && note.contains("image pixels"), "{note}");
}

#[test]
fn reproject_is_an_explicit_transform() {
    let e = Engine::new().unwrap();
    let s = ControlSet { target_crs: "EPSG:4326".into(), points: vec![pt(1, 0.0, 0.0, -99.0, 0.0)], ..set("geo") };
    let u = e.reproject_control_set(&s, "EPSG:32614").unwrap();
    assert_eq!(u.target_crs, "EPSG:32614");
    // -99 is the central meridian of UTM 14: easting 500000.
    assert!((u.points[0].x - 500_000.0).abs() < 0.01, "{}", u.points[0].x);
    assert!(u.points[0].y.abs() < 0.01);
    assert!(e.reproject_control_set(&s, "bogus").is_err());
}
