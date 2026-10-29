//! Layout themes: a coordinated look for the page (background, frame border,
//! text colour, legend box, scale bar and north arrow style) plus a palette
//! for single-symbol layers. One definition here; the UI reads it through
//! `layout_themes` and never keeps its own list. Colours are our own choices;
//! the thematic palette is Okabe–Ito (colour-blind-safe), the topographic one
//! follows the muted greens, tans and blues of printed topographic maps.

use super::model::{Border, Decor, Element, Layout};
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColor {
    pub fill: &'static str,
    pub stroke: &'static str,
    pub stroke_width: f64,
    pub opacity: f64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub page_background: &'static str,
    /// Map frame border; None = no border.
    pub frame_border: Option<(f64, &'static str)>,
    pub text_color: &'static str,
    pub legend_background: Option<&'static str>,
    pub legend_frame: Option<(f64, &'static str)>,
    pub legend_shadow: bool,
    pub scale_bar_style: &'static str,
    pub north_arrow_style: &'static str,
    /// For single-symbol vector layers, in draw order, cycling.
    pub palette: Vec<ThemeColor>,
}

fn c(fill: &'static str, stroke: &'static str, w: f64, o: f64) -> ThemeColor {
    ThemeColor { fill, stroke, stroke_width: w, opacity: o }
}

pub fn themes() -> Vec<Theme> {
    vec![
        Theme {
            id: "topographic",
            name: "Topographic",
            description: "Warm paper, muted greens, tans and blues, black frame, alternating scale bar and a half-arrow north.",
            page_background: "#fbf9f2",
            frame_border: Some((0.5, "#1a1a1a")),
            text_color: "#1a1a1a",
            legend_background: Some("#fbf9f2"),
            legend_frame: Some((0.25, "#4d4d4d")),
            legend_shadow: false,
            scale_bar_style: "alternating",
            north_arrow_style: "half",
            palette: vec![c("#cfdcb4", "#6b7a4f", 0.5, 0.9), c("#ead9b4", "#8a7a55", 0.5, 0.9), c("#b3d1e6", "#4f7a99", 0.6, 0.9), c("#e3b99a", "#8a5a3a", 0.5, 0.9), c("#d9d4c3", "#6b6552", 0.5, 0.9)],
        },
        Theme {
            id: "thematic",
            name: "Thematic",
            description: "White page, colour-blind-safe Okabe–Ito fills, thin grey frame, legend with a soft shadow, line scale bar.",
            page_background: "#ffffff",
            frame_border: Some((0.35, "#555555")),
            text_color: "#1f2a44",
            legend_background: Some("#ffffff"),
            legend_frame: Some((0.25, "#999999")),
            legend_shadow: true,
            scale_bar_style: "line",
            north_arrow_style: "simple",
            palette: vec![c("#0072b2", "#ffffff", 0.4, 0.85), c("#e69f00", "#ffffff", 0.4, 0.85), c("#009e73", "#ffffff", 0.4, 0.85), c("#cc79a7", "#ffffff", 0.4, 0.85), c("#56b4e9", "#ffffff", 0.4, 0.85), c("#d55e00", "#ffffff", 0.4, 0.85), c("#f0e442", "#999999", 0.4, 0.85)],
        },
        Theme {
            id: "minimalist",
            name: "Minimalist",
            description: "No frame, greys with white outlines, no legend box, triangle north arrow and a plain line scale bar.",
            page_background: "#ffffff",
            frame_border: None,
            text_color: "#333333",
            legend_background: None,
            legend_frame: None,
            legend_shadow: false,
            scale_bar_style: "line",
            north_arrow_style: "triangle",
            palette: vec![c("#d9d9d9", "#ffffff", 0.6, 1.0), c("#bdbdbd", "#ffffff", 0.6, 1.0), c("#969696", "#ffffff", 0.6, 1.0), c("#737373", "#ffffff", 0.6, 1.0), c("#525252", "#ffffff", 0.6, 1.0)],
        },
    ]
}

/// The layout with the theme's page look applied. Layer colours are applied by the caller from `palette`.
pub fn apply(layout: &Layout, id: &str) -> Result<Layout> {
    let t = themes().into_iter().find(|t| t.id == id).ok_or_else(|| anyhow!("Unknown theme '{id}' (topographic, thematic or minimalist)"))?;
    let mut out = layout.clone();
    out.page.background = t.page_background.to_string();
    for e in &mut out.elements {
        match e {
            Element::MapFrame { border, .. } => *border = t.frame_border.map(|(width, color)| Border { width, color: color.to_string() }),
            Element::Text { color, .. } => *color = t.text_color.to_string(),
            Element::Legend { at, .. } => {
                at.decor = if t.legend_background.is_some() || t.legend_frame.is_some() || t.legend_shadow {
                    Some(Decor {
                        background: t.legend_background.map(String::from),
                        frame_color: t.legend_frame.map(|f| f.1.to_string()),
                        frame_width: t.legend_frame.map_or(0.0, |f| f.0),
                        shadow: t.legend_shadow,
                        shadow_offset: 1.2,
                        corner_radius: 0.0,
                    })
                } else {
                    None
                }
            }
            Element::ScaleBar { style, color, .. } => {
                *style = t.scale_bar_style.to_string();
                *color = t.text_color.to_string();
            }
            Element::NorthArrow { style, color, .. } => {
                if style != "custom" {
                    *style = t.north_arrow_style.to_string();
                    *color = t.text_color.to_string();
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Layout {
        Layout::from_json(
            r##"{"id":"L","name":"T","version":1,"page":{"width":200,"height":150},"elements":[
              {"kind":"map_frame","id":"f1","x":10,"y":10,"w":100,"h":100,"crs":"EPSG:26915","center":[0,0],"scale":1000,"border":{"width":1,"color":"#ff0000"}},
              {"kind":"text","id":"t","x":0,"y":0,"w":50,"h":10,"text":"x","color":"#ff0000"},
              {"kind":"legend","id":"lg","frame":"f1","x":0,"y":0,"w":40,"h":40},
              {"kind":"scale_bar","id":"sb","frame":"f1","x":0,"y":0,"w":40,"h":10},
              {"kind":"north_arrow","id":"na","frame":"f1","x":0,"y":0,"w":10,"h":15},
              {"kind":"north_arrow","id":"nc","frame":"f1","x":0,"y":0,"w":10,"h":15,"style":"custom"}]}"##,
        )
        .unwrap()
    }

    #[test]
    fn themes_have_distinct_looks_and_safe_palettes() {
        let all = themes();
        assert_eq!(all.iter().map(|t| t.id).collect::<Vec<_>>(), ["topographic", "thematic", "minimalist"]);
        assert!(all.iter().all(|t| t.palette.len() >= 5 && !t.description.is_empty()));
        let thematic = all.iter().find(|t| t.id == "thematic").unwrap();
        assert!(["#0072b2", "#e69f00", "#009e73"].iter().all(|c| thematic.palette.iter().any(|p| p.fill == *c)), "Okabe–Ito");
    }

    #[test]
    fn applying_a_theme_restyles_the_page_and_keeps_the_content() {
        let l = layout();
        let t = apply(&l, "minimalist").unwrap();
        assert_eq!(t.page.background, "#ffffff");
        assert_eq!(t.elements.len(), l.elements.len());
        for e in &t.elements {
            match e {
                Element::MapFrame { border, center, .. } => assert!(border.is_none() && *center == [0.0, 0.0]),
                Element::Text { color, text, .. } => assert!(color == "#333333" && text == "x"),
                Element::Legend { at, .. } => assert!(at.decor.is_none()),
                Element::ScaleBar { style, .. } => assert_eq!(style, "line"),
                Element::NorthArrow { at, style, .. } => assert!(if at.id == "nc" { style == "custom" } else { style == "triangle" }),
                _ => {}
            }
        }
        let t = apply(&l, "thematic").unwrap();
        let Element::Legend { at, .. } = &t.elements[2] else { panic!() };
        assert!(at.decor.as_ref().unwrap().shadow);
        let Element::MapFrame { border, .. } = &t.elements[0] else { panic!() };
        assert_eq!(border.as_ref().unwrap().color, "#555555");
        t.validate().unwrap();
        assert!(apply(&l, "neon").unwrap_err().to_string().contains("Unknown theme"));
    }
}
