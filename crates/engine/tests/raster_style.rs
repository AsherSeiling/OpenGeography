//! Raster symbology on a synthetic ramp raster (docs/TEST_PLAN.md RS1).

use engine::raster::{self, RasterInfo};
use engine::raster_style::{self, ColorMode, RasterStyle, Renderer, Stretch, StretchMode};
use std::path::Path;

/// 20×20 UTM 14N raster, value = column (0..19) × 10, band 2 = 255 - band 1; one NoData cell.
fn make(path: &Path) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, 20, 20, 2).unwrap();
    ds.set_geo_transform(&[600_000.0, 30.0, 0.0, 3_900_000.0, 0.0, -30.0]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614).unwrap()).unwrap();
    for b in 1..=2 {
        let mut band = ds.rasterband(b).unwrap();
        band.set_no_data_value(Some(-9999.0)).unwrap();
        let data: Vec<f32> = (0..400).map(|i| if i == 0 { -9999.0 } else if b == 1 { (i % 20) as f32 * 10.0 } else { 255.0 - (i % 20) as f32 * 10.0 }).collect();
        band.write((0, 0), (20, 20), &mut gdal::raster::Buffer::new((20, 20), data)).unwrap();
    }
}

fn decode(png: &[u8]) -> (Vec<u8>, u32, u32) {
    let dec = png::Decoder::new(std::io::Cursor::new(png));
    let mut r = dec.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let i = r.next_frame(&mut buf).unwrap();
    (buf, i.width, i.height)
}

fn setup() -> (tempfile::TempDir, RasterInfo) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("r.tif");
    make(&p);
    let info = raster::describe(&p).unwrap();
    (dir, info)
}

const EXTENT: [f64; 4] = [600_000.0, 3_899_400.0, 600_600.0, 3_900_000.0];

fn render(info: &RasterInfo, s: &RasterStyle) -> Vec<u8> {
    decode(&raster::render_extent_styled(info, s, "EPSG:32614", EXTENT, 20, 20).unwrap()).0
}

#[test]
fn gray_and_pseudocolor_differ_from_default_and_each_other() {
    let (_d, info) = setup();
    let def = render(&info, &RasterStyle::default());
    let gray = render(&info, &RasterStyle { renderer: Renderer::Gray, ..Default::default() });
    let pc = render(&info, &RasterStyle { renderer: Renderer::Pseudocolor, ..Default::default() });
    assert_ne!(def, gray);
    assert_ne!(gray, pc);
    // Gray: left column is darkest valid, right column brightest; R = G = B.
    let px = |b: &Vec<u8>, c: usize, r: usize| [b[(r * 20 + c) * 4], b[(r * 20 + c) * 4 + 1], b[(r * 20 + c) * 4 + 2], b[(r * 20 + c) * 4 + 3]];
    assert_eq!(px(&gray, 0, 5), [0, 0, 0, 255]);
    assert_eq!(px(&gray, 19, 5), [255, 255, 255, 255]);
    assert_eq!(px(&gray, 0, 0)[3], 0, "NoData cell is transparent");
}

#[test]
fn stretch_modes_set_the_range() {
    let (_d, info) = setup();
    let mut s = RasterStyle { renderer: Renderer::Gray, ..Default::default() };
    assert_eq!(raster_style::stretch_range(&info, &s).unwrap(), (0.0, 190.0));
    s.stretch = Stretch { mode: StretchMode::Percentile, pct_low: 10.0, pct_high: 90.0, ..Default::default() };
    let (lo, hi) = raster_style::stretch_range(&info, &s).unwrap();
    assert!(lo > 10.0 && lo < 40.0 && hi < 180.0 && hi > 150.0, "{lo} {hi}");
    s.stretch = Stretch { mode: StretchMode::StdDev, sigma: 1.0, ..Default::default() };
    let (lo, hi) = raster_style::stretch_range(&info, &s).unwrap();
    assert!(lo > 0.0 && hi < 190.0 && lo < hi);
    s.stretch = Stretch { mode: StretchMode::Manual, min: 50.0, max: 100.0, ..Default::default() };
    assert_eq!(raster_style::stretch_range(&info, &s).unwrap(), (50.0, 100.0));
    let b = render(&info, &s);
    assert_eq!(b[(5 * 20) * 4], 0, "below the manual minimum clips to black");
    assert_eq!(b[(5 * 20 + 19) * 4], 255);
}

#[test]
fn band_choice_and_errors() {
    let (_d, info) = setup();
    let b1 = render(&info, &RasterStyle { renderer: Renderer::Gray, band: 1, ..Default::default() });
    let b2 = render(&info, &RasterStyle { renderer: Renderer::Gray, band: 2, ..Default::default() });
    assert_eq!(b1[(5 * 20) * 4], 0);
    assert_eq!(b2[(5 * 20) * 4], 255, "band 2 is the reverse of band 1");
    let err = raster::render_extent_styled(&info, &RasterStyle { renderer: Renderer::Gray, band: 3, ..Default::default() }, "EPSG:32614", EXTENT, 20, 20).unwrap_err();
    assert!(err.to_string().contains("Band 3"), "{err}");
}

#[test]
fn discrete_mode_and_default_unchanged() {
    let (_d, info) = setup();
    let s = RasterStyle { renderer: Renderer::Pseudocolor, ramp: "Set1".into(), mode: ColorMode::Discrete, ..Default::default() };
    let b = render(&info, &s);
    let mut colours = std::collections::HashSet::new();
    for c in 0..20 {
        colours.insert([b[(5 * 20 + c) * 4], b[(5 * 20 + c) * 4 + 1], b[(5 * 20 + c) * 4 + 2]]);
    }
    assert!(colours.len() <= 9 && colours.len() >= 5, "{}", colours.len());
    // The default style is byte-identical to the unstyled renderer.
    let a = raster::render_extent(&info, "EPSG:32614", EXTENT, 20, 20).unwrap();
    let b = raster::render_extent_styled(&info, &RasterStyle::default(), "EPSG:32614", EXTENT, 20, 20).unwrap();
    assert_eq!(a, b);
}

#[test]
fn histogram_stats() {
    let (_d, info) = setup();
    let s = raster_style::band_stats(&info, 1).unwrap();
    assert_eq!((s.min, s.max), (0.0, 190.0));
    let h = s.histogram(10);
    assert_eq!(h.iter().sum::<u64>(), 399);
}

#[test]
fn unique_values_and_paletted_rendering() {
    let (_d, info) = setup();
    let u = raster_style::unique_values(&info, 1, 100).unwrap();
    assert_eq!(u.values.len(), 20, "columns 0..19 x 10; the NoData cell is not counted");
    assert!(!u.sampled && !u.truncated);
    assert_eq!(u.values[0].value, 0.0);
    assert_eq!(u.values[0].count, 19, "column 0 lost its NoData cell");
    assert_eq!(u.values[1].count, 20);
    assert!(u.values[0].color.is_none());
    assert!(raster_style::unique_values(&info, 1, 5).unwrap().truncated);

    let s = RasterStyle {
        renderer: Renderer::Paletted,
        entries: vec![raster_style::PalEntry { value: 50.0, color: "#ff0000".into(), label: "fifty".into(), hidden: false }],
        ..Default::default()
    };
    let b = render(&info, &s);
    let at = |c: usize| &b[(5 * 20 + c) * 4..(5 * 20 + c) * 4 + 4];
    assert_eq!(at(5), &[255, 0, 0, 255]);
    assert_eq!(at(6)[3], 0, "other values transparent");
    assert_eq!(at(4)[3], 0);
}

#[test]
fn multiband_composite_uses_chosen_bands() {
    let (_d, info) = setup();
    // Bands 1 and 2 are opposites: R = band 2, G = band 1, B = band 1.
    let s = RasterStyle { renderer: Renderer::Multiband, rgb_bands: [2, 1, 1], ..Default::default() };
    let b = render(&info, &s);
    let px = &b[(5 * 20) * 4..(5 * 20) * 4 + 4];
    assert_eq!(px[3], 255);
    assert!(px[0] > 200 && px[1] < 5, "column 0: band 2 high, band 1 low: {px:?}");
    let px = &b[(5 * 20 + 19) * 4..(5 * 20 + 19) * 4 + 4];
    assert!(px[0] < 5 && px[1] > 250);
    let err = raster::render_extent_styled(&info, &RasterStyle { renderer: Renderer::Multiband, rgb_bands: [1, 2, 3], ..Default::default() }, "EPSG:32614", EXTENT, 20, 20).unwrap_err();
    assert!(err.to_string().contains("Band 3"), "{err}");
}

#[test]
fn shading_and_resampling_change_the_picture() {
    let (_d, info) = setup();
    let plain = RasterStyle { renderer: Renderer::Pseudocolor, ..Default::default() };
    let shaded = RasterStyle { shading: Some(Default::default()), ..plain.clone() };
    let (a, b) = (render(&info, &plain), render(&info, &shaded));
    assert_ne!(a, b);
    // Alpha is untouched by shading.
    assert!((0..400).all(|i| a[i * 4 + 3] == b[i * 4 + 3]));
    let near = render(&info, &RasterStyle { resampling: "nearest".into(), ..plain.clone() });
    let png = raster::render_extent_styled(&info, &RasterStyle { resampling: "cubic".into(), ..plain }, "EPSG:32614", [600_000.0, 3_899_400.0, 600_300.0, 3_899_700.0], 40, 40).unwrap();
    assert!(!png.is_empty());
    assert_eq!(near.len(), a.len());
}
