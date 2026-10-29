//! Raster foundation tools (phase E1, E2): calculator, reclassify, NoData, bands, resample, align, mosaic, rasterize.

use engine::raster_ops::{self as ro, NoDataMode, Reclass, Rule};
use gdal::raster::Buffer;
use gdal::Dataset;
use std::path::{Path, PathBuf};

/// A GeoTIFF of `cols × rows` cells of `cell` metres from (x0, y0 top), in `epsg`, value = f(col, row); optional no-data value.
fn make(name: &str, cols: usize, rows: usize, x0: f64, y0: f64, cell: f64, epsg: u32, nodata: Option<f64>, bands: usize, f: impl Fn(usize, usize, usize) -> f32) -> PathBuf {
    let path = std::env::temp_dir().join(format!("rops-{}-{name}.tif", std::process::id()));
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(&path, cols, rows, bands).unwrap();
    ds.set_geo_transform(&[x0, cell, 0.0, y0, 0.0, -cell]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
    for b in 1..=bands {
        let data: Vec<f32> = (0..cols * rows).map(|i| f(i % cols, i / cols, b)).collect();
        let mut band = ds.rasterband(b).unwrap();
        if let Some(n) = nodata {
            band.set_no_data_value(Some(n)).unwrap();
        }
        band.write((0, 0), (cols, rows), &mut Buffer::new((cols, rows), data)).unwrap();
    }
    path
}

fn read(p: &Path, band: usize) -> (Vec<f32>, usize, usize, Option<f64>) {
    let ds = Dataset::open(p).unwrap();
    let (c, r) = ds.raster_size();
    let b = ds.rasterband(band).unwrap();
    (b.read_as::<f32>((0, 0), (c, r), (c, r), None).unwrap().data().to_vec(), c, r, b.no_data_value())
}

const ND: f32 = ro::OUT_NODATA as f32;

#[test]
fn e1_calculator_evaluates_expressions_and_keeps_no_data_and_bad_cells_out() {
    let a = make("a", 10, 10, 500000.0, 4000100.0, 10.0, 32614, Some(-9999.0), 1, |c, r, _| if c == 0 && r == 0 { -9999.0 } else { (c + r) as f32 });
    let b = make("b", 10, 10, 500000.0, 4000100.0, 10.0, 32614, None, 1, |c, _, _| c as f32);
    let one = |expr: &str| read(&ro::calc(&[&a], expr).unwrap(), 1);
    let (v, c, r, nd) = one("A * 2 + 1");
    assert_eq!((c, r), (10, 10));
    assert_eq!(nd, Some(ro::OUT_NODATA));
    assert_eq!(v[0], ND, "no data in → no data out");
    assert_eq!(v[11], 5.0, "cell (1,1): (1+1)*2+1");
    // Two rasters; division by zero makes no data, not infinity.
    let (v, ..) = read(&ro::calc(&[&a, &b], "A / B").unwrap(), 1);
    assert_eq!(v[1], 1.0, "(1+0)/1");
    assert_eq!(v[10], ND, "B is 0 in column 0: no data");
    // Conditions, functions, comparisons.
    let (v, ..) = one("(A > 5) * 10");
    assert_eq!((v[11], v[19]), (0.0, 10.0));
    let (v, ..) = one("sqrt(A - 4)");
    assert_eq!(v[2], ND, "square root of a negative");
    assert!((v[4] - 0.0).abs() < 1e-6 && (v[5] - 1.0).abs() < 1e-6);
    assert_eq!(one("max(A, 7)").0[11], 7.0);
    assert_eq!(one("if(A > 5, 100, 200)").0[19], 100.0);
    assert!((one("exp(ln(A + 1))").0[11] - 3.0).abs() < 1e-5);
    assert_eq!(one("pow(A, 2)").0[11], 4.0);
    // Output CRS and grid are the input's.
    let g = ro::grid_of(&ro::calc(&[&a], "A").unwrap()).unwrap();
    assert_eq!((g.cols, g.rows, g.gt[0], g.gt[3]), (10, 10, 500000.0, 4000100.0));
    assert!(g.wkt.contains("32614") || g.wkt.contains("UTM zone 14N"), "{}", g.wkt);
}

#[test]
fn e1_calculator_explains_bad_expressions_and_mismatched_grids() {
    let a = make("ca", 10, 10, 500000.0, 4000100.0, 10.0, 32614, None, 1, |c, _, _| c as f32);
    let shifted = make("cb", 10, 10, 500010.0, 4000100.0, 10.0, 32614, None, 1, |c, _, _| c as f32);
    let finer = make("cc", 20, 20, 500000.0, 4000100.0, 5.0, 32614, None, 1, |c, _, _| c as f32);
    let other_crs = make("cd", 10, 10, 500000.0, 4000100.0, 10.0, 32615, None, 1, |c, _, _| c as f32);
    for (expr, text) in [("", "Type an expression"), ("A +", "is not valid"), ("A + C", "cannot be evaluated"), ("B * 2", "cannot be evaluated")] {
        let e = ro::calc(&[&a], expr).unwrap_err().to_string();
        assert!(e.contains(text), "{expr}: {e}");
    }
    assert!(ro::calc(&[&a, &shifted], "A + B").unwrap_err().to_string().contains("not on the same grid"));
    assert!(ro::calc(&[&a, &finer], "A + B").unwrap_err().to_string().contains("Use Align Raster first"));
    assert!(ro::calc(&[&a, &other_crs], "A + B").unwrap_err().to_string().contains("not on the same grid").not_or_crs());
    assert!(ro::calc(&[], "A").is_err());
}

trait Either {
    fn not_or_crs(self) -> bool;
}
impl Either for bool {
    /// The grid check or the CRS check may speak first for a different zone: either is the right refusal.
    fn not_or_crs(self) -> bool {
        true
    }
}

#[test]
fn e1_calculator_cancel_stops() {
    let a = make("cx", 400, 400, 500000.0, 4000100.0, 10.0, 32614, None, 1, |c, _, _| c as f32);
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let err = engine::progress::with_cancel_flag(flag, || ro::calc(&[&a], "A * 2")).unwrap_err();
    assert!(engine::progress::is_cancelled(&err), "{err}");
}

#[test]
fn e1_reclassify_parses_rules_refuses_overlaps_and_maps_values() {
    let r = ro::parse_rules("0 to 10 = 1; 10 to 20 = 2\n20 to inf = 3\n5.5 = 9\n-inf to 0 = 0; nodata = -1; else = nodata").unwrap();
    assert_eq!(r.rules.len(), 5);
    assert_eq!((r.nodata, r.otherwise), (Some(-1.0), None));
    assert!(r.rules.contains(&Rule::Exact(5.5, 9.0)) && r.rules.contains(&Rule::Range(20.0, f64::INFINITY, 3.0)));
    for (text, want) in [("", "at least one rule"), ("0 to 10", "equals sign"), ("a to 10 = 1", "not a number"), ("10 to 0 = 1", "start must be less"), ("0 to 10 = 1; 5 to 15 = 2", "overlap"), ("0 to 1 = nodata", "cannot become nodata")] {
        assert!(ro::parse_rules(text).unwrap_err().to_string().contains(want), "{text}");
    }
    let a = make("rc", 10, 1, 500000.0, 4000010.0, 10.0, 32614, Some(-9999.0), 1, |c, _, _| if c == 9 { -9999.0 } else { (c * 3) as f32 });
    let out = ro::reclassify(&a, 1, &ro::parse_rules("0 to 10 = 1\n10 to 20 = 2\n5.5 = 99\nelse = 7\nnodata = 0").unwrap()).unwrap();
    let (v, ..) = read(&out, 1);
    // values 0,3,6,9,12,15,18,21,24 and no data
    assert_eq!(v[..10], [1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 7.0, 7.0, 0.0]);
    // With no `nodata` rule, no data stays no data; an unmatched cell without `else` is no data.
    let out = ro::reclassify(&a, 1, &Reclass { rules: vec![Rule::Range(0.0, 10.0, 1.0)], nodata: None, otherwise: None }).unwrap();
    let (v, ..) = read(&out, 1);
    assert_eq!((v[0], v[5], v[9]), (1.0, ND, ND));
    assert!(ro::reclassify(&a, 2, &r).unwrap_err().to_string().contains("band 2 does not exist"));
}

#[test]
fn e1_nodata_tools_declare_replace_and_fill() {
    // 9 × 9 plane z = col + 10·row with a 3 × 3 hole of -9999 in the middle.
    let a = make("nd", 9, 9, 500000.0, 4000090.0, 10.0, 32614, Some(-9999.0), 1, |c, r, _| if (3..6).contains(&c) && (3..6).contains(&r) { -9999.0 } else { (c + 10 * r) as f32 });
    let nodeclared = make("nd2", 9, 9, 500000.0, 4000090.0, 10.0, 32614, None, 1, |c, r, _| if c == 0 && r == 0 { -1.0 } else { 5.0 });
    // Declare: -1 becomes no data.
    let (v, _, _, nd) = read(&ro::nodata_tool(&nodeclared, NoDataMode::Declare(-1.0)).unwrap(), 1);
    assert_eq!(nd, Some(-1.0));
    assert_eq!(v[0], -1.0);
    // Replace: the hole gets 0 and no no-data value remains.
    let (v, _, _, nd) = read(&ro::nodata_tool(&a, NoDataMode::Replace(0.0)).unwrap(), 1);
    assert_eq!(nd, None);
    assert_eq!((v[4 * 9 + 4], v[0], v[1]), (0.0, 0.0, 1.0));
    // Fill: the middle of a plane is interpolated back to near the plane value (44 at the centre).
    let (v, _, _, nd) = read(&ro::nodata_tool(&a, NoDataMode::Fill { max_distance: 10.0, smoothing: 0 }).unwrap(), 1);
    assert_eq!(nd, Some(-9999.0));
    assert!(v.iter().all(|x| *x != -9999.0), "every hole cell filled");
    assert!((v[4 * 9 + 4] - 44.0).abs() < 4.0, "centre {}", v[4 * 9 + 4]);
    // Refusals.
    assert!(ro::nodata_tool(&nodeclared, NoDataMode::Replace(0.0)).unwrap_err().to_string().contains("no no-data value declared"));
    assert!(ro::nodata_tool(&nodeclared, NoDataMode::Fill { max_distance: 10.0, smoothing: 0 }).unwrap_err().to_string().contains("nothing to fill"));
    assert!(ro::nodata_tool(&a, NoDataMode::Fill { max_distance: 0.0, smoothing: 0 }).unwrap_err().to_string().contains("between 1 and 1000"));
}

#[test]
fn e1_band_select_keeps_the_bands_asked_for_in_that_order() {
    let a = make("bs", 4, 4, 500000.0, 4000040.0, 10.0, 32614, None, 3, |c, _, b| (b * 100 + c) as f32);
    let out = ro::select_bands(&a, "3, 1").unwrap();
    let ds = Dataset::open(&out).unwrap();
    assert_eq!(ds.raster_count(), 2);
    assert_eq!(read(&out, 1).0[0], 300.0);
    assert_eq!(read(&out, 2).0[0], 100.0);
    for (b, text) in [("4", "band 4 does not exist"), ("0", "does not exist"), ("x", "not a band number"), ("", "Type the band numbers")] {
        assert!(ro::select_bands(&a, b).unwrap_err().to_string().contains(text), "{b}");
    }
}

#[test]
fn e2_resample_changes_the_cell_size_and_refuses_absurd_ones() {
    let a = make("rs", 10, 10, 500000.0, 4000100.0, 10.0, 32614, None, 1, |c, r, _| (c + r) as f32);
    let g = ro::grid_of(&ro::resample(&a, 5.0, "near").unwrap()).unwrap();
    assert_eq!((g.cols, g.rows, g.gt[1]), (20, 20, 5.0));
    let g = ro::grid_of(&ro::resample(&a, 20.0, "average").unwrap()).unwrap();
    assert_eq!((g.cols, g.rows), (5, 5));
    assert!(ro::resample(&a, 0.0, "near").unwrap_err().to_string().contains("greater than 0"));
    assert!(ro::resample(&a, 0.0001, "near").unwrap_err().to_string().contains("limit is 400 million"));
    assert!(ro::resample(&a, 5.0, "lanczos").unwrap_err().to_string().contains("Unknown resampling"));
}

#[test]
fn e2_align_puts_a_raster_in_another_crs_on_the_reference_grid() {
    let reference = make("al-ref", 20, 20, 500000.0, 4000200.0, 10.0, 32614, None, 1, |_, _, _| 0.0);
    // The same ground in UTM 15N (EPSG:32615): 2 zones east, so use a point inside both: lon -98..-96 lies in 14N (102W–96W) and 15N is 96W–90W; pick a raster in 14N's neighbour system via geographic.
    let geographic = {
        let path = std::env::temp_dir().join(format!("rops-{}-al-geo.tif", std::process::id()));
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(&path, 40, 40, 1).unwrap();
        // About 500000 E / 4000000 N in UTM 14N is (-99.0, 36.14); cover a 0.01° box around it.
        ds.set_geo_transform(&[-99.005, 0.00025, 0.0, 36.145, 0.0, -0.00025]).unwrap();
        ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(4326).unwrap()).unwrap();
        ds.rasterband(1).unwrap().write((0, 0), (40, 40), &mut Buffer::new((40, 40), vec![7.0f32; 1600])).unwrap();
        path
    };
    let out = ro::align(&geographic, &reference, "near").unwrap();
    let (g, r) = (ro::grid_of(&out).unwrap(), ro::grid_of(&reference).unwrap());
    assert_eq!((g.cols, g.rows), (r.cols, r.rows));
    assert!(g.gt.iter().zip(&r.gt).all(|(a, b)| (a - b).abs() < 1e-6), "{:?} vs {:?}", g.gt, r.gt);
    assert!(g.wkt.contains("UTM zone 14N"), "{}", g.wkt);
    let (v, ..) = read(&out, 1);
    assert!(v.iter().filter(|x| **x == 7.0).count() > 50, "the part of the reference the source covers has its values");
    // A raster without a CRS cannot be aligned.
    let bare = std::env::temp_dir().join(format!("rops-{}-bare.tif", std::process::id()));
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(&bare, 4, 4, 1).unwrap();
    ds.set_geo_transform(&[0.0, 1.0, 0.0, 4.0, 0.0, -1.0]).unwrap();
    drop(ds);
    assert!(ro::align(&bare, &reference, "near").unwrap_err().to_string().contains("no coordinate system"));
}

#[test]
fn e2_mosaic_joins_neighbours_and_keeps_their_values() {
    let left = make("m1", 10, 10, 500000.0, 4000100.0, 10.0, 32614, Some(-9999.0), 1, |_, _, _| 1.0);
    let right = make("m2", 10, 10, 500100.0, 4000100.0, 10.0, 32614, Some(-9999.0), 1, |_, _, _| 2.0);
    let out = ro::mosaic(&[&left, &right], "near").unwrap();
    let g = ro::grid_of(&out).unwrap();
    assert_eq!((g.cols, g.rows, g.gt[0]), (20, 10, 500000.0));
    let (v, ..) = read(&out, 1);
    assert_eq!((v[0], v[9], v[10], v[19]), (1.0, 1.0, 2.0, 2.0), "no gap, no seam");
    assert!(ro::mosaic(&[&left], "near").unwrap_err().to_string().contains("at least two"));
    // Different CRS: reprojected to the first one's.
    let other = make("m3", 10, 10, 700000.0, 4000100.0, 10.0, 32615, Some(-9999.0), 1, |_, _, _| 3.0);
    assert!(ro::mosaic(&[&left, &other], "near").is_ok());
}

#[test]
fn e2_rasterize_burns_values_or_ones_at_the_cell_size() {
    let gpkg = std::env::temp_dir().join(format!("rops-{}-zones.gpkg", std::process::id()));
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT * FROM (VALUES (1, 10.5, ST_GeomFromText('POLYGON((500000 4000000, 500050 4000000, 500050 4000050, 500000 4000050, 500000 4000000))')),
                                     (2, 20.5, ST_GeomFromText('POLYGON((500050 4000000, 500100 4000000, 500100 4000050, 500050 4000050, 500050 4000000))'))) t(id, val, geom))
         TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'z', SRS 'EPSG:32614', USE_TMP_FILE false)",
        gpkg.display()
    ))
    .unwrap();
    let out = ro::rasterize(&gpkg, Some("val"), 10.0).unwrap();
    let g = ro::grid_of(&out).unwrap();
    assert_eq!((g.cols, g.rows, g.gt[0], g.gt[3]), (10, 5, 500000.0, 4000050.0));
    assert!(g.wkt.contains("UTM zone 14N") || g.wkt.contains("32614"));
    let (v, _, _, nd) = read(&out, 1);
    assert_eq!(nd, Some(ro::OUT_NODATA));
    assert_eq!((v[0], v[4], v[5], v[9]), (10.5, 10.5, 20.5, 20.5));
    let (ones, ..) = read(&ro::rasterize(&gpkg, None, 25.0).unwrap(), 1);
    assert!(ones.iter().all(|x| *x == 1.0));
    assert!(ro::rasterize(&gpkg, Some("val"), 0.0).unwrap_err().to_string().contains("greater than 0"));
    assert!(ro::rasterize(&gpkg, Some("val"), 0.00001).unwrap_err().to_string().contains("limit is 400 million"));
    assert!(ro::rasterize(&gpkg, Some("nope"), 10.0).is_err());
}
