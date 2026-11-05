//! LE-1: named text styles are saved in the project. Both new fields are optional, so a layout saved before them opens and saves unchanged,
//! and a style reference to a style that is not there is only a link: the element keeps its look.
use engine::layout::model::{Element, Layout, NamedStyle};
use engine::Engine;
use serde_json::{json, Value};

fn old_layout() -> Layout {
    let raw = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layout_before_le1.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn reopened(layout: &Layout) -> Vec<Layout> {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("styles.gproj");
    let e = Engine::new().unwrap();
    e.save_layout(layout).unwrap();
    e.save_project(&proj, "{}").unwrap();
    drop(e);
    let e2 = Engine::new().unwrap();
    e2.open_project(&proj).unwrap();
    e2.list_layouts().unwrap()
}

#[test]
fn a_layout_saved_before_le1_opens_and_saves_with_nothing_lost_or_added() {
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layout_before_le1.json")).unwrap()).unwrap();
    let old = old_layout();
    assert!(old.named_styles.is_empty());
    assert!(old.elements.iter().all(|e| matches!(e, Element::Text { style_name: None, .. })));
    // Written back, no new key appears and every old key is still there.
    let written = serde_json::to_value(&old).unwrap();
    assert!(written.get("namedStyles").is_none());
    for el in written["elements"].as_array().unwrap() {
        assert!(el.get("styleName").is_none(), "{el}");
    }
    assert_eq!(written["elements"][0]["textStyles"], raw["elements"][0]["textStyles"]);
    assert_eq!(written["elements"][1]["family"], "Georgia");
    assert_eq!(written["guides"], raw["guides"]);
    // And it survives save, close, open.
    assert_eq!(reopened(&old), vec![old]);
}

#[test]
fn named_styles_and_a_style_reference_survive_save_close_open() {
    let mut l = old_layout();
    l.named_styles = vec![
        NamedStyle { name: "Title".into(), size: 21.0, bold: true, italic: false, color: Some("#000000".into()), family: None },
        NamedStyle { name: "Body".into(), size: 9.0, bold: false, italic: true, color: None, family: Some("Georgia".into()) },
    ];
    if let Element::Text { style_name, .. } = &mut l.elements[0] {
        *style_name = Some("Title".into());
    }
    let back = reopened(&l);
    assert_eq!(back, vec![l.clone()]);
    let v = serde_json::to_value(&back[0]).unwrap();
    assert_eq!(v["namedStyles"][0], json!({ "name": "Title", "size": 21.0, "bold": true, "color": "#000000" }));
    assert_eq!(v["elements"][0]["styleName"], "Title");
}

#[test]
fn a_reference_to_a_missing_style_keeps_the_look_and_raises_no_error() {
    let mut l = old_layout();
    if let Element::Text { style_name, .. } = &mut l.elements[1] {
        *style_name = Some("Deleted style".into());
    }
    let back = reopened(&l);
    assert_eq!(back, vec![l.clone()]);
    match &back[0].elements[1] {
        Element::Text { font, size, color, family, style_name, .. } => {
            assert_eq!((font.as_str(), *size, color.as_str(), family.as_deref()), ("sans", 9.0, "#333333", Some("Georgia")));
            assert_eq!(style_name.as_deref(), Some("Deleted style"));
        }
        _ => panic!("text"),
    }
}
