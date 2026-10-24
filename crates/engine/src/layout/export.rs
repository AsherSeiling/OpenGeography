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

pub fn parse(svg: &str) -> Result<usvg::Tree> {
    let opt = usvg::Options { fontdb: fontdb(), font_family: FAMILY.into(), dpi: 96.0, ..Default::default() };
    usvg::Tree::from_str(svg, &opt).map_err(|e| anyhow!("Layout SVG could not be read: {e}"))
}

/// Vector PDF, one page the size of the layout page.
pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    let tree = parse(svg)?;
    // The SVG is in CSS pixels (96 per inch); PDF user units are 1/72 inch.
    svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions { dpi: 96.0 }).map_err(|e| anyhow!("PDF export failed: {e:?}"))
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
