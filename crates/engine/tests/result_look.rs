//! TP-11: each tool says how a raster it makes looks by default (`ToolSpec` via `Tool::result_look`), and the UI reads it from the specs.
use engine::nodes::{PortKind, ResultLook};
use engine::Engine;

#[test]
fn raster_tools_say_how_their_results_look() {
    let specs = Engine::new().unwrap().tool_specs();
    let look = |id: &str| specs.iter().find(|s| s.spec.id == id).unwrap_or_else(|| panic!("tool {id}")).result_look;
    for id in ["clip_raster", "mosaic_rasters", "resample_raster", "align_raster", "select_bands", "warp", "fill_depressions"] {
        assert_eq!(look(id), ResultLook::Source, "{id} keeps the source's renderer");
    }
    assert_eq!(look("hillshade"), ResultLook::Gray);
    for id in ["slope", "aspect", "flow_accumulation", "flow_direction", "extract_streams", "reclassify", "raster_calc", "raster_calc2", "spectral_index"] {
        assert_eq!(look(id), ResultLook::Derived, "{id} makes different values");
    }
    // Every raster-making tool has a look (Derived is the default) and it reaches the UI as camelCase.
    assert!(specs.iter().any(|s| s.spec.output == Some(PortKind::Raster)));
    let v = serde_json::to_value(specs.iter().find(|s| s.spec.id == "hillshade").unwrap()).unwrap();
    assert_eq!(v["resultLook"], "gray");
}
