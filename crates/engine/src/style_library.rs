//! Personal style library (UX9): named layer styles kept once per user in the app's data folder (`resources::data_dir()/style-library.json`),
//! never inside a project and never a secret. The style is the UI's `LayerStyle` as JSON, opaque here apart from a size limit and a check
//! that it is an object with a `kind`. A missing or unreadable file is an empty library, never an error; writes go through a rename.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Entries kept; saving more is refused with a sentence rather than dropping one silently.
pub const MAX_ENTRIES: usize = 200;
const MAX_STYLE_BYTES: usize = 1_000_000;
const MAX_NAME: usize = 100;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub id: String,
    pub name: String,
    /// "Point", "Line", "Polygon" or "Mixed": the kind of layer it was saved from (a hint; a style can be applied to any vector layer).
    pub geom_type: String,
    pub saved_at: u64,
    pub style: Value,
}

pub fn path() -> Option<PathBuf> {
    crate::resources::data_dir().map(|d| d.join("style-library.json"))
}

fn valid(e: &LibraryEntry) -> bool {
    !e.id.is_empty() && !e.name.trim().is_empty() && e.name.chars().count() <= MAX_NAME && e.style.get("kind").and_then(|k| k.as_str()).is_some()
}

/// The entries in `file`, newest first; entries that are not valid are skipped.
pub fn load_from(file: &Path) -> Vec<LibraryEntry> {
    let mut v: Vec<LibraryEntry> = std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    v.retain(valid);
    v.truncate(MAX_ENTRIES);
    v.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    v
}

fn write(file: &Path, entries: &[LibraryEntry]) -> Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(entries)?)?;
    std::fs::rename(&tmp, file)?;
    Ok(())
}

/// Save a style under `name`; a style already saved under the same name (ignoring case) is replaced. Returns the library.
pub fn save_to(file: &Path, name: &str, geom_type: &str, style: Value) -> Result<Vec<LibraryEntry>> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("Give the style a name to save it in the library."));
    }
    if name.chars().count() > MAX_NAME {
        return Err(anyhow!("The style's name is too long (at most {MAX_NAME} characters)."));
    }
    if style.get("kind").and_then(|k| k.as_str()).is_none() {
        return Err(anyhow!("That is not a layer style, so it was not saved."));
    }
    if style.to_string().len() > MAX_STYLE_BYTES {
        return Err(anyhow!("That style is too large to keep in the library (over 1 MB, usually a very large marker picture)."));
    }
    let mut entries = load_from(file);
    entries.retain(|e| e.name.to_lowercase() != name.to_lowercase());
    if entries.len() >= MAX_ENTRIES {
        return Err(anyhow!("The style library is full ({MAX_ENTRIES} styles). Delete some to save more."));
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    entries.insert(0, LibraryEntry { id: format!("st{now:x}"), name: name.to_string(), geom_type: geom_type.chars().take(16).collect(), saved_at: (now / 1_000_000_000) as u64, style });
    write(file, &entries)?;
    Ok(entries)
}

/// Remove one entry by id (nothing to do when it is already gone). Returns the library.
pub fn delete_from(file: &Path, id: &str) -> Result<Vec<LibraryEntry>> {
    let mut entries = load_from(file);
    let before = entries.len();
    entries.retain(|e| e.id != id);
    if entries.len() != before {
        write(file, &entries)?;
    }
    Ok(entries)
}

fn the_file() -> Result<PathBuf> {
    path().ok_or_else(|| anyhow!("This computer has no per-user folder to keep the style library in."))
}

pub fn list() -> Result<Vec<LibraryEntry>> {
    Ok(load_from(&the_file()?))
}
pub fn save(name: &str, geom_type: &str, style: Value) -> Result<Vec<LibraryEntry>> {
    save_to(&the_file()?, name, geom_type, style)
}
pub fn delete(id: &str) -> Result<Vec<LibraryEntry>> {
    delete_from(&the_file()?, id)
}
