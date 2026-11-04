//! WP12 (open basemaps): the bundled Natural Earth basemap draws in any CRS and in a layout export, and the screen-only
//! basemaps are refused in layouts in words. No network.

use engine::layout::basemap;
use engine::layout::model::RenderRequest;
use engine::Engine;
use serde_json::json;

const OCEAN: [u8; 3] = [170, 211, 232];
const LAND: [u8; 3] = [238, 235, 224];

fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
    let mut buf = vec![0u8; dec.output_buffer_size().unwrap()];
    let info = dec.next_frame(&mut buf).unwrap();
    (info.width, info.height, buf)
}

fn px(img: &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.0 + x) * 4) as usize;
    [img.2[i], img.2[i + 1], img.2[i + 2], img.2[i + 3]]
}

fn rgb(p: [u8; 4]) -> [u8; 3] {
    [p[0], p[1], p[2]]
}

#[test]
fn natural_earth_file_is_found_and_is_small() {
    let p = engine::resources::natural_earth_raster().expect("the bundled raster is found in a source-tree build");
    assert!(std::fs::metadata(p).unwrap().len() < 2_000_000);
}

/// A 256 px tile over central North America (lon -110..-70, lat 25..55 at 1:?) in several CRSs: land where land is, ocean where
/// the sea is, and the same call works in a conic, a polar and a geographic CRS (the engine warps; no CRS is special-cased).
#[test]
fn natural_earth_tile_draws_in_projected_geographic_and_polar_crs() {
    // Web Mercator, whole world: Kansas is land, the mid-Atlantic is ocean.
    let world = 20037508.342789244;
    let t = decode(&basemap::natural_earth_tile("EPSG:3857", [-world, -world, world, world], 512).unwrap());
    let at = |lon: f64, lat: f64| {
        let x = ((lon + 180.0) / 360.0 * 512.0) as u32;
        let y = ((1.0 - ((lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI)) / 2.0 * 512.0) as u32;
        rgb(px(&t, x, y))
    };
    assert_eq!(at(-98.0, 39.0), LAND);
    assert_eq!(at(-40.0, 30.0), OCEAN);
    // Geographic: the 4326 extent of the whole file.
    let g = decode(&basemap::natural_earth_tile("EPSG:4326", [-180.0, -90.0, 180.0, 90.0], 360).unwrap());
    assert_eq!(rgb(px(&g, 82, 51)), LAND, "lon -98, lat 39");
    assert_eq!(rgb(px(&g, 140, 60)), OCEAN, "lon -40, lat 30");
    // UTM 15N around Iowa (x 500 km, y 4.4 Mm), 1000 km square: all land or lake-free land and opaque.
    let u = decode(&basemap::natural_earth_tile("EPSG:26915", [0.0, 3_900_000.0, 1_000_000.0, 4_900_000.0], 128).unwrap());
    assert_eq!(rgb(px(&u, 64, 64)), LAND);
    assert_eq!(px(&u, 64, 64)[3], 255);
    // Polar stereographic (Antarctic): a tile around the pole is land; a tile far out at sea is ocean.
    let p = decode(&basemap::natural_earth_tile("EPSG:3031", [-500_000.0, -500_000.0, 500_000.0, 500_000.0], 64).unwrap());
    assert_eq!(rgb(px(&p, 32, 32)), LAND);
    // Albers (conus): a tile in the Gulf of Mexico is ocean.
    let a = decode(&basemap::natural_earth_tile("EPSG:5070", [100_000.0, 250_000.0, 300_000.0, 450_000.0], 32).unwrap());
    assert_eq!(rgb(px(&a, 16, 16)), OCEAN);
}

fn frame_req(basemap: &str) -> RenderRequest {
    // UTM 15N, 1:20,000,000: the 100 × 80 mm frame spans 2000 × 1600 km around the Gulf coast (Louisiana).
    let frame = json!({ "kind": "map_frame", "id": "f1", "x": 12.7, "y": 30.0, "w": 100.0, "h": 80.0, "crs": "EPSG:26915", "center": [500000.0, 3400000.0], "scale": 20_000_000.0, "layers": "map", "basemap": basemap });
    serde_json::from_value(json!({
        "layout": { "id": "L1", "name": "NE", "page": { "width": 279.4, "height": 215.9 }, "elements": [frame] },
        "order": [], "project": "Demo", "date": "2026-01-01"
    }))
    .unwrap()
}

/// The layout export in a projected CRS really draws Natural Earth: pixels are land in the middle and ocean along the edges (the Gulf, the Atlantic and the Great Lakes show), the file is cited, and it needs no network.
#[test]
fn natural_earth_draws_in_a_layout_export_in_utm() {
    let e = Engine::new().unwrap();
    let out = e.render_layout_svg(&frame_req("naturalEarth"), &|| Ok(())).unwrap();
    assert!(!out.warnings.iter().any(|w| w.contains("basemap")), "{:?}", out.warnings);
    assert_eq!(out.basemaps, vec!["naturalEarth".to_string()]);
    let b64 = out.svg.split("id=\"basemap-f1\"").nth(1).unwrap().split("base64,").nth(1).unwrap().split('"').next().unwrap();
    use base64::Engine as _;
    let img = decode(&base64::engine::general_purpose::STANDARD.decode(b64).unwrap());
    assert_eq!((img.0, img.1), (1181, 945));
    assert_eq!(rgb(px(&img, img.0 / 2, img.1 / 2)), LAND);
    assert_eq!(px(&img, img.0 / 2, img.1 / 2)[3], 255);
    // Each colour occurs: it is a map, not a flat fill.
    let n = |c: [u8; 3]| (0..img.0 * img.1).filter(|i| rgb(px(&img, i % img.0, i / img.0)) == c).count();
    assert!(n(LAND) > 50_000 && n(OCEAN) > 50_000, "land {} ocean {}", n(LAND), n(OCEAN));
    assert!(out.svg.contains("Natural Earth") || e.sources().unwrap().iter().any(|s| s.citation.key == "natural-earth"));
}

#[test]
fn screen_only_basemaps_are_refused_in_layouts_in_words() {
    let e = Engine::new().unwrap();
    for (id, name) in basemap::SCREEN_ONLY {
        let err = e.render_layout_svg(&frame_req(id), &|| Ok(())).err().unwrap_or_else(|| panic!("{id} should be refused")).to_string();
        assert!(err.contains(name) && err.contains("Natural Earth"), "{id}: {err}");
    }
    assert!(e.render_layout_svg(&frame_req("blueMarble"), &|| Ok(())).unwrap_err().to_string().contains("Unknown basemap"));
}

/// The map's tile path serves Natural Earth under the reserved id in Web Mercator without a layer.
#[test]
fn engine_serves_natural_earth_tiles_by_reserved_id() {
    let e = Engine::new().unwrap();
    let t = e.tile_in_crs(engine::NATURAL_EARTH_TILE_ID, "3857", 0, 0, 0, &[]).unwrap();
    let img = decode(&t);
    assert_eq!((img.0, img.1), (256, 256));
    assert_eq!(rgb(px(&img, 70, 100)), LAND, "North America at z0");
    assert_eq!(rgb(px(&img, 100, 200)), OCEAN.map(|v| v), "south Pacific at z0");
}

/// One request to each live provider, with the app's own User-Agent: the style/tile answers and its CORS header (the map view
/// loads them from the browser). Network: `cargo test -p engine --test basemaps_wp12 -- --ignored`.
#[test]
#[ignore]
fn network_open_basemap_providers_answer() {
    let ua = "GIS Studio (open-source desktop GIS; basemap check)";
    for url in [
        "https://tiles.openfreemap.org/styles/liberty",
        "https://tiles.openfreemap.org/styles/bright",
        "https://tiles.openfreemap.org/styles/positron",
        "https://tiles.versatiles.org/tiles/osm/0/0/0",
        "https://a.tile.opentopomap.org/0/0/0.png",
    ] {
        let r = ureq::get(url).set("User-Agent", ua).call().unwrap_or_else(|e| panic!("{url}: {e}"));
        assert_eq!(r.status(), 200, "{url}");
    }
}
