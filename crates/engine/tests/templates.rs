//! Layout templates (C3.1, docs/LAYOUTS_PLAN.md): P20 and the user's own templates.

use engine::layout::model::{Element, Layout, RenderRequest};
use engine::layout::templates::{self, BUILTIN};
use engine::Engine;
use serde_json::json;

fn request(layout: &Layout) -> RenderRequest {
    let mut v = json!({ "layout": layout, "styles": {}, "order": [], "visible": {}, "project": "Demo", "date": "2026-01-01" });
    // A template's frames have no CRS or centre yet: fill them like "New Layout from template" does.
    for e in v["layout"]["elements"].as_array_mut().unwrap() {
        if e["kind"] == "map_frame" {
            e["crs"] = json!("EPSG:26915");
            e["center"] = json!([500000.0, 4649776.0]);
            e["scale"] = json!(50000);
        }
    }
    serde_json::from_value(v).unwrap()
}

#[test]
fn p20_every_bundled_template_validates_and_renders() {
    let e = Engine::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (list, skipped) = templates::list(dir.path());
    assert!(skipped.is_empty());
    assert_eq!(list.len(), BUILTIN.len());
    for t in &list {
        let l = templates::load(&t.id, dir.path()).unwrap_or_else(|err| panic!("{}: {err}", t.id));
        l.validate().unwrap_or_else(|err| panic!("{}: {err}", t.id));
        // The same six elements everywhere, in a frame that fits the page.
        let kinds: Vec<&str> = l.elements.iter().map(|x| x.kind()).collect();
        assert_eq!(kinds, ["text", "map_frame", "legend", "north_arrow", "scale_bar", "sources"], "{}", t.id);
        let Element::MapFrame { at, .. } = &l.elements[1] else { unreachable!() };
        assert!(at.x >= 0.0 && at.y >= 0.0 && at.x + at.w <= l.page.width + 1e-6 && at.y + at.h <= l.page.height + 1e-6, "{} frame inside the page", t.id);
        for el in &l.elements {
            let a = el.at();
            assert!(a.x >= -1e-6 && a.y >= -1e-6 && a.x + a.w <= l.page.width + 1e-6 && a.y + a.h <= l.page.height + 1e-6, "{}: {} inside the page", t.id, a.id);
        }
        let out = e.render_layout_svg(&request(&l), &|| Ok(())).unwrap_or_else(|err| panic!("{} did not render: {err}", t.id));
        assert!(out.svg.starts_with("<svg"), "{}", t.id);
        assert!(out.svg.contains(&format!("width=\"{}mm\"", engine::layout::svg::mm(l.page.width))), "{}", t.id);
        // Title token, scale bar ratio and north arrow are drawn.
        assert!(out.svg.contains("1:50,000"), "{}: the scale bar's ratio", t.id);
        assert!(!out.warnings.iter().any(|w| w.contains("too narrow") || w.contains("too short")), "{}: {:?}", t.id, out.warnings);
    }
}

#[test]
fn bundled_templates_are_unique_named_and_ordered() {
    let dir = tempfile::tempdir().unwrap();
    let (list, _) = templates::list(dir.path());
    let ids: std::collections::BTreeSet<_> = list.iter().map(|t| t.id.clone()).collect();
    let names: std::collections::BTreeSet<_> = list.iter().map(|t| t.name.clone()).collect();
    assert_eq!((ids.len(), names.len()), (10, 10));
    assert!(list.iter().all(|t| t.builtin && t.id.starts_with("builtin:")));
    assert_eq!(list[0].name, "Map — Letter landscape");
    assert!((list[0].width - 279.4).abs() < 0.01 && (list[0].height - 215.9).abs() < 0.01);
    assert!((list[1].width - 215.9).abs() < 0.01, "portrait follows landscape");
}

#[test]
fn user_templates_save_list_load_replace_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let mut l = templates::load("builtin:a4-portrait", dir.path()).unwrap();
    l.name = "something else".into();
    let saved = templates::save("  County map / draft ", &l, dir.path()).unwrap();
    assert_eq!(saved.id, "user:County map _ draft");
    assert_eq!(saved.name, "County map / draft", "the name is kept as typed (trimmed); only the file name is made safe");
    assert!(!saved.builtin);

    let (list, skipped) = templates::list(dir.path());
    assert!(skipped.is_empty());
    assert_eq!(list.len(), 11);
    assert_eq!(list.last().unwrap(), &saved, "the user's templates follow the bundled ones");

    let back = templates::load(&saved.id, dir.path()).unwrap();
    assert_eq!(back.name, "County map / draft");
    assert_eq!(back.elements.len(), 6);

    // The same name again replaces it.
    l.page.width += 10.0;
    templates::save("County map / draft", &l, dir.path()).unwrap();
    assert_eq!(templates::list(dir.path()).0.len(), 11);
    assert!((templates::load(&saved.id, dir.path()).unwrap().page.width - l.page.width).abs() < 1e-9);

    templates::delete(&saved.id, dir.path()).unwrap();
    assert_eq!(templates::list(dir.path()).0.len(), 10);
    assert!(templates::load(&saved.id, dir.path()).is_err());
    assert!(templates::delete(&saved.id, dir.path()).unwrap_err().to_string().contains("no template"));
}

#[test]
fn templates_refuse_what_they_cannot_keep_and_stay_inside_their_folder() {
    let dir = tempfile::tempdir().unwrap();
    let l = templates::load("builtin:letter-landscape", dir.path()).unwrap();
    assert!(templates::save("   ", &l, dir.path()).unwrap_err().to_string().contains("needs a name"));
    let mut bad = l.clone();
    bad.page.width = -1.0;
    assert!(templates::save("bad", &bad, dir.path()).is_err(), "an invalid layout is not saved");
    assert_eq!(templates::list(dir.path()).0.len(), 10);
    assert!(templates::delete("builtin:letter-landscape", dir.path()).unwrap_err().to_string().contains("can't be deleted"));
    for id in ["user:../escape", "user:a/b", "user:..", "nonsense", "builtin:nope", "user:"] {
        assert!(templates::load(id, dir.path()).is_err(), "{id}");
        assert!(templates::delete(id, dir.path()).is_err(), "{id}");
    }
    // A name made only of unsafe characters still gets a usable file.
    assert_eq!(templates::file_stem("///"), "___");
    assert_eq!(templates::file_stem(""), "template");
}

#[test]
fn a_broken_user_file_is_skipped_and_reported_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("broken.json"), "{ not json").unwrap();
    std::fs::write(dir.path().join("note.txt"), "ignored").unwrap();
    let (list, skipped) = templates::list(dir.path());
    assert_eq!(list.len(), 10);
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].starts_with("broken.json"), "{skipped:?}");
}

#[test]
fn the_engine_keeps_user_templates_in_the_per_user_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("GIS_STUDIO_DATA", dir.path());
    let e = Engine::new().unwrap();
    let l = e.layout_template("builtin:a3-landscape").unwrap();
    let t = e.save_layout_template("My A3", &l).unwrap();
    assert!(dir.path().join("layout_templates").join("My A3.json").is_file());
    assert!(e.layout_templates().templates.iter().any(|x| x.id == t.id));
    e.delete_layout_template(&t.id).unwrap();
    assert!(!e.layout_templates().templates.iter().any(|x| x.id == t.id));
    std::env::remove_var("GIS_STUDIO_DATA");
}
