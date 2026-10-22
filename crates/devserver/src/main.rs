//! Development server: the same engine commands and tiles as the desktop app,
//! over HTTP on localhost, so the UI can be run and tested in a browser.
//!
//!   cargo run -p devserver            # then: npm run dev, open http://localhost:1420
//!
//! POST /invoke/<command> with a JSON body of the command's arguments.
//! GET  /tiles/<layer>/<z>/<x>/<y>.pbf?fields=a,b

use anyhow::{anyhow, Result};
use engine::graph::Graph;
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tiny_http::{Header, Method, Response, Server};

const ADDR: &str = "127.0.0.1:7878";

fn main() -> Result<()> {
    let engine = Arc::new(Engine::new()?);
    let server = Arc::new(Server::http(ADDR).map_err(|e| anyhow!("{e}"))?);
    println!("GIS Studio dev server on http://{ADDR}");
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let (server, engine) = (server.clone(), engine.clone());
            std::thread::spawn(move || {
                for mut req in server.incoming_requests() {
                    let cors = Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap();
                    let url = req.url().to_string();
                    if req.method() == &Method::Options {
                        let h = Header::from_bytes("Access-Control-Allow-Headers", "content-type").unwrap();
                        let _ = req.respond(Response::empty(204).with_header(cors).with_header(h));
                        continue;
                    }
                    if let Some(rest) = url.strip_prefix("/tiles/") {
                        let resp = match tile(&engine, rest) {
                            Ok(b) => {
                                let ct = if b.starts_with(b"\x89PNG") { "image/png" } else { "application/x-protobuf" };
                                Response::from_data(b).with_header(Header::from_bytes("Content-Type", ct).unwrap())
                            }
                            Err(e) => Response::from_data(e.to_string().into_bytes()).with_status_code(404),
                        };
                        let _ = req.respond(resp.with_header(cors));
                    } else if let Some(cmd) = url.strip_prefix("/invoke/") {
                        let mut body = String::new();
                        let _ = req.as_reader().read_to_string(&mut body);
                        let args: Value = serde_json::from_str(&body).unwrap_or(json!({}));
                        let (code, out) = match invoke(&engine, cmd, &args) {
                            Ok(v) => (200, v),
                            Err(e) => (500, json!(format!("{e:#}"))),
                        };
                        let resp = Response::from_string(out.to_string())
                            .with_status_code(code)
                            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                        let _ = req.respond(resp.with_header(cors));
                    } else {
                        let _ = req.respond(Response::from_string("not found").with_status_code(404));
                    }
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
    Ok(())
}

fn tile(engine: &Engine, rest: &str) -> Result<Vec<u8>> {
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let p: Vec<&str> = path.split('/').collect();
    let [layer, z, x, y] = p.as_slice() else { return Err(anyhow!("bad tile path")) };
    let fields: Vec<String> = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("fields="))
        .map(|v| v.replace("%2C", ",").split(',').filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    Ok(engine.tile(layer, z.parse()?, x.parse()?, y.trim_end_matches(".pbf").trim_end_matches(".png").parse()?, &fields)?.as_ref().clone())
}

fn s(a: &Value, k: &str) -> Result<String> {
    a[k].as_str().map(String::from).ok_or_else(|| anyhow!("missing '{k}'"))
}
fn f(a: &Value, k: &str) -> Result<f64> {
    a[k].as_f64().ok_or_else(|| anyhow!("missing '{k}'"))
}
fn de<T: serde::de::DeserializeOwned>(a: &Value, k: &str) -> Result<T> {
    Ok(serde_json::from_value(a[k].clone())?)
}

/// Mirrors src-tauri/src/commands.rs.
fn invoke(e: &Engine, cmd: &str, a: &Value) -> Result<Value> {
    Ok(match cmd {
        "import_files" => {
            let paths: Vec<PathBuf> = de(a, "paths")?;
            let mut all = Vec::new();
            for p in paths {
                all.extend(e.import(&p)?);
            }
            for l in &all {
                let _ = e.prewarm(&l.id, 8, &[]);
            }
            json!(all)
        }
        "list_layers" => json!(e.list_layers()),
        "remove_layer" => json!(e.remove_layer(&s(a, "id")?)?),
        "add_field" => json!(e.add_field(&s(a, "id")?, &s(a, "name")?, &s(a, "ty")?)?),
        "delete_field" => json!(e.delete_field(&s(a, "id")?, &s(a, "name")?)?),
        "rename_field" => json!(e.rename_field(&s(a, "id")?, &s(a, "old")?, &s(a, "new")?)?),
        "update_cell" => json!(e.update_cell(&s(a, "id")?, a["fid"].as_i64().unwrap_or(0), &s(a, "field")?, a["value"].as_str())?),
        "calculate_field" => {
            let fids: Option<Vec<i64>> = de(a, "fids")?;
            let (layer, count) = e.calculate_field(&s(a, "id")?, &s(a, "field")?, &s(a, "expr")?, a["ty"].as_str(), fids.as_deref())?;
            json!({ "layer": layer, "count": count })
        }
        "calculate_geometry" => {
            let fids: Option<Vec<i64>> = de(a, "fids")?;
            let (layer, count) = e.calculate_geometry(&s(a, "id")?, &s(a, "field")?, &s(a, "property")?, &s(a, "areaUnits")?, &s(a, "lengthUnits")?, fids.as_deref())?;
            json!({ "layer": layer, "count": count })
        }
        "delete_rows" => {
            let fids: Vec<i64> = de(a, "fids")?;
            let (layer, count) = e.delete_rows(&s(a, "id")?, &fids)?;
            json!({ "layer": layer, "count": count })
        }
        "tiger_datasets" => json!(engine::tiger::DATASETS),
        "tiger_counties" => json!(engine::tiger::counties(&s(a, "state")?)?),
        "tiger_import" => {
            let ds: Vec<String> = de(a, "datasets")?;
            let out = e.tiger_import(&ds, &s(a, "state")?, a["county"].as_str(), &s(a, "label")?)?;
            for l in &out {
                let _ = e.prewarm(&l.id, 8, &[]);
            }
            json!(out)
        }
        "download_elevation" => {
            let l = e.download_elevation(de(a, "bbox")?, f(a, "resolution")?, &s(a, "label")?)?;
            json!(l)
        }
        "raster_value" => json!(e.raster_value(&s(a, "id")?, f(a, "lon")?, f(a, "lat")?)?),
        "copy_layer" => {
            let fids: Option<Vec<i64>> = de(a, "fids")?;
            json!(e.copy_layer(&s(a, "id")?, &s(a, "name")?, fids.as_deref())?)
        }
        "set_definition_query" => json!(e.set_definition_query(&s(a, "id")?, a["query"].as_str())?),
        "rename_layer" => json!(e.rename_layer(&s(a, "id")?, &s(a, "name")?)?),
        "define_projection" => json!(e.define_projection(&s(a, "id")?, &s(a, "crs")?, a["crsName"].as_str().map(String::from))?),
        "suggest_crs" => json!(e.suggest_crs(&s(a, "id")?)?),
        "validate_crs" => json!(e.validate_crs(&s(a, "crs")?)?),
        "transform_point" => json!(e.transform_point(f(a, "x")?, f(a, "y")?, &s(a, "from")?, &s(a, "to")?)?),
        "get_rows" => json!(e.rows(&s(a, "id")?, &de::<RowsQuery>(a, "query")?)?),
        "select_where" => json!(e.select_where(&s(a, "id")?, &s(a, "filter")?)?),
        "identify" | "select_box" => {
            let ids: Vec<String> = de(a, "ids")?;
            let mut out = Vec::new();
            for id in ids {
                let fids = if cmd == "identify" {
                    e.identify(&id, f(a, "lon")?, f(a, "lat")?, f(a, "zoom")?)?
                } else {
                    e.select_box(&id, de(a, "bbox")?)?
                };
                if !fids.is_empty() {
                    out.push(json!({ "layer": id, "fids": fids }));
                }
            }
            json!(out)
        }
        "get_feature" => json!(e.feature(&s(a, "id")?, a["fid"].as_i64().unwrap_or(0))?),
        "layer_extent" => json!(e.extent(&s(a, "id")?, de(a, "fids")?)?),
        "unique_values" => json!(e.unique_values(&s(a, "id")?, &s(a, "field")?, a["limit"].as_u64().unwrap_or(100) as usize)?),
        "quantile_breaks" => json!(e.quantile_breaks(&s(a, "id")?, &s(a, "field")?, a["classes"].as_u64().unwrap_or(5) as usize)?),
        "tool_specs" => json!(e.tool_specs()),
        "run_graph" => {
            let g: Graph = de(a, "graph")?;
            let res = e.run_graph(&g, a["target"].as_str(), &mut |_| {})?;
            for l in res.iter().filter_map(|r| r.layer.as_ref()) {
                let _ = e.prewarm(&l.id, 8, &[]);
            }
            json!(res)
        }
        "preview_output" => json!(e.preview_output(&s(a, "output")?, a["crs"].as_str().map(String::from), &s(a, "name")?)?),
        "clear_model_cache" => json!(e.clear_model_cache()?),
        "generate_test_layer" => {
            let l = e.generate_test_layer(&s(a, "kind")?, a["count"].as_u64().unwrap_or(1000), de(a, "bbox")?)?;
            let _ = e.prewarm(&l.id, 8, &[]);
            json!(l)
        }
        "save_project" => json!(e.save_project(&PathBuf::from(s(a, "dir")?), &s(a, "uiState")?)?),
        "open_project" => {
            let ui = e.open_project(&PathBuf::from(s(a, "dir")?))?;
            json!({ "uiState": ui, "layers": e.list_layers() })
        }
        "new_project" => json!(e.new_project()?),
        other => return Err(anyhow!("unknown command {other}")),
    })
}
