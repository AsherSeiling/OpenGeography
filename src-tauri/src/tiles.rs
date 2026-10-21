//! `gis://` custom protocol: serves vector tiles to MapLibre.
//! URL shape: gis://localhost/tiles/{layer}/{z}/{x}/{y}.pbf?fields=a,b&v=3

use engine::Engine;
use percent_encoding::percent_decode_str;
use tauri::http::{Request, Response};

pub fn handle(engine: &Engine, req: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    match tile(engine, req) {
        Ok(body) => Response::builder()
            .status(200)
            .header("Content-Type", "application/x-protobuf")
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
    let ["tiles", layer, z, x, y] = parts.as_slice() else {
        anyhow::bail!("bad tile path {}", uri.path());
    };
    let y = y.trim_end_matches(".pbf");
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
    let t = engine.tile(layer, z.parse()?, x.parse()?, y.parse()?, &fields)?;
    Ok(t.as_ref().clone())
}
