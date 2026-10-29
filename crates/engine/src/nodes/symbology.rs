use super::*;

/// Styling node. The data passes through unchanged; the UI reads this node's settings after a run and applies them
/// to the layer the tree belongs to (line/outline colour and width, index-contour colour, labels). Pairs with Contour:
/// the Contour node's INDEX = 1 lines get their own colour and the ELEV field can be labelled on them only.
/// Label settings follow QGIS layer labeling "Text", "Buffer" (halo) and "Rendering" groups; settings only, no QGIS code.
pub struct Symbology;

impl Tool for Symbology {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "symbology",
            name: "Symbology and Labels",
            category: "Styling",
            role: Role::Tool,
            description: "Set how the layer is drawn: colours, line width and labels (e.g. index contours coloured and labelled with their elevation). The data is not changed.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Vector }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("fill", "Fill / line colour", ParamKind::Color).default("#a67c52"),
                Param::new("stroke", "Outline colour", ParamKind::Color).default("#a67c52").help("Outline of polygons and points"),
                Param::new("width", "Line width (px)", ParamKind::Number).default(0.8),
                Param::new("opacity", "Opacity (0–1)", ParamKind::Number).default(1),
                Param::new("index_color", "Index lines colour", ParamKind::Color).optional().help("Features with INDEX = 1 (the Contour node's index lines) are drawn in this colour; leave empty for no special colour"),
                Param::new("label", "Show labels", ParamKind::Bool).default(false),
                Param::new("label_field", "Label field", ParamKind::Field).field_of("in").optional().help("For contours choose ELEV"),
                Param::new("label_index_only", "Label index lines only", ParamKind::Bool).default(false).help("Only features with INDEX = 1 get a label"),
                Param::new("contour_spacing", "Contour labels every (px)", ParamKind::Number).optional().help("Repeat the label along each line every this many pixels, upright (contour labels); empty = one label per line"),
                Param::new("contour_break", "Break the line under the label", ParamKind::Bool).default(true),
                Param::new("contour_decimals", "Elevation decimals", ParamKind::Number).default(0),
                Param::new("contour_unit", "Unit after the value", ParamKind::Text).optional().help("For example \" m\" or \" ft\""),
                Param::new("label_size", "Label size (pt)", ParamKind::Number).default(9),
                Param::new("label_color", "Label colour", ParamKind::Color).default("#5c3d1e"),
                Param::new("halo_color", "Halo colour", ParamKind::Color).default("#ffffff"),
                Param::new("halo_width", "Halo width (px)", ParamKind::Number).default(1.5),
            ],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        if p.bool("label") && p.str("label_field").is_none() {
            bail!("Choose the field to label (for contours, ELEV), or turn off Show labels.");
        }
        for (k, label) in [("opacity", "Opacity")] {
            if let Some(v) = p.f64(k) {
                if !(0.0..=1.0).contains(&v) {
                    bail!("{label} must be between 0 and 1.");
                }
            }
        }
        for (k, label) in [("width", "Line width"), ("label_size", "Label size"), ("halo_width", "Halo width")] {
            if p.f64(k).is_some_and(|v| v < 0.0) {
                bail!("{label} can't be negative.");
            }
        }
        Ok(Output::Select { sql: format!("SELECT * FROM {}", ident(&i.table)), crs: i.crs.clone(), view: true })
    }
}
