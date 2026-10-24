//! Map display in any CRS (docs/MAP_PROJECTION_PLAN.md M1, acceptance P1–P6).

use engine::display::{self, DisplayInfo};
use engine::Engine;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn write_fixture(path: &Path, select: &str, driver: &str, srs: Option<&str>) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let srs = srs.map(|s| format!(", SRS '{s}'")).unwrap_or_default();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER '{driver}'{srs}, USE_TMP_FILE false)", path.display())).unwrap();
}

fn import_one(e: &Engine, dir: &Path, name: &str, select: &str, srs: &str) -> String {
    let p = dir.join(format!("{name}.gpkg"));
    write_fixture(&p, select, "GPKG", Some(srs));
    e.import(&p).unwrap()[0].id.clone()
}

// ---------- a minimal MVT reader ----------

fn varint(b: &[u8], i: &mut usize) -> u64 {
    let (mut v, mut shift) = (0u64, 0);
    loop {
        let byte = b[*i];
        *i += 1;
        v |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return v;
        }
        shift += 7;
    }
}

/// (field number, varint value or length-delimited bytes)
fn pb_fields(b: &[u8]) -> Vec<(u64, u64, Vec<u8>)> {
    let (mut i, mut out) = (0, Vec::new());
    while i < b.len() {
        let key = varint(b, &mut i);
        match key & 7 {
            0 => out.push((key >> 3, varint(b, &mut i), vec![])),
            2 => {
                let n = varint(b, &mut i) as usize;
                out.push((key >> 3, 0, b[i..i + n].to_vec()));
                i += n;
            }
            w => panic!("unexpected wire type {w}"),
        }
    }
    out
}

/// (feature id, geometry commands) of every feature, sorted: the thinned
/// features come out of a GROUP BY in no fixed order, in the old code too.
fn feature_set(tile: &[u8]) -> Vec<(u64, Vec<u8>)> {
    let mut out = Vec::new();
    for (f, _, layer) in pb_fields(tile) {
        if f == 3 {
            for (lf, _, feat) in pb_fields(&layer) {
                if lf == 2 {
                    let p = pb_fields(&feat);
                    let id = p.iter().find(|g| g.0 == 1).map_or(0, |g| g.1);
                    out.push((id, p.into_iter().find(|g| g.0 == 4).map(|g| g.2).unwrap_or_default()));
                }
            }
        }
    }
    out.sort();
    out
}

/// Each feature of the first layer as parts (rings, lines or points) of (x, y) tile coordinates.
fn decode(tile: &[u8]) -> Vec<Vec<Vec<(i64, i64)>>> {
    let mut feats = Vec::new();
    for (f, _, layer) in pb_fields(tile) {
        if f != 3 {
            continue;
        }
        for (lf, _, feat) in pb_fields(&layer) {
            if lf != 2 {
                continue;
            }
            let geom = pb_fields(&feat).into_iter().find(|g| g.0 == 4).map(|g| g.2).unwrap_or_default();
            let (mut i, mut cur, mut parts, mut part) = (0, (0i64, 0i64), Vec::<Vec<(i64, i64)>>::new(), Vec::new());
            let zz = |v: u64| ((v >> 1) as i64) ^ -((v & 1) as i64);
            while i < geom.len() {
                let cmd = varint(&geom, &mut i);
                match (cmd & 7, cmd >> 3) {
                    (1 | 2, n) => {
                        for _ in 0..n {
                            if cmd & 7 == 1 && !part.is_empty() {
                                parts.push(std::mem::take(&mut part));
                            }
                            cur = (cur.0 + zz(varint(&geom, &mut i)), cur.1 + zz(varint(&geom, &mut i)));
                            part.push(cur);
                        }
                    }
                    _ => {} // ClosePath
                }
            }
            if !part.is_empty() {
                parts.push(part);
            }
            feats.push(parts);
        }
    }
    feats
}

/// Tile (x, y) and the position of (px, py) inside it in 4096-unit tile coordinates.
fn locate(di: &DisplayInfo, z: u32, px: f64, py: f64) -> (u32, u32, f64, f64) {
    let span = di.grid.resolutions[z as usize] * di.grid.tile_size as f64;
    let (x, y) = (((px - di.grid.origin[0]) / span).floor(), ((di.grid.origin[1] - py) / span).floor());
    let b = di.grid.tile_bounds(z, x as u32, y as u32).unwrap();
    (x as u32, y as u32, (px - b[0]) / span * 4096.0, (b[3] - py) / span * 4096.0)
}

fn disp_tables(e: &Engine) -> i64 {
    e.with_conn_for_tests(|c| Ok(c.query_row("SELECT count(*) FROM information_schema.tables WHERE table_name LIKE '\\_\\_disp\\_%' ESCAPE '\\' AND table_name <> '__disp_index'", [], |r| r.get(0))?)).unwrap()
}

// ---------- crs_display_info ----------

#[test]
fn display_info_grids() {
    let e = Engine::new().unwrap();
    let m = e.crs_display_info("EPSG:3857", None).unwrap();
    assert_eq!(m.crs_key, "3857");
    assert!((m.grid.resolutions[0] - engine::crs::pixel_size(0)).abs() < 1e-9);

    let u = e.crs_display_info("EPSG:26915", None).unwrap();
    assert_eq!(u.crs_key.len(), 12);
    assert_eq!(u.crs_key, e.crs_display_info("EPSG:26915", None).unwrap().crs_key, "key is stable");
    assert_ne!(u.crs_key, e.crs_display_info("EPSG:26914", None).unwrap().crs_key);
    assert!(u.proj4.contains("+proj=utm") && u.proj4.contains("+zone=15"), "{}", u.proj4);
    assert!(!u.geographic && u.area_of_use_known && (u.unit_m - 1.0).abs() < 1e-12);
    let g = &u.grid;
    assert!((g.extent[2] - g.extent[0] - (g.extent[3] - g.extent[1])).abs() < 1e-6, "square");
    assert_eq!(g.origin, [g.extent[0], g.extent[3]]);
    assert!((g.resolutions[3] * 2.0 - g.resolutions[2]).abs() < 1e-12);
    assert!((g.resolutions[0] * 256.0 - (g.extent[2] - g.extent[0])).abs() < 1e-6);
    // The central meridian at mid-latitude is inside the grid.
    let (x, y) = e.transform_point(-93.0, 40.0, "EPSG:4326", "EPSG:26915").unwrap();
    assert!(x > g.extent[0] && x < g.extent[2] && y > g.extent[1] && y < g.extent[3]);

    let geo = e.crs_display_info("EPSG:4326", None).unwrap();
    assert!(geo.geographic && geo.grid.extent == [-180.0, -90.0, 180.0, 90.0]);
    assert!((geo.grid.resolutions[0] - 180.0 / 256.0).abs() < 1e-12, "two tiles across at z0");

    let polar = e.crs_display_info("EPSG:3031", None).unwrap();
    let c = polar.grid.extent;
    assert!((c[0] + c[2]).abs() < 1.0 && (c[1] + c[3]).abs() < 1.0, "grid is centred on the pole: {c:?}");

    assert!(e.crs_display_info("EPSG:99999999", None).is_err());
}

// ---------- P1: Web Mercator tiles are unchanged ----------

/// The tile builder as it was before M1 (copied), to prove the refactor changed no bytes.
fn legacy_tile(e: &Engine, layer: &str, z: u32, x: u32, y: u32, fields: &[&str]) -> Vec<u8> {
    use engine::db::{ident, lit};
    let info = e.layer(layer).unwrap();
    e.with_conn_for_tests(|conn| {
        let t = ident(&engine::layers::tile_source(&info, z));
        let env = format!("ST_TileEnvelope({z}, {x}, {y})");
        let px = engine::crs::pixel_size(z);
        let d = info.def_and();
        let count: i64 = conn.query_row(&format!("SELECT count(*) FROM {t} WHERE ST_Intersects(__g, {env}){d}"), [], |r| r.get(0))?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let k = (count as f64 / 25_000.0).sqrt().max(1.0);
        let (small, cell) = (px * k, px * 2.0 * k);
        let mut props = vec!["'__fid': __fid".to_string()];
        let mut cols = vec!["__fid".to_string()];
        for f in fields {
            if info.fields.iter().any(|x| x.name == *f) {
                props.push(format!("{}: {}", lit(f), ident(f)));
                cols.push(ident(f));
            }
        }
        let cols = cols.join(", ");
        let simplify = if info.geom_type == "Point" { "__g".to_string() } else { format!("CASE WHEN __sz > {} THEN ST_Simplify(__g, {}) ELSE __g END", px * 4.0, px / 2.0) };
        let sql = format!(
            "WITH cand AS (SELECT * FROM {t} WHERE ST_Intersects(__g, {env}){d}),
             big AS (SELECT {cols}, {simplify} AS g FROM cand WHERE __sz >= {small}),
             sm AS (SELECT {any_cols}, any_value(__g) AS g FROM cand WHERE __sz < {small}
                    GROUP BY floor(__x0 / {cell}), floor(__y0 / {cell})),
             feats AS (SELECT * FROM big UNION ALL SELECT * FROM sm)
             SELECT ST_AsMVT({{'geom': ST_AsMVTGeom(g, ST_Extent({env}), 4096, 64, true), {props}}}, 'layer', 4096, 'geom', '__fid')
             FROM feats WHERE g IS NOT NULL AND NOT ST_IsEmpty(g)",
            any_cols = cols.split(", ").map(|c| format!("any_value({c}) AS {c}")).collect::<Vec<_>>().join(", "),
            props = props.join(", "),
        );
        let blob: Option<Vec<u8>> = conn.query_row(&sql, [], |r| r.get(0))?;
        Ok(blob.unwrap_or_default())
    })
    .unwrap()
}

#[test]
fn p1_web_mercator_tiles_are_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = import_one(
        &e,
        dir.path(),
        "mix",
        "SELECT i AS id, 'c' || (i % 3) AS cls, ST_Point(-100 + i * 0.7, 30 + i * 0.4) AS geom FROM range(40) t(i)
         UNION ALL SELECT 100, 'poly', ST_GeomFromText('POLYGON((-100 30, -90 30, -90 40, -100 40, -100 30))')
         UNION ALL SELECT 101, 'line', ST_GeomFromText('LINESTRING(-110 25, -80 45)')",
        "EPSG:4326",
    );
    let _ = e.crs_display_info("EPSG:3857", None).unwrap();
    let fields = vec!["cls".to_string()];
    let mut checked = 0;
    for z in 0..=7u32 {
        let n = 1u32 << z;
        for x in 0..n.min(8) {
            for y in 0..n.min(8) {
                let want = legacy_tile(&e, &id, z, x, y, &["cls"]);
                let got = e.tile(&id, z, x, y, &fields).unwrap();
                assert_eq!(feature_set(&got), feature_set(&want), "tile {z}/{x}/{y}");
                assert_eq!(got.len(), want.len(), "tile {z}/{x}/{y} size");
                assert_eq!(*e.tile_in_crs(&id, "3857", z, x, y, &fields).unwrap(), *got, "3857 key {z}/{x}/{y}");
                checked += !want.is_empty() as u32;
            }
        }
    }
    assert!(checked > 5, "the fixture must produce non-empty tiles ({checked})");
}

// ---------- P2: UTM 15N ----------

#[test]
fn p2_utm_tile_places_the_point_within_a_pixel() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let native = import_one(&e, dir.path(), "utm", "SELECT 1 AS id, ST_Point(500000.0, 4500000.0) AS geom", "EPSG:26915");
    let lonlat = import_one(&e, dir.path(), "ll", "SELECT 1 AS id, ST_Point(-93.0, 40.0) AS geom", "EPSG:4326");
    let di = e.crs_display_info("EPSG:26915", None).unwrap();
    let (lx, ly) = e.transform_point(-93.0, 40.0, "EPSG:4326", "EPSG:26915").unwrap();

    // Not built yet: no tile, and a readable error.
    assert!(!e.display_status(&native, "EPSG:26915").unwrap().ready);
    assert!(e.tile_in_crs(&native, &di.crs_key, 6, 0, 0, &[]).is_err());

    for (id, px, py) in [(&native, 500000.0, 4500000.0), (&lonlat, lx, ly)] {
        let st = e.build_display_blocking(id, "EPSG:26915").unwrap();
        assert!(st.ready && !st.building && st.dropped == 0 && st.error.is_none(), "{st:?}");
        for z in [3u32, 6, 10, 14] {
            let (x, y, ex, ey) = locate(&di, z, px, py);
            let tile = e.tile_in_crs(id, &di.crs_key, z, x, y, &[]).unwrap();
            let feats = decode(&tile);
            assert_eq!(feats.len(), 1, "z{z} tile {x}/{y}");
            let (gx, gy) = feats[0][0][0];
            assert!(((gx as f64) - ex).abs() <= 16.0 && ((gy as f64) - ey).abs() <= 16.0, "z{z}: got ({gx},{gy}) expected ({ex:.1},{ey:.1})");
        }
    }
}

// ---------- P3: the pole in EPSG:3031 ----------

#[test]
fn p3_polar_tile_has_a_finite_polygon_around_the_pole() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // An Antarctica-like polygon: a coast at 70°S every 10° and an edge along the pole.
    let coast: Vec<String> = (0..=36).rev().map(|i| format!("{} -70", -180 + i * 10)).collect();
    let wkt = format!("POLYGON((-180 -90, 180 -90, {}, -180 -90))", coast.join(", "));
    let id = import_one(&e, dir.path(), "ant", &format!("SELECT 1 AS id, ST_GeomFromText('{wkt}') AS geom"), "EPSG:4326");
    let di = e.crs_display_info("EPSG:3031", None).unwrap();
    let st = e.build_display_blocking(&id, "EPSG:3031").unwrap();
    assert!(st.ready && st.dropped == 0, "{st:?}");

    let table = display::table_name(&id, &di.crs_key);
    let bad: i64 = e
        .with_conn_for_tests(|c| {
            Ok(c.query_row(
                &format!("SELECT count(*) FROM \"{table}\" WHERE NOT (isfinite(__x0) AND isfinite(__y0) AND isfinite(__sz) AND isfinite(ST_XMax(__d)) AND isfinite(ST_YMax(__d)))"),
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(bad, 0, "non-finite display geometry");

    // z0 (the whole grid) and the four z1 tiles that meet at the pole.
    for (z, x, y) in [(0u32, 0u32, 0u32), (1, 0, 0), (1, 1, 0), (1, 0, 1), (1, 1, 1)] {
        let feats = decode(&e.tile_in_crs(&id, &di.crs_key, z, x, y, &[]).unwrap());
        assert_eq!(feats.len(), 1, "tile {z}/{x}/{y} has the polygon");
        let pts: Vec<(i64, i64)> = feats[0].iter().flatten().copied().collect();
        assert!(pts.len() >= 4, "tile {z}/{x}/{y}: {pts:?}");
        assert!(pts.iter().all(|&(px, py)| (-64..=4160).contains(&px) && (-64..=4160).contains(&py)), "tile {z}/{x}/{y} coordinates leave the tile+buffer");
    }
    // At z0 the polygon is the disc: vertices on every side of the centre.
    let pts: Vec<(i64, i64)> = decode(&e.tile_in_crs(&id, &di.crs_key, 0, 0, 0, &[]).unwrap())[0].iter().flatten().copied().collect();
    let (min, max) = (pts.iter().map(|p| p.0).min().unwrap(), pts.iter().map(|p| p.0).max().unwrap());
    assert!(min < 2048 - 500 && max > 2048 + 500, "disc spans the tile: x {min}..{max}");
}

// ---------- P4: features outside the area of use ----------

#[test]
fn p4_world_layer_in_utm_reports_dropped_features() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = import_one(
        &e,
        dir.path(),
        "world",
        "SELECT 1 AS id, ST_Point(-93.0, 40.0) AS geom
         UNION ALL SELECT 2, ST_Point(10.0, 50.0)
         UNION ALL SELECT 3, ST_Point(-120.0, -30.0)
         UNION ALL SELECT 4, ST_GeomFromText('POLYGON((-120 30, -60 30, -60 50, -120 50, -120 30))')
         UNION ALL SELECT 5, ST_GeomFromText('POLYGON((100 30, 120 30, 120 50, 100 50, 100 30))')",
        "EPSG:4326",
    );
    let di = e.crs_display_info("EPSG:26915", None).unwrap();
    let st = e.build_display_blocking(&id, "EPSG:26915").unwrap();
    assert!(st.ready);
    assert_eq!(st.dropped, 3, "two points and one polygon lie outside zone 15");
    assert_eq!(e.display_status(&id, "EPSG:26915").unwrap().dropped, 3);

    // Only the point and the clipped polygon are drawn, and the polygon stays inside the zone.
    let feats = decode(&e.tile_in_crs(&id, &di.crs_key, 0, 0, 0, &[]).unwrap());
    assert_eq!(feats.len(), 2);
    let table = display::table_name(&id, &di.crs_key);
    let (x0, x1): (f64, f64) = e
        .with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT min(__x0), max(ST_XMax(__d)) FROM \"{table}\" WHERE __sz > 1000"), [], |r| Ok((r.get(0)?, r.get(1)?)))?))
        .unwrap();
    assert!(x0 > 100_000.0 && x1 < 900_000.0, "clipped to the 96°W–90°W zone: {x0}..{x1}");

    // Web Mercator and a CRS the data fits in drop nothing.
    assert_eq!(e.build_display_blocking(&id, "EPSG:3857").unwrap().dropped, 0);
}

// ---------- P5: edits drop display tables and cancel builds ----------

#[test]
fn p5_editing_a_layer_drops_its_display_tables() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = import_one(&e, dir.path(), "pts", "SELECT i AS id, ST_Point(-95 + i * 0.01, 40) AS geom FROM range(100) t(i)", "EPSG:4326");
    e.crs_display_info("EPSG:26915", None).unwrap();
    assert!(e.build_display_blocking(&id, "EPSG:26915").unwrap().ready);
    assert_eq!(disp_tables(&e), 1);

    e.set_definition_query(&id, Some("id < 50")).unwrap();
    assert!(!e.display_status(&id, "EPSG:26915").unwrap().ready, "stale table is not served");
    assert_eq!(disp_tables(&e), 0, "stale table dropped");
    let rows: i64 = e.with_conn_for_tests(|c| Ok(c.query_row("SELECT count(*) FROM __disp_index", [], |r| r.get(0))?)).unwrap();
    assert_eq!(rows, 0);

    // Rebuild, then remove the layer.
    assert!(e.build_display_blocking(&id, "EPSG:26915").unwrap().ready);
    e.remove_layer(&id).unwrap();
    assert_eq!(disp_tables(&e), 0);
}

#[test]
fn p5_an_edit_cancels_a_running_build() {
    let dir = tempfile::tempdir().unwrap();
    let e = Arc::new(Engine::new().unwrap());
    let id = import_one(&e, dir.path(), "many", "SELECT i AS id, ST_Point(-97 + (i % 1000) * 0.005, 30 + (i // 1000) * 0.005) AS geom FROM range(400000) t(i)", "EPSG:4326");
    e.crs_display_info("EPSG:26915", None).unwrap();
    let st = e.build_display(&id, "EPSG:26915").unwrap();
    assert!(st.building || st.ready, "{st:?}");
    e.set_definition_query(&id, Some("id >= 0")).unwrap();

    let t0 = Instant::now();
    while e.display_status(&id, "EPSG:26915").unwrap().building {
        assert!(t0.elapsed() < Duration::from_secs(60), "build did not stop");
        std::thread::sleep(Duration::from_millis(50));
    }
    let st = e.display_status(&id, "EPSG:26915").unwrap();
    assert!(!st.ready && st.error.is_none(), "{st:?}");
    assert_eq!(disp_tables(&e), 0, "no leftovers from the cancelled build");
    // The new version builds normally.
    assert!(e.build_display_blocking(&id, "EPSG:26915").unwrap().ready);
}

// ---------- P6: performance (release, ignored) ----------

#[test]
#[ignore]
fn p6_one_million_points() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = import_one(&e, dir.path(), "million", "SELECT i AS id, ST_Point(-125 + random() * 58, 25 + random() * 24) AS geom FROM range(1000000) t(i)", "EPSG:4326");
    let di = e.crs_display_info("EPSG:5070", None).unwrap();
    let t = Instant::now();
    let st = e.build_display_blocking(&id, "EPSG:5070").unwrap();
    let build = t.elapsed();
    println!("1M points: display table in {build:.2?} (dropped {})", st.dropped);
    assert!(build < Duration::from_secs(10), "{build:?}");
    let mut worst = Duration::ZERO;
    for z in [2u32, 4, 6, 8] {
        let (x, y, _, _) = locate(&di, z, 0.0, 0.0);
        let t = Instant::now();
        let tile = e.tile_in_crs(&id, &di.crs_key, z, x, y, &[]).unwrap();
        let d = t.elapsed();
        println!("z{z} tile {x}/{y}: {} bytes in {d:.2?}", tile.len());
        worst = worst.max(d);
    }
    assert!(worst < Duration::from_millis(300), "{worst:?}");
}
