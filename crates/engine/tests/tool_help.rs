//! Tool help (UX12): the before-and-after picture is the tool's own result on a sample, and a tool the sample cannot serve says why.

use engine::Engine;

#[test]
fn ux12_which_tools_get_a_picture() {
    let e = Engine::new().unwrap();
    let mut with = Vec::new();
    let mut without = Vec::new();
    for s in e.tool_specs() {
        let h = e.tool_help(s.spec.id).unwrap();
        match (&h.picture, &h.picture_note) {
            (Some(_), None) => with.push(s.spec.id),
            (None, Some(n)) => without.push(format!("{}: {n}", s.spec.id)),
            other => panic!("{}: picture and note must be one or the other: {other:?}", s.spec.id),
        }
    }
    eprintln!("WITH PICTURE ({}): {with:?}", with.len());
    eprintln!("WITHOUT ({}):\n{}", without.len(), without.join("\n"));
    for id in ["buffer", "clip", "erase", "intersect", "union", "simplify"] {
        assert!(with.contains(&id), "{id} should have a picture: {without:?}");
    }
}

#[test]
fn ux12_the_picture_is_the_real_result() {
    let e = Engine::new().unwrap();
    let h = e.tool_help("buffer").unwrap();
    let p = h.picture.expect("buffer has a picture");
    assert_eq!(p.before.iter().map(|l| l.role).collect::<Vec<_>>(), ["input"]);
    assert!(p.after.iter().any(|l| l.role == "result" && l.kind == "polygon" && !l.paths.is_empty()));
    assert!(p.after.iter().all(|l| l.paths.iter().all(|d| d.starts_with('M'))), "SVG path data: {:?}", p.after);
    assert!(h.citation.is_some_and(|c| c.text.contains("Karney")));
    // Clip uses the overlay: both inputs are shown before.
    let clip = e.tool_help("clip").unwrap().picture.unwrap();
    assert_eq!(clip.before.iter().map(|l| l.role).collect::<Vec<_>>(), ["input", "overlay"]);
    // Asking again gives the same answer, and the project is untouched.
    assert_eq!(serde_json::to_string(&e.tool_help("clip").unwrap().picture).unwrap(), serde_json::to_string(&Some(clip)).unwrap());
    assert!(e.list_layers().is_empty());
}

#[test]
fn ux12_tools_without_a_sample_say_why() {
    let e = Engine::new().unwrap();
    let raster = e.tool_help("hillshade").unwrap();
    assert!(raster.picture.is_none());
    assert!(raster.picture_note.unwrap().contains("raster"));
    assert!(e.tool_help("nope").is_err());
}
