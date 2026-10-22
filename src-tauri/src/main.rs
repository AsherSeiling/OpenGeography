// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod tiles;

use engine::Engine;
use std::sync::Arc;

fn main() {
    let engine = Arc::new(Engine::new().expect("failed to start the GIS engine"));
    let tile_engine = engine.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(engine)
        .register_asynchronous_uri_scheme_protocol("gis", move |_ctx, request, responder| {
            let engine = tile_engine.clone();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(tiles::handle(&engine, &request));
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_files,
            commands::list_layers,
            commands::remove_layer,
            commands::rename_layer,
            commands::copy_layer,
            commands::define_projection,
            commands::suggest_crs,
            commands::validate_crs,
            commands::transform_point,
            commands::get_rows,
            commands::select_where,
            commands::identify,
            commands::select_box,
            commands::get_feature,
            commands::layer_extent,
            commands::unique_values,
            commands::quantile_breaks,
            commands::tool_specs,
            commands::run_graph,
            commands::preview_output,
            commands::clear_model_cache,
            commands::generate_test_layer,
            commands::save_project,
            commands::open_project,
            commands::new_project,
            commands::frontend_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running GIS Studio");
}
