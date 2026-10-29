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
