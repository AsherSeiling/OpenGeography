//! Text slots (docs/LAYOUT_REWORK_PLAN.md §4.2): every piece of text an element draws has a slot id like `legend.title`, a label for the
//! Properties pane, a default size and weight, and can be restyled through `Placement.text` (size, font, bold, italic, underline, colour,
//! letter spacing, halo). This table is the one authority: the renderers take their default sizes from it, the UI builds its Text section
//! from it (`layout_text_slots`), and a guard test fails if a slot has no effect on the drawing or an element kind has no slot.

use serde::Serialize;

pub const GRID_LABEL_PT: f64 = 6.0;
pub const BAR_LABEL_PT: f64 = 7.0;
pub const LEGEND_PT: f64 = 8.0;
pub const TITLE_PT: f64 = 10.0;
pub const TABLE_PT: f64 = 7.0;
pub const SOURCES_PT: f64 = 6.0;
/// North-arrow letters size themselves from the box; 0 = automatic in the table below.
pub const AUTO: f64 = 0.0;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    /// Element kind as in `Element`'s serde tag.
    pub kind: &'static str,
    /// Full slot id, `<kind>.<name>`.
    pub id: &'static str,
    pub label: &'static str,
    /// Default size in points; 0 = automatic (derived from the element's box).
    pub default_pt: f64,
    pub bold: bool,
    /// The element has its own size field that sets this slot's base size (`size`, `textSize`), shown as-is in the UI.
    pub legacy_size: bool,
}

const fn s(kind: &'static str, id: &'static str, label: &'static str, default_pt: f64, bold: bool, legacy_size: bool) -> Slot {
    Slot { kind, id, label, default_pt, bold, legacy_size }
}

/// Every text slot of every element kind.
pub const SLOTS: &[Slot] = &[
    s("text", "text.text", "Text", 10.0, false, true),
    s("map_frame", "map_frame.graticule_labels", "Graticule labels", GRID_LABEL_PT, false, false),
    s("map_frame", "map_frame.grid_labels", "Grid labels", GRID_LABEL_PT, false, false),
    s("legend", "legend.title", "Title", TITLE_PT, true, false),
    s("legend", "legend.headings", "Layer headings", LEGEND_PT, true, false),
    s("legend", "legend.entries", "Entries", LEGEND_PT, false, true),
    s("scale_bar", "scale_bar.numbers", "Distances and units", BAR_LABEL_PT, false, false),
    s("scale_bar", "scale_bar.notes", "Ratio and verbal scale", BAR_LABEL_PT, false, false),
    s("north_arrow", "north_arrow.letters", "N, S, E and W letters", AUTO, true, false),
    s("declination", "declination.text", "Letters and caption", super::declination::DECLINATION_PT, false, false),
    s("table", "table.header", "Header", TABLE_PT, true, true),
    s("table", "table.cells", "Cells", TABLE_PT, false, true),
    s("chart", "chart.title", "Title", TABLE_PT * 1.2, true, true),
    s("chart", "chart.labels", "Labels", TABLE_PT, false, true),
    s("sources", "sources.text", "Text", SOURCES_PT, false, true),
];

/// The slots of one element kind.
pub fn of_kind(kind: &str) -> Vec<&'static Slot> {
    SLOTS.iter().filter(|s| s.kind == kind).collect()
}

/// Element kinds that draw no text of their own (shapes, pictures, overview maps draw through map frames or have none).
pub const NO_TEXT: &[&str] = &["shape", "picture"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_ids_are_unique_and_match_their_kind() {
        for (i, a) in SLOTS.iter().enumerate() {
            assert!(a.id.starts_with(&format!("{}.", a.kind)), "{}", a.id);
            assert!(SLOTS[i + 1..].iter().all(|b| b.id != a.id), "duplicate {}", a.id);
            assert!(a.default_pt >= 0.0 && a.default_pt < 100.0, "{}", a.id);
        }
    }
}
