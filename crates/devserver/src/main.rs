//! Development server: the same engine commands and tiles as the desktop app,
//! over HTTP on localhost, so the UI can be run and tested in a browser.
//!
//!   cargo run -p devserver            # then: npm run dev, open http://localhost:1420
//!
//! POST /invoke/<command> with a JSON body of the command's arguments.
//! GET  /tiles/<layer>/<z>/<x>/<y>.pbf?fields=a,b          (Web Mercator)
//! GET  /tiles/<layer>/<crsKey>/<z>/<x>/<y>.pbf?fields=a,b (a map CRS's grid)

use anyhow::{anyhow, Result};
use engine::graph::Graph;
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tiny_http::{Header, Method, Response, Server};

const DEFAULT_ADDR: &str = "127.0.0.1:7878";

fn main() -> Result<()> {
    // GIS_DEVSERVER_ADDR lets scripts/e2e.sh pick a free port.
    let addr = std::env::var("GIS_DEVSERVER_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let engine = Arc::new(Engine::new()?);
    let server = Arc::new(Server::http(addr.as_str()).map_err(|e| anyhow!("{e}"))?);
    println!("GIS Studio dev server on http://{addr}");
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
    let (layer, key, z, x, y) = match p.as_slice() {
        [layer, z, x, y] => (layer, engine::display::MERCATOR_KEY, z, x, y),
        [layer, key, z, x, y] => (layer, *key, z, x, y),
        _ => return Err(anyhow!("bad tile path")),
    };
    let fields: Vec<String> = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("fields="))
        .map(|v| v.replace("%2C", ",").split(',').filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    Ok(engine.tile_in_crs(layer, key, z.parse()?, x.parse()?, y.trim_end_matches(".pbf").trim_end_matches(".png").parse()?, &fields)?.as_ref().clone())
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
fn invoke(e: &Arc<Engine>, cmd: &str, a: &Value) -> Result<Value> {
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
        "tiger_vintages" => json!(engine::tiger::vintages()?),
        "tiger_availability" => json!(engine::tiger::availability(&s(a, "vintage")?)?),
        "tiger_counties" => json!(engine::tiger::counties(&s(a, "state")?, &s(a, "vintage")?)?),
        "tiger_import" => {
            let ds: Vec<String> = de(a, "datasets")?;
            let out = e.tiger_import(&ds, &s(a, "vintage")?, &s(a, "state")?, a["county"].as_str(), &s(a, "label")?)?;
            for l in &out {
                let _ = e.prewarm(&l.id, 8, &[]);
            }
            json!(out)
        }
        "tnm_datasets" => json!(engine::tnm_vector::DATASETS),
        "tnm_import" => {
            let ds: Vec<String> = de(a, "datasets")?;
            let out = e.tnm_import(&ds, de(a, "bbox")?, a["clipLayer"].as_str(), a["clip"].as_bool().unwrap_or(true), &s(a, "label")?)?;
            for l in &out.layers {
                let _ = e.prewarm(&l.id, 8, &[]);
            }
            json!(out)
        }
        "progress" => json!(engine::progress::snapshot()),
        "select_by_location" => {
            let by_fids: Option<Vec<i64>> = de(a, "byFids").ok();
            json!(e.select_by_location(&s(a, "target")?, &s(a, "by")?, by_fids.as_deref(), &s(a, "relation")?, f(a, "distance")?, &s(a, "units")?, a["invert"].as_bool().unwrap_or(false))?)
        }
        "fs_roots" => json!(engine::catalog::roots()),
        "fs_list" => json!(engine::catalog::list(&s(a, "path")?)?),
        "fs_inspect" => json!(engine::catalog::inspect(&s(a, "path")?)?),
        "fs_stat" => json!(engine::catalog::stat(&de::<Vec<String>>(a, "paths")?)),
        "save_text" => {
            let path = std::path::PathBuf::from(s(a, "path")?);
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, s(a, "text")?)?;
            json!(null)
        }
        "sources" => json!(e.sources()?),
        "update_source" => {
            e.update_source(&de(a, "citation")?)?;
            json!(null)
        }
        "remove_source" => {
            e.remove_source(&s(a, "key")?)?;
            json!(null)
        }
        "record_basemap" => {
            if let Some(c) = engine::sources::for_basemap(&s(a, "id")?) {
                e.record_source(&c, "Basemap")?;
            }
            json!(null)
        }
        "cancel_task" => {
            engine::progress::cancel(a["id"].as_u64().unwrap_or(0));
            json!(null)
        }
        "plan_elevation" => json!(e.plan_elevation(de(a, "bbox")?, f(a, "resolution")?)?),
        "download_elevation" => {
            let l = e.download_elevation(de(a, "bbox")?, f(a, "resolution")?, &s(a, "label")?, a["confirmed"].as_bool().unwrap_or(false))?;
            json!(l)
        }
        "measure_feature" => json!(e.measure_feature(&s(a, "id")?, a["fid"].as_i64().unwrap_or(0))?),
        "create_features" => {
            let feats: Vec<serde_json::Value> = de(a, "features")?;
            let list: Vec<(String, serde_json::Map<String, Value>)> = feats
                .into_iter()
                .map(|f| (f["wkt"].as_str().unwrap_or("").to_string(), f["props"].as_object().cloned().unwrap_or_default()))
                .collect();
            json!(e.create_features(&s(a, "name")?, &list)?)
        }
        "raster_value" => json!(e.raster_value(&s(a, "id")?, f(a, "lon")?, f(a, "lat")?)?),
        "copy_layer" => {
            let fids: Option<Vec<i64>> = de(a, "fids")?;
            json!(e.copy_layer(&s(a, "id")?, &s(a, "name")?, fids.as_deref())?)
        }
        "split_groups" => json!(e.split_groups(&s(a, "id")?, &s(a, "field")?)?),
        "split_by_attribute" => {
            let target: engine::split::SplitTarget = de(a, "target")?;
            json!(e.split_by_attribute(&s(a, "id")?, &s(a, "field")?, &target, a["confirmed"].as_bool().unwrap_or(false))?)
        }
        "datum_data_status" => json!(e.datum_data_status()),
        "set_definition_query" => json!(e.set_definition_query(&s(a, "id")?, a["query"].as_str())?),
        "rename_layer" => json!(e.rename_layer(&s(a, "id")?, &s(a, "name")?)?),
        "define_projection" => json!(e.define_projection(&s(a, "id")?, &s(a, "crs")?, a["crsName"].as_str().map(String::from))?),
        "suggest_crs" => json!(e.suggest_crs(&s(a, "id")?)?),
        "crs_info" => json!(e.crs_info(&s(a, "crs")?)?),
        "tissot_layer" => json!(e.tissot_layer(&s(a, "crs")?, f(a, "spacingDeg")?, f(a, "radiusKm")?)?),
        "same_crs" => json!(e.same_crs(&s(a, "a")?, &s(a, "b")?)),
        "world_outline" => json!(e.world_outline()?),
        "world_reference" => json!(e.world_reference(&s(a, "crs")?, f(a, "spacingDeg")?)?),
        "map_factors" => json!(e.map_factors(&s(a, "crs")?, f(a, "x")?, f(a, "y")?)?),
        "validate_crs" => json!(e.validate_crs(&s(a, "crs")?)?),
        "transform_point" => json!(e.transform_point(f(a, "x")?, f(a, "y")?, &s(a, "from")?, &s(a, "to")?)?),
        "map_scale_factor" => json!(e.map_scale_factor(&s(a, "crs")?, f(a, "x")?, f(a, "y")?)?),
        "crs_display_info" => json!(e.crs_display_info(&s(a, "crs")?, de(a, "fallbackExtent").ok())?),
        "build_display" => json!(e.build_display(&s(a, "layer")?, &s(a, "crs")?)?),
        "display_status" => json!(e.display_status(&s(a, "layer")?, &s(a, "crs")?)?),
        "get_rows" => json!(e.rows(&s(a, "id")?, &de::<RowsQuery>(a, "query")?)?),
        "select_where" => json!(e.select_where(&s(a, "id")?, &s(a, "filter")?)?),
        "identify" | "select_rect" => {
            let ids: Vec<String> = de(a, "ids")?;
            let mut out = Vec::new();
            for id in ids {
                let fids = if cmd == "identify" {
                    e.identify(&id, f(a, "x")?, f(a, "y")?, &s(a, "crs")?, f(a, "tolerance")?)?
                } else {
                    e.select_rect(&id, de(a, "bbox")?, &s(a, "crs")?)?
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
        "prune_node_cache" => json!(e.prune_node_cache(&de::<Vec<String>>(a, "live")?, a["pinned"].as_str())?),
        "set_node_cache_budget" => json!(e.set_node_cache_budget(a["bytes"].as_u64().ok_or_else(|| anyhow!("Missing 'bytes'"))?)),
        "node_table_info" => json!(e.node_table_info(&s(a, "table")?)?),
        "node_rows" => json!(e.node_rows(&s(a, "table")?, &de::<RowsQuery>(a, "query")?)?),
        "node_diff" => json!(e.node_diff(&s(a, "a")?, &s(a, "b")?, a["key"].as_str())?),
        "node_diff_rows" => json!(e.node_diff_rows(&s(a, "diffId")?, &s(a, "kind")?, a["offset"].as_u64().unwrap_or(0) as usize, a["limit"].as_u64().unwrap_or(100) as usize)?),
        "drop_node_diff" => json!(e.drop_node_diff(&s(a, "diffId")?)?),
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
        "list_layouts" => json!(e.list_layouts()?),
        "save_layout" => {
            e.save_layout(&de(a, "layout")?)?;
            json!(null)
        }
        "layout_series_pages" => json!(e.series_pages(&de(a, "layout")?)?),
        "export_layout_series" => {
            let req: engine::layout::model::RenderRequest = de(a, "req")?;
            let fmt = engine::layout::ExportFormat::parse(&s(a, "format")?)?;
            json!(e.export_series(&req, fmt, f(a, "dpi")?, std::path::Path::new(&s(a, "path")?))?)
        }
        "list_layout_templates" => json!(e.layout_templates()),
        "get_layout_template" => json!(e.layout_template(&s(a, "id")?)?),
        "save_layout_template" => json!(e.save_layout_template(&s(a, "name")?, &de(a, "layout")?)?),
        "delete_layout_template" => json!(e.delete_layout_template(&s(a, "id")?)?),
        "delete_layout" => json!(e.delete_layout(&s(a, "id")?)?),
        "render_layout_preview" => json!(e.render_layout_preview(&de(a, "req")?, f(a, "dpi")?)?),
        "export_layout" => json!(e.export_layout(&de(a, "req")?, engine::layout::ExportFormat::parse(&s(a, "format")?)?, f(a, "dpi")?, &PathBuf::from(s(a, "path")?))?),
        "new_project" => json!(e.new_project()?),
        other => return Err(anyhow!("unknown command {other}")),
    })
}
