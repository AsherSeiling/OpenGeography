//! UR-13: a citation the user writes (form or pasted text) is attached to a layer; layers made from it inherit it.

use engine::sources::{Citation, PASTED_KIND};
use engine::Engine;

fn uses_of(e: &Engine, layer: &str) -> Vec<String> {
    e.sources().unwrap().into_iter().filter(|s| s.uses.iter().any(|u| u.layer_id == layer)).map(|s| s.citation.key).collect()
}

#[test]
fn a_layer_with_no_source_can_be_given_one_and_copies_inherit_it() {
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 5, [-100.0, 30.0, -95.0, 35.0]).unwrap();
    assert!(uses_of(&e, &l.id).is_empty(), "a generated layer has no citation");

    let form = Citation { title: "County parcels".into(), author: "Example County GIS".into(), year: "2025".into(), publisher: "Example County".into(), url: "https://example.org/parcels".into(), license: "CC BY 4.0".into(), ..Default::default() };
    let added = e.add_layer_source(&l.id, &form, Some("2026-03-04")).unwrap();
    assert!(added.key.starts_with("user:") && added.complete && added.kind == "Dataset");
    let entry = e.sources().unwrap().into_iter().find(|s| s.citation.key == added.key).unwrap();
    assert_eq!(entry.first_used, "2026-03-04", "the accessed date is the one given");
    assert_eq!((entry.citation.author.as_str(), entry.citation.license.as_str()), ("Example County GIS", "CC BY 4.0"));
    assert_eq!(uses_of(&e, &l.id), vec![added.key.clone()]);

    // A copy made later inherits it.
    let copy = e.copy_layer(&l.id, "Copy", None).unwrap();
    assert_eq!(uses_of(&e, &copy.id), vec![added.key.clone()]);

    // Pasted text is kept whole and printed as it is.
    let l2 = e.generate_test_layer("polygons", 3, [-100.0, 30.0, -95.0, 35.0]).unwrap();
    let text = "Smith, J. (2020). Soil survey of Example County. U.S. Dept. of Agriculture. https://example.org/soil";
    let pasted = e.add_layer_source(&l2.id, &Citation { title: format!("  {text}  "), kind: PASTED_KIND.into(), ..Default::default() }, None).unwrap();
    assert_eq!(pasted.title, text);
    assert_ne!(pasted.key, added.key);
    let today = e.sources().unwrap().into_iter().find(|s| s.citation.key == pasted.key).unwrap().first_used;
    assert_eq!(today.len(), 10, "no date given: today's");
}

#[test]
fn an_empty_citation_or_a_bad_date_is_refused_in_words() {
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 3, [-100.0, 30.0, -95.0, 35.0]).unwrap();
    let err = e.add_layer_source(&l.id, &Citation::default(), None).unwrap_err().to_string();
    assert!(err.contains("title or an author"), "{err}");
    let err = e.add_layer_source(&l.id, &Citation { kind: PASTED_KIND.into(), title: "  ".into(), ..Default::default() }, None).unwrap_err().to_string();
    assert!(err.contains("Paste the citation text"), "{err}");
    let err = e.add_layer_source(&l.id, &Citation { title: "x".into(), ..Default::default() }, Some("last Tuesday")).unwrap_err().to_string();
    assert!(err.contains("year-month-day"), "{err}");
    assert!(e.sources().unwrap().is_empty(), "nothing was recorded by the refusals");
    assert!(e.add_layer_source("nope", &Citation { title: "x".into(), ..Default::default() }, None).is_err());
}
