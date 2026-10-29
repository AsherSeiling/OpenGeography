//! IPC commands. Each runs on a blocking worker thread so the UI never waits
//! on DuckDB. Errors are returned to the UI as plain strings.

use engine::crs::Suggestion;
use engine::display::{DisplayInfo, DisplayStatus};
use engine::graph::{Graph, NodeResult};
use engine::layers::LayerInfo;
use engine::table::{RowsPage, RowsQuery};
use engine::Engine;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

type Eng<'a> = State<'a, Arc<Engine>>;
type Res<T> = Result<T, String>;

async fn blocking<T, F>(engine: &Arc<Engine>, f: F) -> Res<T>
where
    T: Send + 'static,
    F: FnOnce(&Engine) -> anyhow::Result<T> + Send + 'static,
{
    let e = engine.clone();
    tauri::async_runtime::spawn_blocking(move || f(&e))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

/// Build low-zoom tiles in the background after data changes.
fn prewarm(engine: &Arc<Engine>, ids: Vec<String>) {
    let e = engine.clone();
    tauri::async_runtime::spawn_blocking(move || {
        for id in ids {
            let _ = e.prewarm(&id, 8, &[]);
        }
    });
}

#[tauri::command]
pub async fn import_files(engine: Eng<'_>, paths: Vec<PathBuf>) -> Res<Vec<LayerInfo>> {
    let out = blocking(&engine, move |e| {
        let mut all = Vec::new();
        for p in paths {
            all.extend(e.import(&p)?);
        }
        Ok(all)
    })
    .await?;
    prewarm(&engine, out.iter().map(|l| l.id.clone()).collect());
    Ok(out)
}

#[tauri::command]
pub async fn list_layers(engine: Eng<'_>) -> Res<Vec<LayerInfo>> {
    blocking(&engine, |e| Ok(e.list_layers())).await
}

#[tauri::command]
pub async fn remove_layer(engine: Eng<'_>, id: String) -> Res<()> {
    blocking(&engine, move |e| e.remove_layer(&id)).await
}

#[tauri::command]
pub async fn copy_layer(engine: Eng<'_>, id: String, name: String, fids: Option<Vec<i64>>) -> Res<LayerInfo> {
    let info = blocking(&engine, move |e| e.copy_layer(&id, &name, fids.as_deref())).await?;
    prewarm(&engine, vec![info.id.clone()]);
    Ok(info)
}

/// The values of a field with row counts, for the Split by Attribute dialog (B6).
#[tauri::command]
pub async fn split_groups(engine: Eng<'_>, id: String, field: String) -> Res<Vec<engine::split::SplitGroup>> {
    blocking(&engine, move |e| e.split_groups(&id, &field)).await
}

/// Split a layer by a field into new layers or files (B6).
#[tauri::command]
pub async fn split_by_attribute(engine: Eng<'_>, id: String, field: String, target: engine::split::SplitTarget, confirmed: bool) -> Res<engine::split::SplitResult> {
    let r = blocking(&engine, move |e| e.split_by_attribute(&id, &field, &target, confirmed)).await?;
    prewarm(&engine, r.layers.iter().map(|l| l.id.clone()).collect());
    Ok(r)
}

/// Known-answer check of the datum shifts (RD-3): the UI shows an error when PROJ's datum files are missing.
#[tauri::command]
pub async fn datum_data_status(engine: Eng<'_>) -> Res<engine::crs::DatumReport> {
    blocking(&engine, |e| Ok(e.datum_data_status())).await
}

#[tauri::command]
pub async fn set_definition_query(engine: Eng<'_>, id: String, query: Option<String>) -> Res<LayerInfo> {
    let info = blocking(&engine, move |e| e.set_definition_query(&id, query.as_deref())).await?;
    prewarm(&engine, vec![info.id.clone()]);
    Ok(info)
}

#[tauri::command]
pub async fn rename_layer(engine: Eng<'_>, id: String, name: String) -> Res<LayerInfo> {
    blocking(&engine, move |e| e.rename_layer(&id, &name)).await
}

#[tauri::command]
pub async fn define_projection(engine: Eng<'_>, id: String, crs: String, crs_name: Option<String>) -> Res<LayerInfo> {
    let info = blocking(&engine, move |e| e.define_projection(&id, &crs, crs_name)).await?;
    prewarm(&engine, vec![info.id.clone()]);
    Ok(info)
}

#[tauri::command]
pub async fn suggest_crs(engine: Eng<'_>, id: String) -> Res<Vec<Suggestion>> {
    blocking(&engine, move |e| e.suggest_crs(&id)).await
}

#[tauri::command]
pub async fn crs_info(engine: Eng<'_>, crs: String) -> Res<engine::crs::CrsInfo> {
    blocking(&engine, move |e| e.crs_info(&crs)).await
}

#[tauri::command]
pub async fn tissot_layer(engine: Eng<'_>, crs: String, spacing_deg: f64, radius_km: f64) -> Res<(engine::layers::LayerInfo, usize)> {
    blocking(&engine, move |e| e.tissot_layer(&crs, spacing_deg, radius_km)).await
}

#[tauri::command]
pub async fn same_crs(engine: Eng<'_>, a: String, b: String) -> Res<bool> {
    blocking(&engine, move |e| Ok(e.same_crs(&a, &b))).await
}

#[tauri::command]
pub async fn world_outline(engine: Eng<'_>) -> Res<engine::layers::LayerInfo> {
    blocking(&engine, move |e| e.world_outline()).await
}

#[tauri::command]
pub async fn world_reference(engine: Eng<'_>, crs: String, spacing_deg: f64) -> Res<Vec<engine::layers::LayerInfo>> {
    blocking(&engine, move |e| e.world_reference(&crs, spacing_deg)).await
}

#[tauri::command]
pub async fn map_factors(engine: Eng<'_>, crs: String, x: f64, y: f64) -> Res<(f64, f64)> {
    blocking(&engine, move |e| e.map_factors(&crs, x, y)).await
}

#[tauri::command]
pub async fn validate_crs(engine: Eng<'_>, crs: String) -> Res<()> {
    blocking(&engine, move |e| e.validate_crs(&crs)).await
}

#[tauri::command]
pub async fn transform_point(engine: Eng<'_>, x: f64, y: f64, from: String, to: String) -> Res<(f64, f64)> {
    blocking(&engine, move |e| e.transform_point(x, y, &from, &to)).await
}

#[tauri::command]
pub async fn map_scale_factor(engine: Eng<'_>, crs: String, x: f64, y: f64) -> Res<f64> {
    blocking(&engine, move |e| e.map_scale_factor(&crs, x, y)).await
}

/// Grid, proj4 string and key for drawing the map in `crs`.
#[tauri::command]
pub async fn crs_display_info(engine: Eng<'_>, crs: String, fallback_extent: Option<[f64; 4]>) -> Res<DisplayInfo> {
    blocking(&engine, move |e| e.crs_display_info(&crs, fallback_extent)).await
}

/// Start projecting a layer for display in `crs` in the background.
#[tauri::command]
pub async fn build_display(engine: Eng<'_>, layer: String, crs: String) -> Res<DisplayStatus> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.build_display(&layer, &crs))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn display_status(engine: Eng<'_>, layer: String, crs: String) -> Res<DisplayStatus> {
    blocking(&engine, move |e| e.display_status(&layer, &crs)).await
}

#[tauri::command]
pub async fn get_rows(engine: Eng<'_>, id: String, query: RowsQuery) -> Res<RowsPage> {
    blocking(&engine, move |e| e.rows(&id, &query)).await
}

#[tauri::command]
pub async fn select_where(engine: Eng<'_>, id: String, filter: String) -> Res<Vec<i64>> {
    blocking(&engine, move |e| e.select_where(&id, &filter)).await
}

#[derive(Serialize)]
pub struct LayerHits {
    layer: String,
    fids: Vec<i64>,
}

#[tauri::command]
pub async fn identify(engine: Eng<'_>, ids: Vec<String>, x: f64, y: f64, crs: String, tolerance: f64) -> Res<Vec<LayerHits>> {
    blocking(&engine, move |e| {
        let mut out = Vec::new();
        for id in ids {
            let fids = e.identify(&id, x, y, &crs, tolerance)?;
            if !fids.is_empty() {
                out.push(LayerHits { layer: id, fids });
            }
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn select_rect(engine: Eng<'_>, ids: Vec<String>, bbox: [f64; 4], crs: String) -> Res<Vec<LayerHits>> {
    blocking(&engine, move |e| {
        ids.into_iter()
            .map(|id| Ok(LayerHits { fids: e.select_rect(&id, bbox, &crs)?, layer: id }))
            .collect()
    })
    .await
}

#[tauri::command]
pub async fn get_feature(engine: Eng<'_>, id: String, fid: i64) -> Res<Vec<(String, Option<String>)>> {
    blocking(&engine, move |e| e.feature(&id, fid)).await
}

#[tauri::command]
pub async fn layer_extent(engine: Eng<'_>, id: String, fids: Option<Vec<i64>>) -> Res<Option<[f64; 4]>> {
    blocking(&engine, move |e| e.extent(&id, fids)).await
}

#[tauri::command]
pub async fn unique_values(engine: Eng<'_>, id: String, field: String, limit: Option<usize>) -> Res<Vec<(String, i64)>> {
    blocking(&engine, move |e| e.unique_values(&id, &field, limit.unwrap_or(100))).await
}

#[tauri::command]
pub async fn quantile_breaks(engine: Eng<'_>, id: String, field: String, classes: usize) -> Res<Vec<f64>> {
    blocking(&engine, move |e| e.quantile_breaks(&id, &field, classes)).await
}

#[tauri::command]
pub async fn tool_specs(engine: Eng<'_>) -> Res<Vec<engine::nodes::SpecOut>> {
    blocking(&engine, |e| Ok(e.tool_specs())).await
}

#[tauri::command]
pub async fn run_graph(app: AppHandle, engine: Eng<'_>, graph: Graph, target: Option<String>) -> Res<Vec<NodeResult>> {
    let results = blocking(&engine, move |e| {
        e.run_graph(&graph, target.as_deref(), &mut |r| {
            let _ = app.emit("model-status", r);
        })
    })
    .await?;
    prewarm(&engine, results.iter().filter_map(|r| r.layer.as_ref().map(|l| l.id.clone())).collect());
    Ok(results)
}

#[tauri::command]
pub async fn preview_output(engine: Eng<'_>, output: String, crs: Option<String>, name: String) -> Res<LayerInfo> {
    blocking(&engine, move |e| e.preview_output(&output, crs, &name)).await
}

#[tauri::command]
pub async fn clear_model_cache(engine: Eng<'_>) -> Res<()> {
    blocking(&engine, |e| e.clear_model_cache()).await
}

#[tauri::command]
pub async fn prune_node_cache(engine: Eng<'_>, live: Vec<String>, pinned: Option<String>) -> Res<usize> {
    blocking(&engine, move |e| e.prune_node_cache(&live, pinned.as_deref())).await
}

#[tauri::command]
pub async fn set_node_cache_budget(engine: Eng<'_>, bytes: u64) -> Res<()> {
    blocking(&engine, move |e| {
        e.set_node_cache_budget(bytes);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn node_table_info(engine: Eng<'_>, table: String) -> Res<engine::history::NodeTableInfo> {
    blocking(&engine, move |e| e.node_table_info(&table)).await
}

#[tauri::command]
pub async fn node_rows(engine: Eng<'_>, table: String, query: RowsQuery) -> Res<RowsPage> {
    blocking(&engine, move |e| e.node_rows(&table, &query)).await
}

#[tauri::command]
pub async fn node_diff(engine: Eng<'_>, a: String, b: String, key: Option<String>) -> Res<engine::history::DiffSummary> {
    blocking(&engine, move |e| e.node_diff(&a, &b, key.as_deref())).await
}

#[tauri::command]
pub async fn node_diff_rows(engine: Eng<'_>, diff_id: String, kind: String, offset: usize, limit: usize) -> Res<Vec<engine::history::DiffRow>> {
    blocking(&engine, move |e| e.node_diff_rows(&diff_id, &kind, offset, limit)).await
}

#[tauri::command]
pub async fn drop_node_diff(engine: Eng<'_>, diff_id: String) -> Res<()> {
    blocking(&engine, move |e| e.drop_node_diff(&diff_id)).await
}

#[tauri::command]
pub async fn generate_test_layer(engine: Eng<'_>, kind: String, count: u64, bbox: [f64; 4]) -> Res<LayerInfo> {
    let info = blocking(&engine, move |e| e.generate_test_layer(&kind, count, bbox)).await?;
    prewarm(&engine, vec![info.id.clone()]);
    Ok(info)
}

#[tauri::command]
pub async fn save_project(engine: Eng<'_>, dir: PathBuf, ui_state: String) -> Res<()> {
    blocking(&engine, move |e| e.save_project(&dir, &ui_state)).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedProject {
    ui_state: String,
    layers: Vec<LayerInfo>,
}

#[tauri::command]
pub async fn open_project(engine: Eng<'_>, dir: PathBuf) -> Res<OpenedProject> {
    blocking(&engine, move |e| {
        let ui_state = e.open_project(&dir)?;
        Ok(OpenedProject { ui_state, layers: e.list_layers() })
    })
    .await
}

#[tauri::command]
pub async fn new_project(engine: Eng<'_>) -> Res<()> {
    blocking(&engine, |e| e.new_project()).await
}

/// Messages from the UI (startup report, uncaught errors), printed to the
/// app's stderr so problems in release builds can be diagnosed.
#[tauri::command]
pub fn frontend_log(level: String, message: String) {
    eprintln!("[ui:{level}] {message}");
}

// ---------- attribute editing ----------

#[derive(Serialize)]
pub struct EditResult {
    layer: LayerInfo,
    count: i64,
}

#[tauri::command]
pub async fn add_field(engine: Eng<'_>, id: String, name: String, ty: String) -> Res<LayerInfo> {
    blocking(&engine, move |e| e.add_field(&id, &name, &ty)).await
}

#[tauri::command]
pub async fn delete_field(engine: Eng<'_>, id: String, name: String) -> Res<LayerInfo> {
    blocking(&engine, move |e| e.delete_field(&id, &name)).await
}

#[tauri::command]
pub async fn rename_field(engine: Eng<'_>, id: String, old: String, new: String) -> Res<LayerInfo> {
    blocking(&engine, move |e| e.rename_field(&id, &old, &new)).await
}

#[tauri::command]
pub async fn update_cell(engine: Eng<'_>, id: String, fid: i64, field: String, value: Option<String>) -> Res<LayerInfo> {
    blocking(&engine, move |e| e.update_cell(&id, fid, &field, value.as_deref())).await
}

#[tauri::command]
pub async fn calculate_field(engine: Eng<'_>, id: String, field: String, expr: String, ty: Option<String>, fids: Option<Vec<i64>>) -> Res<EditResult> {
    blocking(&engine, move |e| {
        let (layer, count) = e.calculate_field(&id, &field, &expr, ty.as_deref(), fids.as_deref())?;
        Ok(EditResult { layer, count })
    })
    .await
}

#[tauri::command]
pub async fn calculate_geometry(engine: Eng<'_>, id: String, field: String, property: String, area_units: String, length_units: String, fids: Option<Vec<i64>>) -> Res<EditResult> {
    blocking(&engine, move |e| {
        let (layer, count) = e.calculate_geometry(&id, &field, &property, &area_units, &length_units, fids.as_deref())?;
        Ok(EditResult { layer, count })
    })
    .await
}

#[tauri::command]
pub async fn delete_rows(engine: Eng<'_>, id: String, fids: Vec<i64>) -> Res<EditResult> {
    blocking(&engine, move |e| {
        let (layer, count) = e.delete_rows(&id, &fids)?;
        Ok(EditResult { layer, count })
    })
    .await
}

// ---------- sample data ----------

#[tauri::command]
pub async fn tiger_datasets() -> Res<Vec<engine::tiger::Dataset>> {
    Ok(engine::tiger::DATASETS.to_vec())
}

#[tauri::command]
pub async fn tiger_vintages(engine: Eng<'_>) -> Res<Vec<engine::tiger::Vintage>> {
    blocking(&engine, |_| engine::tiger::vintages()).await
}

#[tauri::command]
pub async fn tiger_availability(engine: Eng<'_>, vintage: String) -> Res<Vec<(String, bool)>> {
    blocking(&engine, move |_| engine::tiger::availability(&vintage)).await
}

#[tauri::command]
pub async fn tiger_counties(engine: Eng<'_>, state: String, vintage: String) -> Res<Vec<(String, String)>> {
    blocking(&engine, move |_| engine::tiger::counties(&state, &vintage)).await
}

#[tauri::command]
pub async fn tiger_import(engine: Eng<'_>, datasets: Vec<String>, vintage: String, state: String, county: Option<String>, label: String) -> Res<Vec<LayerInfo>> {
    let out = blocking(&engine, move |e| e.tiger_import(&datasets, &vintage, &state, county.as_deref(), &label)).await?;
    prewarm(&engine, out.iter().map(|l| l.id.clone()).collect());
    Ok(out)
}

#[tauri::command]
pub async fn tnm_datasets() -> Res<Vec<engine::tnm_vector::Dataset>> {
    Ok(engine::tnm_vector::DATASETS.to_vec())
}

#[tauri::command]
pub async fn tnm_import(engine: Eng<'_>, datasets: Vec<String>, bbox: [f64; 4], clip_layer: Option<String>, clip: bool, label: String) -> Res<engine::TnmImport> {
    let out = blocking(&engine, move |e| e.tnm_import(&datasets, bbox, clip_layer.as_deref(), clip, &label)).await?;
    prewarm(&engine, out.layers.iter().map(|l| l.id.clone()).collect());
    Ok(out)
}

/// Running long tasks, for progress bars.
#[tauri::command]
pub async fn progress() -> Res<Vec<engine::progress::TaskInfo>> {
    Ok(engine::progress::snapshot())
}

#[tauri::command]
pub async fn cancel_task(id: u64) -> Res<()> {
    engine::progress::cancel(id);
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn select_by_location(engine: Eng<'_>, target: String, by: String, by_fids: Option<Vec<i64>>, relation: String, distance: f64, units: String, invert: bool) -> Res<Vec<i64>> {
    blocking(&engine, move |e| e.select_by_location(&target, &by, by_fids.as_deref(), &relation, distance, &units, invert)).await
}

// ---------- file manager ----------

#[tauri::command]
pub async fn fs_roots() -> Res<Vec<engine::catalog::Root>> {
    Ok(engine::catalog::roots())
}

#[tauri::command]
pub async fn fs_list(engine: Eng<'_>, path: String) -> Res<engine::catalog::Listing> {
    blocking(&engine, move |_| engine::catalog::list(&path)).await
}

#[tauri::command]
pub async fn fs_stat(paths: Vec<String>) -> Res<Vec<engine::catalog::PathState>> {
    Ok(engine::catalog::stat(&paths))
}

#[tauri::command]
pub async fn fs_inspect(engine: Eng<'_>, path: String) -> Res<engine::catalog::Details> {
    blocking(&engine, move |_| engine::catalog::inspect(&path)).await
}

// ---------- sources & citations ----------

#[tauri::command]
pub async fn sources(engine: Eng<'_>) -> Res<Vec<engine::sources::SourceEntry>> {
    blocking(&engine, |e| e.sources()).await
}

#[tauri::command]
pub async fn update_source(engine: Eng<'_>, citation: engine::sources::Citation) -> Res<()> {
    blocking(&engine, move |e| e.update_source(&citation)).await
}

#[tauri::command]
pub async fn remove_source(engine: Eng<'_>, key: String) -> Res<()> {
    blocking(&engine, move |e| e.remove_source(&key)).await
}

/// Log the basemap in use as a data source.
#[tauri::command]
pub async fn record_basemap(engine: Eng<'_>, id: String) -> Res<()> {
    blocking(&engine, move |e| match engine::sources::for_basemap(&id) {
        Some(c) => e.record_source(&c, "Basemap"),
        None => Ok(()),
    })
    .await
}

/// Write a text file chosen in a save dialog (bibliography exports).
#[tauri::command]
pub async fn save_text(path: String, text: String) -> Res<()> {
    let p = std::path::Path::new(&path);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    }
    std::fs::write(p, text).map_err(|e| format!("Could not save {path}: {e}"))
}

// ---------- rasters ----------

#[tauri::command]
pub async fn plan_elevation(engine: Eng<'_>, bbox: [f64; 4], resolution: f64) -> Res<engine::tnm::ElevationPlan> {
    blocking(&engine, move |e| e.plan_elevation(bbox, resolution)).await
}

#[tauri::command]
pub async fn download_elevation(engine: Eng<'_>, bbox: [f64; 4], resolution: f64, label: String, confirmed: Option<bool>) -> Res<LayerInfo> {
    let info = blocking(&engine, move |e| e.download_elevation(bbox, resolution, &label, confirmed.unwrap_or(false))).await?;
    prewarm(&engine, vec![info.id.clone()]);
    Ok(info)
}

#[tauri::command]
pub async fn raster_value(engine: Eng<'_>, id: String, lon: f64, lat: f64) -> Res<Option<f64>> {
    blocking(&engine, move |e| e.raster_value(&id, lon, lat)).await
}

// ---------- measuring ----------

#[tauri::command]
pub async fn measure_feature(engine: Eng<'_>, id: String, fid: i64) -> Res<engine::table::Measures> {
    blocking(&engine, move |e| e.measure_feature(&id, fid)).await
}

#[derive(serde::Deserialize)]
pub struct NewFeature {
    wkt: String,
    #[serde(default)]
    props: serde_json::Map<String, serde_json::Value>,
}

#[tauri::command]
pub async fn create_features(engine: Eng<'_>, name: String, features: Vec<NewFeature>) -> Res<LayerInfo> {
    blocking(&engine, move |e| e.create_features(&name, &features.into_iter().map(|f| (f.wkt, f.props)).collect::<Vec<_>>())).await
}

// ---------- layouts ----------

#[tauri::command]
pub async fn list_layouts(engine: Eng<'_>) -> Res<Vec<engine::layout::model::Layout>> {
    blocking(&engine, |e| e.list_layouts()).await
}

#[tauri::command]
pub async fn save_layout(engine: Eng<'_>, layout: engine::layout::model::Layout) -> Res<()> {
    blocking(&engine, move |e| e.save_layout(&layout)).await
}

/// Layout templates, bundled and the user's (C3.1).
#[tauri::command]
pub async fn list_layout_templates(engine: Eng<'_>) -> Res<engine::layout::TemplateList> {
    blocking(&engine, |e| Ok(e.layout_templates())).await
}

#[tauri::command]
pub async fn get_layout_template(engine: Eng<'_>, id: String) -> Res<engine::layout::model::Layout> {
    blocking(&engine, move |e| e.layout_template(&id)).await
}

#[tauri::command]
pub async fn save_layout_template(engine: Eng<'_>, name: String, layout: engine::layout::model::Layout) -> Res<engine::layout::templates::TemplateInfo> {
    blocking(&engine, move |e| e.save_layout_template(&name, &layout)).await
}

#[tauri::command]
pub async fn delete_layout_template(engine: Eng<'_>, id: String) -> Res<()> {
    blocking(&engine, move |e| e.delete_layout_template(&id)).await
}

#[tauri::command]
pub async fn delete_layout(engine: Eng<'_>, id: String) -> Res<()> {
    blocking(&engine, move |e| e.delete_layout(&id)).await
}

#[tauri::command]
pub async fn render_layout_preview(engine: Eng<'_>, req: engine::layout::model::RenderRequest, dpi: f64) -> Res<engine::layout::LayoutPreview> {
    blocking(&engine, move |e| e.render_layout_preview(&req, dpi)).await
}

/// The pages of the layout's map series (C3.2).
#[tauri::command]
pub async fn layout_series_pages(engine: Eng<'_>, layout: engine::layout::model::Layout) -> Res<Vec<engine::layout::series::SeriesPage>> {
    blocking(&engine, move |e| e.series_pages(&layout)).await
}

/// Export the whole series: one multi-page PDF at `path`, or one PNG per page in the folder `path`.
#[tauri::command]
pub async fn export_layout_series(engine: Eng<'_>, req: engine::layout::model::RenderRequest, format: String, dpi: f64, path: PathBuf) -> Res<engine::layout::series::SeriesExport> {
    blocking(&engine, move |e| e.export_series(&req, engine::layout::ExportFormat::parse(&format)?, dpi, &path)).await
}

/// Returns the render's warnings.
#[tauri::command]
pub async fn export_layout(engine: Eng<'_>, req: engine::layout::model::RenderRequest, format: String, dpi: f64, path: PathBuf) -> Res<Vec<String>> {
    blocking(&engine, move |e| e.export_layout(&req, engine::layout::ExportFormat::parse(&format)?, dpi, &path)).await
}

#[tauri::command]
pub async fn arcgis_catalog() -> Res<Vec<engine::arcgis::Entry>> {
    Ok(engine::arcgis::catalog().to_vec())
}

#[tauri::command]
pub async fn arcgis_layers(url: String) -> Res<Vec<engine::arcgis::ServiceLayer>> {
    tauri::async_runtime::spawn_blocking(move || engine::arcgis::describe_service(&url)).await.map_err(|e| e.to_string())?.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn arcgis_import(engine: Eng<'_>, spec: engine::arcgis::Spec, bbox: [f64; 4], label: String) -> Res<Vec<LayerInfo>> {
    let layers = blocking(&engine, move |e| e.arcgis_import(&spec, bbox, &label)).await?;
    prewarm(&engine, layers.iter().map(|l| l.id.clone()).collect());
    Ok(layers)
}

#[tauri::command]
pub async fn census_datasets() -> Res<Vec<engine::census::Dataset>> {
    Ok(engine::census::DATASETS.to_vec())
}

#[tauri::command]
pub async fn census_key_status() -> Res<bool> {
    Ok(crate::secrets::has(crate::secrets::CENSUS))
}

#[tauri::command]
pub async fn census_key_set(key: String) -> Res<()> {
    crate::secrets::set(crate::secrets::CENSUS, &key)
}

#[tauri::command]
pub async fn census_key_delete() -> Res<()> {
    crate::secrets::delete(crate::secrets::CENSUS)
}

#[tauri::command]
pub async fn census_join(engine: Eng<'_>, layer: String, geoid_field: String, spec: engine::census::Spec) -> Res<engine::census::JoinResult> {
    let key = crate::secrets::get(crate::secrets::CENSUS);
    let r = blocking(&engine, move |e| e.census_join(&layer, &geoid_field, &spec, key.as_deref())).await?;
    prewarm(&engine, vec![r.layer.id.clone()]);
    Ok(r)
}

#[tauri::command]
pub async fn stac_catalog() -> Res<Vec<engine::stac::Api>> {
    Ok(engine::stac::catalog().to_vec())
}

#[tauri::command]
pub async fn stac_search(spec: engine::stac::SearchSpec) -> Res<Vec<engine::stac::Item>> {
    tauri::async_runtime::spawn_blocking(move || engine::stac::search(&spec)).await.map_err(|e| e.to_string())?.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn stac_import(engine: Eng<'_>, spec: engine::stac::ImportSpec) -> Res<LayerInfo> {
    let info = blocking(&engine, move |e| e.stac_import(&spec)).await?;
    prewarm(&engine, vec![info.id.clone()]);
    Ok(info)
}
