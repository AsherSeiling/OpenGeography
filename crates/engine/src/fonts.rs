//! Fonts installed on this computer (macOS, Windows, Linux), for map labels and layouts.
//! The list comes from `fontdb`, the font database resvg already uses. Layout rendering still never loads *all* system fonts:
//! `layout::export` copies only the families a page names into its render database, so a missing font falls back to the
//! bundled Noto Sans instead of changing the whole page.

use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use resvg::usvg::fontdb::{self, Database, Family, Query, Stretch, Style, Weight};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FontFamily {
    pub name: String,
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
}

fn system() -> &'static Database {
    static DB: OnceLock<Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = Database::new();
        db.load_system_fonts();
        db
    })
}

/// Installed font families, sorted by name, with whether a bold and an italic face exist.
pub fn list() -> Vec<FontFamily> {
    let mut by: BTreeMap<String, FontFamily> = BTreeMap::new();
    for f in system().faces() {
        let Some((name, _)) = f.families.first() else { continue };
        // macOS keeps system UI faces under dot-names (".SF NS"); they are not meant for documents.
        if name.trim().is_empty() || name.starts_with('.') {
            continue;
        }
        let e = by.entry(name.clone()).or_insert_with(|| FontFamily { name: name.clone(), bold: false, italic: false, monospace: false });
        e.bold |= f.weight.0 >= 600;
        e.italic |= f.style != Style::Normal;
        e.monospace |= f.monospaced;
    }
    by.into_values().collect()
}

/// True when `family` is installed.
pub fn has_family(family: &str) -> bool {
    let want = family.trim().to_lowercase();
    !want.is_empty() && system().faces().any(|f| f.families.iter().any(|(n, _)| n.to_lowercase() == want))
}

/// Raw font files of every face of `family` (for the layout render database).
pub fn family_data(family: &str) -> Vec<Vec<u8>> {
    let want = family.trim().to_lowercase();
    let db = system();
    let mut seen: Vec<std::path::PathBuf> = Vec::new();
    let mut out = Vec::new();
    for f in db.faces() {
        if !f.families.iter().any(|(n, _)| n.to_lowercase() == want) {
            continue;
        }
        // Collections (.ttc) hold several faces in one file: load each file once.
        let key = match &f.source {
            fontdb::Source::File(p) => Some(p.clone()),
            _ => None,
        };
        if let Some(k) = &key {
            if seen.contains(k) {
                continue;
            }
            seen.push(k.clone());
        }
        db.with_face_data(f.id, |data, _| out.push(data.to_vec()));
    }
    out
}

type Cached = Arc<(Vec<u8>, u32)>;

fn face_bytes(family: &str, bold: bool, italic: bool) -> Option<Cached> {
    static CACHE: OnceLock<Mutex<HashMap<(String, bool, bool), Option<Cached>>>> = OnceLock::new();
    let mut c = CACHE.get_or_init(Default::default).lock().ok()?;
    c.entry((family.to_lowercase(), bold, italic))
        .or_insert_with(|| {
            let db = system();
            let fam = [Family::Name(family)];
            let q = Query { families: &fam, weight: if bold { Weight::BOLD } else { Weight::NORMAL }, stretch: Stretch::Normal, style: if italic { Style::Italic } else { Style::Normal } };
            let id = db.query(&q)?;
            db.with_face_data(id, |d, i| Arc::new((d.to_vec(), i)))
        })
        .clone()
}

/// Width in mm of `text` in an installed font (advance widths, no kerning); None when the font is not installed.
pub fn text_width(text: &str, family: &str, bold: bool, italic: bool, size_pt: f64) -> Option<f64> {
    let data = face_bytes(family, bold, italic)?;
    let face = ttf_parser::Face::parse(&data.0, data.1).ok()?;
    let upem = face.units_per_em() as f64;
    let units: f64 = text.chars().map(|c| face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g)).unwrap_or(face.units_per_em() / 2) as f64).sum();
    Some(units / upem * size_pt * 25.4 / 72.0)
}

/// A family name made safe to put inside an SVG attribute.
pub fn clean_name(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '\'' | '"' | '<' | '>' | '&' | ',' | ';' | '\\') && !c.is_control()).collect::<String>().trim().to_string()
}
