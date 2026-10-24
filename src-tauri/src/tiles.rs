//! `gis://` custom protocol: serves vector tiles to MapLibre.
//! URL shapes: gis://localhost/tiles/{layer}/{z}/{x}/{y}.pbf?fields=a,b&v=3 (Web Mercator)
//! and gis://localhost/tiles/{layer}/{crsKey}/{z}/{x}/{y}.pbf (a map CRS's grid; key `3857` = the first).

use engine::Engine;
use percent_encoding::percent_decode_str;
use tauri::http::{Request, Response};

pub fn handle(engine: &Engine, req: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    match tile(engine, req) {
        Ok(body) => Response::builder()
            .status(200)
            .header("Content-Type", if body.starts_with(b"\x89PNG") { "image/png" } else { "application/x-protobuf" })
            .header("Access-Control-Allow-Origin", "*")
            .body(body)
            .unwrap(),
        Err(e) => Response::builder()
            .status(404)
            .header("Access-Control-Allow-Origin", "*")
            .body(e.to_string().into_bytes())
            .unwrap(),
    }
}

fn tile(engine: &Engine, req: &Request<Vec<u8>>) -> anyhow::Result<Vec<u8>> {
    let uri = req.uri();
    let parts: Vec<&str> = uri.path().trim_matches('/').split('/').collect();
    let (layer, key, z, x, y) = match parts.as_slice() {
        ["tiles", layer, z, x, y] => (layer, engine::display::MERCATOR_KEY, z, x, y),
        ["tiles", layer, key, z, x, y] => (layer, *key, z, x, y),
        _ => anyhow::bail!("bad tile path {}", uri.path()),
    };
    let y = y.trim_end_matches(".pbf").trim_end_matches(".png");
    let mut fields = Vec::new();
    for pair in uri.query().unwrap_or("").split('&') {
        if let Some(v) = pair.strip_prefix("fields=") {
            fields = percent_decode_str(v)
                .decode_utf8_lossy()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
        }
    }
    let t = engine.tile_in_crs(layer, key, z.parse()?, x.parse()?, y.parse()?, &fields)?;
    Ok(t.as_ref().clone())
}
