//! R6 leftovers: Tissot circles (R-H11), the CRS statement for layout text (R-I5), `same_crs` for the source panel (R-H10).

use engine::Engine;

/// (lat, k_east, k_north, area_ratio, circle count) per row of a Tissot layer.
fn rows(e: &Engine, table: &str) -> Vec<(f64, f64, f64, f64)> {
    e.with_conn_for_tests(|c| {
        let mut st = c.prepare(&format!("SELECT lat, k_east, k_north, area_ratio FROM \"{table}\" ORDER BY lat, lon"))?;
        let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(r)
    })
    .unwrap()
}

#[test]
fn r_h11_albers_is_equal_area_everywhere() {
    let e = Engine::new().unwrap();
    // Albers Equal Area (EPSG:5070): every circle keeps its area, so area_ratio is 1 wherever it is defined.
    let (l, _skipped) = e.tissot_layer("EPSG:5070", 15.0, 100.0).unwrap();
    let r = rows(&e, &e.layer(&l.id).unwrap().table);
    assert!(r.len() > 20);
    for (lat, _, _, a) in &r {
        assert!((a - 1.0).abs() < 0.001, "area ratio {a} at {lat}°");
    }
}

#[test]
fn r_h11_web_mercator_scale_grows_as_one_over_cos_lat() {
    let e = Engine::new().unwrap();
    let (l, _) = e.tissot_layer("EPSG:3857", 15.0, 100.0).unwrap();
    let r = rows(&e, &e.layer(&l.id).unwrap().table);
    let at60: Vec<_> = r.iter().filter(|x| (x.0 - 60.0).abs() < 1e-9).collect();
    assert!(!at60.is_empty());
    for (_, ke, kn, _) in at60 {
        assert!((ke - 2.0).abs() < 0.01 && (kn - 2.0).abs() < 0.01, "k {ke} {kn} at 60°; expected 1/cos 60° = 2");
    }
    // Circles stay round (conformal): east and north scale agree everywhere.
    // (on the ellipsoid the sphere-based Web Mercator differs by ≤ 0.7 % between the two directions up to 75°)
    assert!(r.iter().filter(|x| x.0.abs() <= 75.0).all(|(lat, ke, kn, _)| { let ok = (ke / kn - 1.0).abs() < 1e-2; if !ok { eprintln!("lat {lat} ke {ke} kn {kn}"); } ok }));
}

#[test]
fn r_h11_polar_stereographic_has_unit_scale_at_its_standard_parallel() {
    let e = Engine::new().unwrap();
    let (l, skipped) = e.tissot_layer("EPSG:3031", 15.0, 100.0).unwrap();
    let r = rows(&e, &e.layer(&l.id).unwrap().table);
    let at71: Vec<_> = r.iter().filter(|x| (x.0 + 75.0).abs() < 1e-9 || (x.0 + 60.0).abs() < 1e-9).collect();
    assert!(!at71.is_empty());
    // The standard parallel is 71°S (k = 1); 71° is not on a 15° grid, so check the sphere formula
    // k = (1 + sin 71°) / (1 + sin |φ|) at -75° (the ellipsoid differs by < 0.5 %).
    let want = (1.0 + 71f64.to_radians().sin()) / (1.0 + 75f64.to_radians().sin());
    for (lat, ke, ..) in r.iter().filter(|x| (x.0 + 75.0).abs() < 1e-9) {
        assert!((ke / want - 1.0).abs() < 0.005, "k {ke} at {lat}°, expected ≈ {want}");
    }
    let _ = skipped;
}

#[test]
fn r_h11_unprojectable_points_are_counted_and_inputs_are_checked() {
    let e = Engine::new().unwrap();
    // A UTM zone cannot represent the far side of the Earth: those grid points are left out and counted.
    let (l, skipped) = e.tissot_layer("EPSG:26914", 30.0, 100.0).unwrap();
    assert!(e.layer(&l.id).unwrap().feature_count > 0);
    let total_grid = ((180.0_f64 / 30.0).floor() as usize + 1) * 12;
    assert!(skipped + e.layer(&l.id).unwrap().feature_count as usize <= total_grid);
    assert!(e.tissot_layer("EPSG:4326", 0.5, 100.0).unwrap_err().to_string().contains("spacing"));
    assert!(e.tissot_layer("EPSG:4326", 15.0, 0.0).unwrap_err().to_string().contains("radius"));
    assert!(e.tissot_layer("nonsense", 15.0, 100.0).is_err());
}

#[test]
fn r_i5_crs_statement_for_layout_text() {
    let s = engine::crs::statement("EPSG:26914");
    assert!(s.crs.contains("UTM zone 14N") && s.crs.contains("(EPSG:26914)"), "{}", s.crs);
    assert_eq!(s.datum, "North American Datum 1983");
    assert_eq!(s.units.to_lowercase(), "metre");
    let c = engine::crs::statement("+proj=utm +zone=14 +datum=NAD83 +units=m");
    assert_eq!(c.crs, "Custom CRS");
    assert_eq!(c.datum, "North American Datum 1983");
    let g = engine::crs::statement("EPSG:4326");
    assert_eq!(g.units.to_lowercase(), "degree");
    // Nothing readable → no error text in a printed map.
    let n = engine::crs::statement("");
    assert!(n.crs.is_empty() && n.datum.is_empty() && n.units.is_empty());
    assert_eq!(engine::crs::statement("garbage").datum, "");
}

#[test]
fn r_h10_same_crs_by_meaning() {
    let e = Engine::new().unwrap();
    assert!(e.same_crs("EPSG:26915", "urn:ogc:def:crs:EPSG::26915"));
    assert!(!e.same_crs("EPSG:26915", "EPSG:26715"));
    assert!(!e.same_crs("EPSG:26915", "EPSG:3857"));
}
