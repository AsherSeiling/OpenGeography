//! Shapes, pictures and element decoration in layouts (docs/TEST_PLAN.md CG1).

use base64::Engine as _;
use engine::layout::export;
use engine::layout::model::RenderRequest;
use engine::Engine;
use serde_json::json;

fn png_data_uri() -> String {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 4, 4);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[200u8, 30, 30, 255].repeat(16)).unwrap();
    }
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out))
}

fn svg_data_uri() -> String {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><circle cx="5" cy="5" r="4" fill="#2060c0"/></svg>"##;
    format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(svg))
}

fn req(elements: serde_json::Value) -> RenderRequest {
    serde_json::from_value(json!({
        "layout": { "id": "L1", "name": "Shapes", "version": 1, "page": { "width": 200.0, "height": 120.0 }, "elements": elements },
        "order": [], "project": "Demo", "date": "2026-01-01"
    }))
    .unwrap()
}

fn render(e: &Engine, r: &RenderRequest) -> anyhow::Result<engine::layout::LayoutSvg> {
    e.render_layout_svg(r, &|| Ok(()))
}

#[test]
fn shapes_pictures_and_decor_render_to_svg_png_and_pdf() {
    let e = Engine::new().unwrap();
    let r = req(json!([
        { "kind": "shape", "id": "a", "shape": "rectangle", "x": 10.0, "y": 10.0, "w": 40.0, "h": 20.0, "fill": "#ffcc00", "stroke": "#333333", "strokeWidth": 0.6, "cornerRadius": 3.0,
          "decor": { "shadow": true, "shadowOffset": 1.5 } },
        { "kind": "shape", "id": "b", "shape": "ellipse", "x": 60.0, "y": 10.0, "w": 30.0, "h": 20.0, "fill": "#88ccee", "fillOpacity": 0.5 },
        { "kind": "shape", "id": "c", "shape": "arrow", "x": 100.0, "y": 20.0, "w": 40.0, "h": 1.0, "stroke": "#cc0000", "strokeWidth": 0.8, "dash": "dashed" },
        { "kind": "shape", "id": "d", "shape": "triangle", "x": 150.0, "y": 10.0, "w": 20.0, "h": 20.0, "rotation": 15.0 },
        { "kind": "picture", "id": "p", "x": 10.0, "y": 50.0, "w": 30.0, "h": 20.0, "source": png_data_uri(), "fit": "contain",
          "decor": { "background": "#eeeeee", "frameColor": "#000000", "frameWidth": 0.3 } },
        { "kind": "picture", "id": "q", "x": 60.0, "y": 50.0, "w": 30.0, "h": 20.0, "source": svg_data_uri(), "fit": "cover", "opacity": 0.8 },
        { "kind": "text", "id": "t", "x": 10.0, "y": 90.0, "w": 100.0, "h": 10.0, "text": "Boxed", "decor": { "background": "#ffffcc", "frameColor": "#999999", "frameWidth": 0.2 } }
    ]));
    let out = render(&e, &r).unwrap();
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    for id in ["shape-a", "shape-b", "shape-c", "shape-d", "picture-p", "picture-q", "decor-a", "decor-p", "decor-t"] {
        assert!(out.svg.contains(&format!("<g id=\"{id}\"")), "{id}");
    }
    assert!(out.svg.find("id=\"decor-a\"").unwrap() < out.svg.find("id=\"shape-a\"").unwrap(), "decor sits under its element");
    // Deterministic.
    assert_eq!(out.svg, render(&e, &r).unwrap().svg);
    // PNG and PDF export work, the PDF holds the raster picture.
    let preview = e.render_layout_preview(&r, 96.0).unwrap();
    assert!(!preview.png.is_empty());
    let pdf = export::svg_to_pdf(&out.svg).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    assert!(pdf.windows(14).any(|w| w == b"/Subtype /Imag"), "the PNG picture is embedded");
}

#[test]
fn bad_shape_or_picture_is_a_readable_error_and_empty_picture_warns() {
    let e = Engine::new().unwrap();
    let err = render(&e, &req(json!([{ "kind": "shape", "id": "a", "shape": "star", "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 }]))).unwrap_err();
    assert!(err.to_string().contains("Unknown shape"), "{err}");
    let err = render(&e, &req(json!([{ "kind": "picture", "id": "p", "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0, "source": "http://example.com/a.png" }]))).unwrap_err();
    assert!(err.to_string().contains("PNG, JPEG or SVG"), "{err}");
    let out = render(&e, &req(json!([{ "kind": "picture", "id": "p", "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 }]))).unwrap();
    assert!(out.warnings[0].contains("no image yet"));
}

// ---------------------------------------------------------------- CG2

fn with_frame(extra: serde_json::Value, rotation: f64) -> RenderRequest {
    let mut els = vec![json!({ "kind": "map_frame", "id": "f1", "x": 10.0, "y": 10.0, "w": 100.0, "h": 80.0, "rotation": rotation, "crs": "EPSG:26915", "center": [500000.0, 4649776.0], "scale": 24000, "layers": "map" })];
    els.extend(extra.as_array().cloned().unwrap());
    req(json!(els))
}

fn group<'a>(svg: &'a str, id: &str) -> &'a str {
    let start = svg.find(&format!("<g id=\"{id}\"")).unwrap_or_else(|| panic!("no group {id}"));
    let end = svg[start..].find("</g></g>").map_or(svg.len(), |e| e + start);
    &svg[start..end]
}

#[test]
fn cg2_every_north_arrow_style_draws_and_rotates() {
    let e = Engine::new().unwrap();
    for style in ["simple", "compass", "arrow", "triangle", "half", "rose", "circle"] {
        let out = render(&e, &with_frame(json!([{ "kind": "north_arrow", "id": "na", "frame": "f1", "x": 120.0, "y": 10.0, "w": 20.0, "h": 30.0, "style": style, "color": "#003366" }]), 0.0)).unwrap();
        let g = group(&out.svg, "northarrow-outer-na");
        assert!(g.contains("<polygon"), "{style}");
        assert!(style == "rose" || g.contains(">N<"), "{style} has an N");
        if matches!(style, "arrow" | "triangle" | "circle") {
            assert!(g.contains("#003366"), "{style} uses the colour");
        }
    }
    // Rose has all four cardinal letters.
    let out = render(&e, &with_frame(json!([{ "kind": "north_arrow", "id": "na", "frame": "f1", "x": 120.0, "y": 10.0, "w": 30.0, "h": 30.0, "style": "rose" }]), 0.0)).unwrap();
    let g = group(&out.svg, "northarrow-outer-na");
    for l in [">N<", ">S<", ">E<", ">W<"] {
        assert!(g.contains(l), "{l}");
    }
    // Grid north ignores convergence; true north does not. Frame rotation adds to both.
    let angle = |north: &str, rot: f64| -> f64 {
        let o = render(&e, &with_frame(json!([{ "kind": "north_arrow", "id": "na", "frame": "f1", "x": 120.0, "y": 10.0, "w": 20.0, "h": 30.0, "north": north }]), rot)).unwrap();
        let s = group(&o.svg, "northarrow-na");
        s.split("rotate(").nth(1).map_or(0.0, |t| t.split(' ').next().unwrap().parse().unwrap())
    };
    assert_eq!(angle("grid", 0.0), 0.0);
    assert_eq!(angle("grid", 25.0), 25.0);
    // Zone 15 central meridian is -93 (frame at -93): convergence is ~0 here, so true == grid at this fixture.
    assert!((angle("true", 0.0)).abs() < 0.2);
}

#[test]
fn cg2_custom_north_arrow_embeds_the_picture_and_needs_one() {
    let e = Engine::new().unwrap();
    let ok = render(&e, &with_frame(json!([{ "kind": "north_arrow", "id": "na", "frame": "f1", "x": 120.0, "y": 10.0, "w": 20.0, "h": 30.0, "style": "custom", "source": svg_data_uri() }]), 0.0)).unwrap();
    assert!(group(&ok.svg, "northarrow-outer-na").contains("<image"));
    assert!(ok.warnings.is_empty(), "{:?}", ok.warnings);
    let none = render(&e, &with_frame(json!([{ "kind": "north_arrow", "id": "na", "frame": "f1", "x": 120.0, "y": 10.0, "w": 20.0, "h": 30.0, "style": "custom" }]), 0.0)).unwrap();
    assert!(none.warnings.iter().any(|w| w.contains("custom style")));
    let err = render(&e, &with_frame(json!([{ "kind": "north_arrow", "id": "na", "frame": "f1", "x": 0.0, "y": 0.0, "w": 5.0, "h": 5.0, "style": "fancy" }]), 0.0)).unwrap_err();
    assert!(err.to_string().contains("Unknown north arrow style"), "{err}");
}

#[test]
fn cg2_scale_bar_styles_and_texts() {
    let e = Engine::new().unwrap();
    let bar = |extra: serde_json::Value| {
        let mut b = json!({ "kind": "scale_bar", "id": "sb", "frame": "f1", "x": 10.0, "y": 95.0, "w": 80.0, "h": 24.0, "units": "km", "segments": 4 });
        b.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let o = render(&e, &with_frame(json!([b]), 0.0)).unwrap();
        (group(&o.svg, "scalebar-sb").to_string(), o.warnings)
    };
    let (alt, _) = bar(json!({}));
    assert_eq!(alt.matches("<rect").count(), 4);
    assert_eq!(alt.matches("fill=\"#ffffff\"").count(), 2);
    let (hollow, _) = bar(json!({ "style": "hollow" }));
    assert_eq!(hollow.matches("fill=\"#ffffff\"").count(), 4);
    let (line, _) = bar(json!({ "style": "line" }));
    assert!(line.contains("<line") && !line.contains("<rect"));
    assert_eq!(line.matches("<line").count(), 6, "baseline + 5 ticks");
    let (ticks, _) = bar(json!({ "style": "ticks", "color": "#003366" }));
    assert!(ticks.contains("#003366"));
    // Double: kilometres above, miles below; the ground length is the same, so mile labels differ.
    let (dbl, w) = bar(json!({ "style": "double", "units2": "mi" }));
    assert!(dbl.contains(" km<") && dbl.contains(" mi<") && w.is_empty(), "{w:?}");
    assert_eq!(dbl.matches("<rect").count(), 8);
    // Verbal scale: 1:24,000 → 1 cm = 240 m.
    let (v, _) = bar(json!({ "verbal": true }));
    assert!(v.contains("1 cm on the map = 240 m"), "{v}");
    // Too short for the extra lines says so.
    let (_, w) = bar(json!({ "style": "double", "units2": "mi", "verbal": true, "ratio": true, "h": 10.0 }));
    assert!(!w.is_empty());
    let err = render(&e, &with_frame(json!([{ "kind": "scale_bar", "id": "sb", "frame": "f1", "x": 0.0, "y": 0.0, "w": 40.0, "h": 10.0, "style": "zigzag" }]), 0.0)).unwrap_err();
    assert!(err.to_string().contains("Unknown scale bar style"), "{err}");
}

#[test]
fn cg2_verbal_scale_wording() {
    use engine::layout::surround::verbal_scale;
    assert_eq!(verbal_scale(24_000.0, "km"), "1 cm on the map = 240 m");
    assert_eq!(verbal_scale(100_000.0, "km"), "1 cm on the map = 1 km");
    assert_eq!(verbal_scale(250_000.0, "mi"), "1 cm on the map = 1.55 mi");
    assert_eq!(verbal_scale(2_000.0, "mi"), "1 cm on the map = 65.62 ft");
    assert_eq!(verbal_scale(24_000.0, "mi"), "1 cm on the map = 787.4 ft");
}
