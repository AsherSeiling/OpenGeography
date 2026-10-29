//! SVG → PDF (vector, fonts embedded and subset) and PNG. Text uses the
//! bundled Noto Sans faces only: the font database never looks at the system,
//! so the same layout renders identically on every platform.

use anyhow::{anyhow, Result};
use resvg::{tiny_skia, usvg};
use std::sync::{Arc, OnceLock};

const FAMILY: &str = "Noto Sans";
const REGULAR: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Bold.ttf");
const ITALIC: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Italic.ttf");

pub fn font_family() -> &'static str {
    FAMILY
}

/// CSS font stack for SVG text: the chosen installed family first, the bundled Noto Sans as the fallback.
pub fn font_stack(family: Option<&str>) -> String {
    match family.map(crate::fonts::clean_name).filter(|f| !f.is_empty() && f != FAMILY) {
        Some(f) => format!("'{f}', {FAMILY}"),
        None => FAMILY.to_string(),
    }
}

/// Bundled face for a layout font name ("sans" | "sans-bold" | "sans-italic").
pub fn font_bytes(font: &str) -> &'static [u8] {
    match font {
        "sans-bold" => BOLD,
        "sans-italic" => ITALIC,
        _ => REGULAR,
    }
}

fn fontdb() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        for f in [REGULAR, BOLD, ITALIC] {
            db.load_font_data(f.to_vec());
        }
        for set in [usvg::fontdb::Database::set_sans_serif_family, usvg::fontdb::Database::set_serif_family, usvg::fontdb::Database::set_monospace_family, usvg::fontdb::Database::set_cursive_family, usvg::fontdb::Database::set_fantasy_family] {
            set(&mut db, FAMILY);
        }
        Arc::new(db)
    })
    .clone()
}

/// The render database for a page: the bundled faces plus the installed families the page's text names (cached per set).
fn render_db(svg: &str) -> Arc<usvg::fontdb::Database> {
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, Arc<usvg::fontdb::Database>>>> = OnceLock::new();
    let mut names: Vec<String> = Vec::new();
    for part in svg.split("font-family=\"").skip(1) {
        let Some(value) = part.split('"').next() else { continue };
        for f in value.split(',') {
            let f = f.trim().trim_matches('\'').to_string();
            if !f.is_empty() && f != FAMILY && !names.contains(&f) && crate::fonts::has_family(&f) {
                names.push(f);
            }
        }
    }
    if names.is_empty() {
        return fontdb();
    }
    names.sort();
    let key = names.join("|");
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry(key)
        .or_insert_with(|| {
            let mut db = (*fontdb()).clone();
            for n in &names {
                for data in crate::fonts::family_data(n) {
                    db.load_font_data(data);
                }
            }
            Arc::new(db)
        })
        .clone()
}

pub fn parse(svg: &str) -> Result<usvg::Tree> {
    let opt = usvg::Options { fontdb: render_db(svg), font_family: FAMILY.into(), dpi: 96.0, ..Default::default() };
    usvg::Tree::from_str(svg, &opt).map_err(|e| anyhow!("Layout SVG could not be read: {e}"))
}

/// Vector PDF, one page the size of the layout page.
pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    let tree = parse(svg)?;
    // The SVG is in CSS pixels (96 per inch); PDF user units are 1/72 inch.
    svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions { dpi: 96.0 }).map_err(|e| anyhow!("PDF export failed: {e:?}"))
}

/// One vector PDF with a page per SVG, each page the size of its layout page (map series, C3.2).
pub fn svgs_to_pdf(svgs: &[String]) -> Result<Vec<u8>> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
    use std::collections::HashMap;
    if svgs.is_empty() {
        return Err(anyhow!("There are no pages to write"));
    }
    let mut pdf = Pdf::new();
    let mut next = Ref::new(1);
    let catalog = next.bump();
    let page_tree = next.bump();
    let mut kids = Vec::new();
    for svg in svgs {
        let tree = parse(svg)?;
        let (chunk, root) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default()).map_err(|e| anyhow!("PDF export failed: {e:?}"))?;
        // Give this page's objects numbers of their own in the shared file.
        let mut map = HashMap::new();
        let chunk = chunk.renumber(|old| *map.entry(old).or_insert_with(|| next.bump()));
        let xobject = map[&root];
        // The SVG is in CSS pixels (96 per inch); PDF user units are 1/72 inch.
        let (w, h) = (tree.size().width() * 0.75, tree.size().height() * 0.75);
        let (page, content_id) = (next.bump(), next.bump());
        let mut p = pdf.page(page);
        p.media_box(Rect::new(0.0, 0.0, w, h)).parent(page_tree).contents(content_id);
        p.resources().x_objects().pair(Name(b"S1"), xobject);
        p.finish();
        let mut c = Content::new();
        c.save_state().transform([w, 0.0, 0.0, h, 0.0, 0.0]).x_object(Name(b"S1")).restore_state();
        pdf.stream(content_id, &c.finish());
        pdf.extend(&chunk);
        kids.push(page);
    }
    pdf.catalog(catalog).pages(page_tree);
    pdf.pages(page_tree).count(kids.len() as i32).kids(kids);
    Ok(pdf.finish())
}

/// PNG of the page at `dpi` (clamped to 20–1200).
pub fn svg_to_png(svg: &str, dpi: f64) -> Result<Vec<u8>> {
    let tree = parse(svg)?;
    let k = dpi.clamp(20.0, 1200.0) / 96.0;
    let (w, h) = ((tree.size().width() as f64 * k).round().max(1.0) as u32, (tree.size().height() as f64 * k).round().max(1.0) as u32);
    let mut pix = tiny_skia::Pixmap::new(w, h).ok_or_else(|| anyhow!("Page too large to render at {dpi} dpi ({w}×{h} px)"))?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(w as f32 / tree.size().width(), h as f32 / tree.size().height()), &mut pix.as_mut());
    pix.encode_png().map_err(|e| anyhow!("PNG encoding failed: {e}"))
}
