// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod selftest;
mod tiles;

use engine::Engine;
use std::sync::Arc;

fn main() {
    // Bundled GDAL/PROJ data and DuckDB extension (installed app), before anything starts.
    engine::resources::init();
    if std::env::args().any(|a| a == "--self-test") {
        std::process::exit(selftest::run());
    }
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
            commands::set_definition_query,
            commands::define_projection,
            commands::suggest_crs,
            commands::validate_crs,
            commands::transform_point,
            commands::map_scale_factor,
            commands::crs_display_info,
            commands::build_display,
            commands::display_status,
            commands::get_rows,
            commands::select_where,
            commands::identify,
            commands::select_rect,
            commands::get_feature,
            commands::layer_extent,
            commands::unique_values,
            commands::quantile_breaks,
            commands::tool_specs,
            commands::run_graph,
            commands::preview_output,
            commands::clear_model_cache,
            commands::prune_node_cache,
            commands::set_node_cache_budget,
            commands::node_table_info,
            commands::node_rows,
            commands::node_diff,
            commands::node_diff_rows,
            commands::drop_node_diff,
            commands::generate_test_layer,
            commands::save_project,
            commands::open_project,
            commands::new_project,
            commands::frontend_log,
            commands::add_field,
            commands::delete_field,
            commands::rename_field,
            commands::update_cell,
            commands::calculate_field,
            commands::calculate_geometry,
            commands::delete_rows,
            commands::tiger_datasets,
            commands::tiger_counties,
            commands::tiger_import,
            commands::tiger_vintages,
            commands::tiger_availability,
            commands::tnm_datasets,
            commands::tnm_import,
            commands::progress,
            commands::cancel_task,
            commands::list_layouts,
            commands::save_layout,
            commands::delete_layout,
            commands::render_layout_preview,
            commands::export_layout,
            commands::sources,
            commands::update_source,
            commands::remove_source,
            commands::record_basemap,
            commands::save_text,
            commands::select_by_location,
            commands::fs_roots,
            commands::fs_list,
            commands::fs_inspect,
            commands::fs_stat,
            commands::download_elevation,
            commands::raster_value,
            commands::measure_feature,
            commands::create_features,
        ])
        .run(tauri::generate_context!())
        .expect("error while running GIS Studio");
}
