//! Program settings (UR-16): the user's own choices, stored once per user in the app's data folder (`resources::data_dir()/settings.json`),
//! never inside a project. Only the fields below are ever written, so nothing else (and no secret: API keys stay in the OS keychain)
//! can end up in the file. A missing, unreadable or half-valid file gives defaults field by field, never an error.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DefaultCrs {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "light" or "dark"; None = whatever the app remembered last.
    pub theme: Option<String>,
    /// Measure tool and calculators: unit keys of the UI's unit tables ("mi", "km", "m", "ft" …; "acres", "ha" …).
    pub length_unit: Option<String>,
    pub area_unit: Option<String>,
    /// The coordinate system a new, empty map starts with when there is none to copy.
    pub default_crs: Option<DefaultCrs>,
    /// The dock arrangement new projects and empty starts open with (UR-17), as the UI saved it. Opaque here.
    pub startup_layout: Option<Value>,
    /// Batch D: "python", "sql", … Kept as text; unknown values are the UI's to ignore.
    pub scripting_language: Option<String>,
    /// UX14: the one format the status bar shows the pointer in ("dd", "dms", "projected", "mgrs").
    pub coordinate_format: Option<String>,
    /// RB3: show the Developer group (Frame Rate, Benchmark, Generate Test Data) on the View tab. Off unless the user turns it on.
    pub show_developer_tools: Option<bool>,
    /// UX1: minutes between autosaves of the recovery copy (0 = off). None = the default (2).
    pub autosave_minutes: Option<u32>,
    /// UX2: project folders opened or saved recently, newest first (at most 10). Paths only; whether they still exist is checked when opened.
    pub recent_projects: Option<Vec<String>>,
    /// UX2: show the Start page when the app opens. None = yes.
    pub show_start_page: Option<bool>,
}

/// How many recent projects are kept.
pub const MAX_RECENT: usize = 10;

fn text(v: &Value, key: &str, allowed: Option<&[&str]>) -> Option<String> {
    let s = v.get(key)?.as_str()?.trim();
    if s.is_empty() || s.len() > 64 {
        return None;
    }
    match allowed {
        Some(list) if !list.contains(&s) => None,
        _ => Some(s.to_string()),
    }
}

impl Settings {
    /// Read the fields that are valid; everything else is left at its default.
    pub fn from_value(v: &Value) -> Settings {
        let default_crs = v.get("defaultCrs").and_then(|c| {
            let id = c.get("id")?.as_str()?.trim();
            let name = c.get("name")?.as_str()?.trim();
            (!id.is_empty() && id.len() <= 4096 && !name.is_empty() && name.len() <= 256).then(|| DefaultCrs { id: id.to_string(), name: name.to_string() })
        });
        // The layout is only kept when it is a JSON object of a sane size.
        let startup_layout = v.get("startupLayout").filter(|l| l.is_object() && l.to_string().len() <= 1_000_000).cloned();
        Settings {
            theme: text(v, "theme", Some(&["light", "dark"])),
            length_unit: text(v, "lengthUnit", None),
            area_unit: text(v, "areaUnit", None),
            default_crs,
            startup_layout,
            scripting_language: text(v, "scriptingLanguage", None),
            coordinate_format: text(v, "coordinateFormat", Some(&["dd", "dms", "projected", "mgrs"])),
            show_developer_tools: v.get("showDeveloperTools").and_then(|b| b.as_bool()),
            autosave_minutes: v.get("autosaveMinutes").and_then(|n| n.as_u64()).filter(|n| *n <= 120).map(|n| n as u32),
            recent_projects: v.get("recentProjects").and_then(|a| a.as_array()).map(|a| {
                let mut out: Vec<String> = vec![];
                for p in a.iter().filter_map(|p| p.as_str()).map(str::trim).filter(|p| !p.is_empty() && p.len() <= 4096) {
                    if !out.iter().any(|o| o == p) && out.len() < MAX_RECENT {
                        out.push(p.to_string());
                    }
                }
                out
            }),
            show_start_page: v.get("showStartPage").and_then(|b| b.as_bool()),
        }
    }
}

pub fn path() -> Option<PathBuf> {
    crate::resources::data_dir().map(|d| d.join("settings.json"))
}

/// The settings in `file`; defaults when it is missing or not JSON.
pub fn load_from(file: &Path) -> Settings {
    std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).map(|v| Settings::from_value(&v)).unwrap_or_default()
}

/// Write `settings` (cleaned the same way as on reading) to `file`, replacing it whole; the folder is made if needed.
pub fn save_to(file: &Path, settings: &Settings) -> Result<Settings> {
    let clean = Settings::from_value(&serde_json::to_value(settings)?);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&clean)?)?;
    std::fs::rename(&tmp, file)?;
    Ok(clean)
}

pub fn load() -> Settings {
    path().map(|p| load_from(&p)).unwrap_or_default()
}

pub fn save(settings: &Settings) -> Result<Settings> {
    match path() {
        Some(p) => save_to(&p, settings),
        None => anyhow::bail!("This computer has no per-user folder to keep settings in."),
    }
}
