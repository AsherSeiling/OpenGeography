//! R8 world view (RD-10, R-W cases): whole-Earth grids, the cut meridian, reference lines, the bundled outline.

use engine::display::{display_info, world_window};
use engine::Engine;

const LCC: &str = "+proj=lcc +lat_1=33 +lat_2=45 +lat_0=39 +lon_0=-96 +datum=NAD83 +units=m";
const ALBERS_150: &str = "+proj=aea +lat_1=20 +lat_2=60 +lat_0=0 +lon_0=150 +datum=WGS84 +units=m";
const ALBERS_96: &str = "+proj=aea +lat_1=20 +lat_2=60 +lat_0=0 +lon_0=-96 +datum=WGS84 +units=m";

fn wait_display(e: &std::sync::Arc<Engine>, layer: &str, crs: &str) {
    let st = e.build_display_blocking(layer, crs).unwrap();
    assert!(st.ready && st.error.is_none(), "{st:?}");
}

#[test]
fn r_w1_the_world_grid_of_a_conic_covers_the_globe_and_reports_its_limit() {
    let plain = display_info(LCC, Some([-100.0, 30.0, -90.0, 40.0])).unwrap();
    assert!(plain.world.is_none());
    let w = display_info(&format!("world:{LCC}"), None).unwrap();
    let info = w.world.clone().unwrap();
    assert_eq!(info.lon0, -96.0);
    assert!(info.lat_north == 90.0 && info.lat_south < -40.0 && info.lat_south > -60.0, "{info:?}");
    assert!(info.note.as_deref().unwrap().contains("stops at"), "{:?}", info.note);
    // Different key from the area-of-use view of the same CRS, and a grid much larger than the CONUS one.
    assert_ne!(w.crs_key, plain.crs_key);
    let span = |g: &engine::display::Grid| g.extent[2] - g.extent[0];
    assert!(span(&w.grid) > 4.0 * span(&plain.grid), "world {} vs conus {}", span(&w.grid), span(&plain.grid));
    // The grid is square, so the whole image of the globe lies inside it: the cut meridian's projections are inside.
    let e = Engine::new().unwrap();
    let (x, y) = e.transform_point(-96.0 + 179.99, 0.0, "EPSG:4326", &format!("world:{LCC}")).unwrap();
    assert!(x > w.grid.extent[0] && x < w.grid.extent[2] && y > w.grid.extent[1] && y < w.grid.extent[3], "({x}, {y}) in {:?}", w.grid.extent);
}

#[test]
fn r_w4_projections_without_a_whole_earth_say_so_and_limits_are_stated() {
    for bad in ["+proj=utm +zone=14 +datum=NAD83", "+proj=ortho +lat_0=40 +lon_0=-100 +datum=WGS84"] {
        let e = display_info(&format!("world:{bad}"), None).unwrap_err().to_string();
        assert!(e.contains("world") || e.contains("Earth"), "{e}");
    }
    // Mercator and polar stereographic stop at a stated latitude.
    let m = world_window("+proj=merc +lon_0=0 +datum=WGS84").unwrap();
    assert_eq!((m.lat_south, m.lat_north), (-85.0, 85.0));
    let p = world_window("+proj=stere +lat_0=-90 +lat_ts=-71 +lon_0=0 +datum=WGS84").unwrap();
    assert_eq!(p.lat_north, 30.0);
    // A projection that already shows the whole Earth (Robinson) has no limit and no note.
    let r = world_window("+proj=robin +lon_0=150 +datum=WGS84").unwrap();
    assert_eq!((r.lat_south, r.lat_north, r.lon0), (-90.0, 90.0, 150.0));
    assert!(r.note.is_none());
    // Web Mercator and geographic CRSs are unchanged by the request.
    assert!(display_info("world:EPSG:4326", None).unwrap().world.is_none());
    assert!(display_info("world:EPSG:3857", None).unwrap().world.is_none());
}

#[test]
fn r_w3_land_is_cut_at_the_far_meridian_with_no_streaks_and_equal_area_is_kept() {
    let e = std::sync::Arc::new(Engine::new().unwrap());
    let land = e.world_outline().unwrap();
    assert!(land.feature_count > 100);
    let mut areas = vec![];
    for (crs, name) in [(ALBERS_96, "lon_0 -96"), (ALBERS_150, "lon_0 150")] {
        let spec = format!("world:{crs}");
        let di = e.crs_display_info(&spec, None).unwrap();
        wait_display(&e, &land.id, &spec);
        let table = format!("__disp_{}_{}", land.id, di.crs_key);
        let (area, max_w, n): (f64, f64, i64) = e
            .with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT sum(ST_Area(__d)), max(CASE WHEN __fid IN (SELECT __fid FROM \"{src}\" WHERE ST_YMax(geom) < -60) THEN 0 ELSE ST_XMax(__d) - ST_XMin(__d) END), count(*) FROM \"{table}\"", src = e.layer(&land.id).unwrap().table), [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?))
            .unwrap();
        // No piece is wider than the map itself (a wrap-around streak would span it); Antarctica legitimately spans a conic's width.
        let map_w = di.grid.extent[2] - di.grid.extent[0];
        assert!(max_w < map_w * 0.9, "{name}: widest piece {max_w} of a {map_w} wide map");
        assert!(n >= land.feature_count, "{name}: {n} pieces");
        areas.push(area);
    }
    // The same land cut at two different meridians has the same area in an equal-area CRS.
    assert!((areas[0] / areas[1] - 1.0).abs() < 2e-3, "{areas:?}");
    // And it matches the geodesic area of the source (110m polygons, straight edges: within 1 %).
    let truth: f64 = e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT sum(ST_Area_Spheroid(geom)) FROM \"{}\"", e.layer(&land.id).unwrap().table), [], |r| r.get(0))?)).unwrap();
    assert!((areas[0] / truth - 1.0).abs() < 0.01, "projected {} vs geodesic {truth}", areas[0]);
}

#[test]
fn r_w5_w6_w7_reference_lines() {
    let e = Engine::new().unwrap();
    let layers = e.world_reference(&format!("world:{LCC}"), 15.0).unwrap();
    assert_eq!(layers.len(), 2);
    let kinds = |id: &str| -> Vec<(String, f64)> {
        let t = e.layer(id).unwrap().table;
        e.with_conn_for_tests(|c| {
            let mut st = c.prepare(&format!("SELECT kind, value FROM \"{t}\" ORDER BY kind, value"))?;
            Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?)
        })
        .unwrap()
    };
    let g = kinds(&layers[0].id);
    assert_eq!(g.iter().filter(|k| k.0 == "meridian").count(), 24);
    assert!(g.iter().any(|k| k.0 == "equator"));
    assert!(g.iter().filter(|k| k.0 == "tropic").count() == 2);
    // Parallels stop at the conic's southern limit, not at the pole it blows up toward.
    let south = world_window(&display_info(LCC, Some([-100.0, 30.0, -90.0, 40.0])).unwrap().proj4).unwrap().lat_south;
    assert!(g.iter().filter(|k| k.0 == "parallel").all(|k| k.1 >= south - 1e-9));
    let r = kinds(&layers[1].id);
    let std: Vec<f64> = r.iter().filter(|k| k.0 == "standard parallel").map(|k| k.1).collect();
    assert_eq!(std, vec![33.0, 45.0]);
    let cm: Vec<f64> = r.iter().filter(|k| k.0 == "central meridian").map(|k| k.1).collect();
    assert_eq!(cm, vec![-96.0]);
    assert!(r.iter().any(|k| k.0 == "outline"));
    assert!(e.world_reference(LCC, 2.0).is_err());
    // Scale is 1 on the standard parallels: the engine's own factors agree (R-W6).
    let (x, y) = e.transform_point(-100.0, 33.0, "EPSG:4326", LCC).unwrap();
    let (k, _) = e.map_factors(LCC, x, y).unwrap();
    assert!((k - 1.0).abs() < 1e-4, "k on the standard parallel {k}");
}

#[test]
fn r_w8_the_bundled_outline_is_small_cited_and_offline() {
    let size = include_str!("../assets/ne_110m_land.geojson").len();
    assert!(size <= 300 * 1024, "{size} bytes");
    let e = Engine::new().unwrap();
    let l = e.world_outline().unwrap();
    assert_eq!(l.crs.as_deref(), Some("EPSG:4326"));
    let list = e.sources().unwrap();
    let s = list.iter().find(|s| s.citation.key == "natural-earth-110m-land").expect("cited");
    assert!(s.citation.license.contains("Public domain"));
    assert_eq!(s.uses.len(), 1);
}

#[test]
fn r_w2_a_long_straight_edge_is_densified_so_it_curves() {
    let e = std::sync::Arc::new(Engine::new().unwrap());
    let l = e.create_features("Parallel", &[("LINESTRING(-100 45, 80 45)".to_string(), serde_json::Map::new())]).unwrap();
    let spec = format!("world:{LCC}");
    let di = e.crs_display_info(&spec, None).unwrap();
    wait_display(&e, &l.id, &spec);
    let table = format!("__disp_{}_{}", l.id, di.crs_key);
    let n: i64 = e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT sum(ST_NPoints(__d)) FROM \"{table}\""), [], |r| r.get(0))?)).unwrap();
    assert!(n >= 150, "{n} vertices: a chord, not a curve");
    // Every vertex of the drawn line lies on the 45th parallel (inverse of the projection).
    let pts: Vec<(f64, f64)> = e
        .with_conn_for_tests(|c| {
            let mut st = c.prepare(&format!("SELECT ST_X(p), ST_Y(p) FROM (SELECT unnest(ST_Dump(ST_Points(__d))).geom AS p FROM \"{table}\")"))?;
            Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?)
        })
        .unwrap();
    for (x, y) in pts.iter().step_by(10) {
        let (_, lat) = e.transform_point(*x, *y, LCC, "EPSG:4326").unwrap();
        assert!((lat - 45.0).abs() < 1e-6, "vertex at lat {lat}");
    }
}

/// TP-5: the 15° Tissot circles drawn in Mollweide with the cut at 84°E gave "TopologyException: side location conflict".
#[test]
fn tp5_tissot_circles_draw_in_a_mollweide_cut_at_the_far_meridian() {
    let e = std::sync::Arc::new(Engine::new().unwrap());
    let (l, _) = e.tissot_layer("EPSG:4326", 15.0, 500.0).unwrap();
    wait_display(&e, &l.id, "world:+proj=moll +lon_0=-96 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs");
}
