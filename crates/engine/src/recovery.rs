//! Autosave and crash recovery (UX1).
//!
//! The recovery copy is a separate set of files in the per-user app-data folder (`<data_dir>/recovery/<session>/`), never inside a project
//! folder and never over one. Writing it does not touch the open project, its files, or any source data. It holds what a project holds
//! (the database and `project.json`) and nothing else: no API keys or tokens (those live in the OS keychain and are in neither file).
//!
//! Layout of one session's folder:
//! * `project-<gen>.duckdb`, `project-<gen>.json`: generation `gen` of the copy;
//! * `recovery.json`: the manifest, written last through a rename. It names the generation and the exact byte sizes of its two files.
//!   A copy is whole only when the manifest is there and both files match it, so a crash in the middle of a write leaves the earlier
//!   generation in force (older generations are deleted only after the new manifest is in place).
//!
//! A session is one run of the program (`s<pid>-<start>`). A clean save removes its folder; a crash leaves it, and the next start lists
//! the folders of sessions that are no longer running and offers to restore or discard each.

use crate::db::{ident, lit, Pool};
use crate::{check_project_format, Engine};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Version of the recovery copy's own layout (the manifest). Not the project format.
const RECOVERY_FORMAT: u32 = 1;
const MANIFEST: &str = "recovery.json";
/// A copy whose owner still shows as running is hidden only this long after it was last written, in case the process id was reused.
const ALIVE_GRACE_SECS: u64 = 3600;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format: u32,
    session: String,
    pid: u32,
    gen: u64,
    written_at: u64,
    app_version: String,
    /// The project folder the work came from; None for a project that was never saved.
    origin: Option<String>,
    name: String,
    db_bytes: u64,
    json_bytes: u64,
}

/// One recovery copy as the start-up offer shows it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryEntry {
    /// Opaque; pass back to restore or discard.
    pub id: String,
    pub name: String,
    /// The project it came from (None: a project that was never saved).
    pub origin: Option<String>,
    /// Seconds since 1970 when the copy was last written.
    pub written_at: u64,
    /// Why the copy can't be restored, as a sentence; None when it is whole.
    pub problem: Option<String>,
}

/// What a restore hands the UI.
pub struct Restored {
    pub ui_state: String,
    pub origin: Option<String>,
}

/// This run's session id.
pub fn session_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        format!("s{}-{n:x}", std::process::id())
    })
}

/// Where recovery copies live: `<per-user data folder>/recovery`. `GIS_STUDIO_DATA` moves it, like everything else per-user.
pub fn root() -> Result<PathBuf> {
    crate::resources::data_dir().map(|d| d.join("recovery")).ok_or_else(|| anyhow!("This computer has no per-user folder to keep recovery copies in."))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Writes of this session's copy never overlap (an autosave and a discard, two autosaves).
fn write_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

fn read_manifest(dir: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(dir.join(MANIFEST)).map_err(|_| anyhow!("its index file is missing, so the copy was never finished"))?;
    let m: Manifest = serde_json::from_str(&text).map_err(|_| anyhow!("its index file is damaged"))?;
    if m.format != RECOVERY_FORMAT {
        return Err(anyhow!("it was written by a different version of GIS Studio (recovery format {})", m.format));
    }
    Ok(m)
}

fn db_file(dir: &Path, gen: u64) -> PathBuf {
    dir.join(format!("project-{gen}.duckdb"))
}
fn json_file(dir: &Path, gen: u64) -> PathBuf {
    dir.join(format!("project-{gen}.json"))
}

/// Check one copy is whole; returns its manifest and `project.json` text.
fn verify(dir: &Path) -> std::result::Result<(Manifest, String), String> {
    let m = read_manifest(dir).map_err(|e| format!("This recovery copy can't be used: {e}."))?;
    let size = |p: PathBuf| std::fs::metadata(p).map(|x| x.len()).ok();
    if size(db_file(dir, m.gen)) != Some(m.db_bytes) || size(json_file(dir, m.gen)) != Some(m.json_bytes) {
        return Err("This recovery copy can't be used: its files are incomplete or damaged (the program probably stopped while writing it).".into());
    }
    let ui = std::fs::read_to_string(json_file(dir, m.gen)).map_err(|_| "This recovery copy can't be used: its settings file can't be read.".to_string())?;
    check_project_format(&ui).map_err(|e| format!("This recovery copy can't be used: {e}"))?;
    Ok((m, ui))
}

fn process_alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        // Signal 0 checks that the process exists; EPERM means it exists but belongs to someone else.
        let r = unsafe { libc::kill(pid as i32, 0) };
        r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        // No dependency-free check here: treat every other session as finished, so a second running copy of the program may see the first's
        // copy offered. Restoring is always the user's choice.
        false
    }
}

/// An id from the UI names a folder directly inside `root`; anything else is refused.
fn slot(root: &Path, id: &str) -> Result<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return Err(anyhow!("That is not a recovery copy."));
    }
    Ok(root.join(id))
}

impl Engine {
    /// Write this session's recovery copy of the open work: the database as it is now plus `ui_state`. The open project, its folder and the
    /// engine's state are not touched, so an autosave can never mark the project saved or change it. Does not wait for other work: a database
    /// copy reads a snapshot, so running tasks carry on. `origin` is the project folder the work came from, if it was ever saved.
    pub fn write_recovery(&self, root: &Path, ui_state: &str, origin: Option<&Path>, name: &str) -> Result<()> {
        let _guard = write_lock().lock().unwrap_or_else(|p| p.into_inner());
        let dir = root.join(session_id());
        std::fs::create_dir_all(&dir).with_context(|| format!("Creating the recovery folder {}", dir.display()))?;
        let gen = read_manifest(&dir).map(|m| m.gen + 1).unwrap_or(1);
        let db_path = db_file(&dir, gen);
        let cleanup = |dir: &Path| {
            let _ = std::fs::remove_file(db_file(dir, gen));
            let _ = std::fs::remove_file(format!("{}.wal", db_file(dir, gen).display()));
            let _ = std::fs::remove_file(json_file(dir, gen));
        };
        cleanup(&dir);
        let copied: Result<()> = (|| {
            let pool = self.pool.read();
            let conn = pool.get()?;
            let cur: String = conn.query_row("SELECT current_database()", [], |r| r.get(0))?;
            let r = conn.execute_batch(&format!(
                "ATTACH {} AS __rec; COPY FROM DATABASE {} TO __rec; DETACH __rec;",
                lit(db_path.to_str().ok_or_else(|| anyhow!("Bad path"))?),
                ident(&cur)
            ));
            if let Err(e) = r {
                let _ = conn.execute_batch("DETACH DATABASE IF EXISTS __rec");
                return Err(e.into());
            }
            Ok(())
        })();
        if let Err(e) = copied {
            cleanup(&dir);
            return Err(anyhow!("The recovery copy could not be written: {e:#}. Your project is unchanged."));
        }
        let _ = std::fs::remove_file(format!("{}.wal", db_path.display()));
        let written: Result<()> = (|| {
            std::fs::write(json_file(&dir, gen), ui_state)?;
            let m = Manifest {
                format: RECOVERY_FORMAT,
                session: session_id().to_string(),
                pid: std::process::id(),
                gen,
                written_at: now_secs(),
                app_version: env!("CARGO_PKG_VERSION").to_string(),
                origin: origin.map(|p| p.display().to_string()),
                name: name.chars().take(200).collect(),
                db_bytes: std::fs::metadata(&db_path)?.len(),
                json_bytes: ui_state.len() as u64,
            };
            let tmp = dir.join(format!("{MANIFEST}.tmp"));
            std::fs::write(&tmp, serde_json::to_string_pretty(&m)?)?;
            std::fs::rename(&tmp, dir.join(MANIFEST))?;
            Ok(())
        })();
        match written {
            Ok(()) => {
                // The new generation is in force; earlier ones can go.
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for f in rd.flatten() {
                        let n = f.file_name().to_string_lossy().into_owned();
                        if n.starts_with("project-") && !n.starts_with(&format!("project-{gen}.")) {
                            let _ = std::fs::remove_file(f.path());
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                cleanup(&dir);
                Err(anyhow!("The recovery copy could not be written: {e:#}. Your project is unchanged."))
            }
        }
    }

    /// Restore one recovery copy into the engine as an unsaved project (not tied to any folder: the next Save asks where). The copy itself
    /// is left in place until the UI discards it. A partial or damaged copy is refused with a sentence and the open work is left as it was.
    pub fn restore_recovery(&self, root: &Path, id: &str) -> Result<Restored> {
        let dir = slot(root, id)?;
        let (m, ui_state) = verify(&dir).map_err(|e| anyhow!(e))?;
        // Build the working copy in a fresh in-memory database; the engine switches to it only when it is complete.
        let pool = Pool::open(None, self.gate.clone())?;
        {
            let conn = pool.get()?;
            let dst: String = conn.query_row("SELECT current_database()", [], |r| r.get(0))?;
            let src = db_file(&dir, m.gen);
            let r = conn.execute_batch(&format!(
                "ATTACH {} AS __rec (READ_ONLY); COPY FROM DATABASE __rec TO {}; DETACH __rec;",
                lit(src.to_str().ok_or_else(|| anyhow!("Bad path"))?),
                ident(&dst)
            ));
            if let Err(e) = r {
                let _ = conn.execute_batch("DETACH DATABASE IF EXISTS __rec");
                return Err(anyhow!("This recovery copy can't be used: its data could not be read ({e}). Nothing was changed."));
            }
        }
        *self.pool.write() = pool;
        // Rasters kept inside the original project are stored relative to it: resolve them there for loading, then the work is unsaved.
        *self.project_dir.write() = m.origin.as_deref().map(PathBuf::from);
        *self.graph_cache.lock() = Default::default();
        self.tile_cache.lock().clear();
        let init = self.init_schema();
        *self.project_dir.write() = None;
        init.map_err(|e| anyhow!("This recovery copy can't be used: {e:#}"))?;
        Ok(Restored { ui_state, origin: m.origin })
    }
}

/// The recovery copies left by sessions that are no longer running, newest first. Damaged ones are listed with `problem` so they can be discarded.
pub fn list(root: &Path) -> Vec<RecoveryEntry> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(root) else { return out };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let id = e.file_name().to_string_lossy().into_owned();
        if id == session_id() {
            continue;
        }
        let dir = e.path();
        match verify(&dir) {
            Ok((m, _)) => {
                // Another running copy of the program owns this one; leave it alone (unless it has been silent for an hour).
                if process_alive(m.pid) && now_secs().saturating_sub(m.written_at) < ALIVE_GRACE_SECS {
                    continue;
                }
                out.push(RecoveryEntry { id, name: m.name, origin: m.origin, written_at: m.written_at, problem: None });
            }
            Err(problem) => {
                let mtime = std::fs::metadata(&dir).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                // A folder being written by a running session right now has no manifest yet; give it a minute.
                if !dir.join(MANIFEST).exists() && now_secs().saturating_sub(mtime) < 60 {
                    continue;
                }
                out.push(RecoveryEntry { id, name: "Unfinished recovery copy".into(), origin: None, written_at: mtime, problem: Some(problem) });
            }
        }
    }
    out.sort_by(|a, b| b.written_at.cmp(&a.written_at));
    out
}

/// Delete one recovery copy (the user's choice to discard it, or after restoring it).
pub fn discard(root: &Path, id: &str) -> Result<()> {
    let dir = slot(root, id)?;
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("Removing the recovery copy {}", dir.display()))?;
    }
    Ok(())
}

/// Remove this session's copy: after a clean save, or when the work it held was left on purpose.
pub fn discard_own(root: &Path) -> Result<()> {
    let _guard = write_lock().lock().unwrap_or_else(|p| p.into_inner());
    discard(root, session_id())
}
