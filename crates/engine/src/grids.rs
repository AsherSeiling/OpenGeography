//! Datum grids (CRS accuracy plan B1/B2): the catalog of PROJ-data grids sorted by region and purpose, which are installed, and
//! downloads from the PROJ-data server on the user's click, checked against a SHA-256 from the catalog. Grids the installer ships are
//! listed in `assets/bundled_grids.txt` (fetched at packaging time by `scripts/stage-resources.sh`); downloaded ones go to
//! `resources::grid_dir()`, which PROJ is told to search.

use crate::progress::Task;
use crate::resources;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A grid above this size (50 MB) asks for confirmation before it downloads.
pub const CONFIRM_ABOVE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridEntry {
    pub name: String,
    pub country: String,
    pub continent: String,
    pub source: String,
    pub source_id: String,
    /// horizontal, vertical, 3d, deformation or other.
    pub kind: String,
    pub area: String,
    pub from: String,
    pub to: String,
    pub description: String,
    pub size: u64,
    pub sha256: String,
    pub url: String,
    /// Shipped in the installer.
    #[serde(default)]
    pub bundled: bool,
    /// Present in a folder PROJ searches.
    #[serde(default)]
    pub installed: bool,
    /// "app" (shipped), "downloaded" (the user's grid folder) or "" when not installed.
    #[serde(default)]
    pub location: String,
}

fn catalog() -> &'static Vec<GridEntry> {
    static C: OnceLock<Vec<GridEntry>> = OnceLock::new();
    C.get_or_init(|| {
        let mut v: Vec<GridEntry> = serde_json::from_str(include_str!("../assets/proj_grids_catalog.json")).unwrap_or_default();
        let bundled: std::collections::HashSet<&str> = include_str!("../assets/bundled_grids.txt")
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .filter_map(|l| l.split_whitespace().next())
            .collect();
        for g in &mut v {
            g.bundled = bundled.contains(g.name.as_str());
        }
        v
    })
}

/// The folders PROJ reads grids from, as far as this app sets them: `PROJ_DATA`/`PROJ_LIB` (one or more) and the user's grid folder.
fn search_dirs() -> Vec<(PathBuf, &'static str)> {
    let mut v: Vec<(PathBuf, &'static str)> = vec![];
    for var in ["PROJ_DATA", "PROJ_LIB"] {
        if let Some(p) = std::env::var_os(var) {
            v.extend(std::env::split_paths(&p).map(|d| (d, "app")));
        }
    }
    if let Some(d) = resources::grid_dir() {
        v.push((d, "downloaded"));
    }
    v
}

/// Every grid PROJ-data offers, with where it is installed. Sorted by continent, country, purpose, then name.
pub fn list() -> Vec<GridEntry> {
    let dirs = search_dirs();
    catalog()
        .iter()
        .map(|g| {
            let mut g = g.clone();
            if let Some((_, loc)) = dirs.iter().find(|(d, _)| d.join(&g.name).is_file()) {
                g.installed = true;
                g.location = (*loc).to_string();
            }
            g
        })
        .collect()
}

fn find(name: &str) -> Result<GridEntry> {
    catalog().iter().find(|g| g.name == name).cloned().ok_or_else(|| anyhow!("'{name}' is not a datum grid in the PROJ-data catalog"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Download `entry` from `url` into `dir`, checking size and SHA-256; the file appears only when complete and correct.
pub fn download_to(entry: &GridEntry, url: &str, dir: &Path, task: &Task) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).map_err(|e| anyhow!("Could not create the grid folder {}: {e}", dir.display()))?;
    let dest = dir.join(&entry.name);
    let part = dir.join(format!("{}.part", entry.name));
    let resp = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(20))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
        .get(url)
        .call()
        .map_err(|e| anyhow!("Could not download {}: {e}. Check the internet connection and try again.", entry.name))?;
    let mut reader = resp.into_reader();
    let mut out = std::fs::File::create(&part).map_err(|e| anyhow!("Could not write {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut total = 0u64;
    let result = (|| -> Result<()> {
        loop {
            task.check()?;
            let n = reader.read(&mut buf).map_err(|e| anyhow!("The download of {} was interrupted: {e}", entry.name))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| anyhow!("Could not write {} (is the disk full?): {e}", part.display()))?;
            total += n as u64;
            task.set(total as f64 / entry.size.max(1) as f64, Some(format!("{:.1} of {:.1} MB", total as f64 / 1e6, entry.size as f64 / 1e6)));
        }
        out.flush()?;
        if total != entry.size {
            bail!("{} arrived as {total} bytes but should be {}; it was not installed.", entry.name, entry.size);
        }
        let got = hex(&hasher.finalize());
        if got != entry.sha256 {
            bail!("{} failed its checksum (got {got}); it was not installed. Try again, and report it if it keeps failing.", entry.name);
        }
        Ok(())
    })();
    drop(out);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &dest).map_err(|e| anyhow!("Could not finish installing {}: {e}", entry.name))?;
    Ok(dest)
}

/// Download one catalog grid to the user's grid folder (a Task with Cancel). Grids above 50 MB need `confirmed`.
pub fn download(name: &str, confirmed: bool) -> Result<GridEntry> {
    let entry = find(name)?;
    if entry.size > CONFIRM_ABOVE_BYTES && !confirmed {
        bail!("{} is {:.0} MB. Confirm to download it.", entry.name, entry.size as f64 / 1e6);
    }
    let dir = resources::grid_dir().ok_or_else(|| anyhow!("There is no per-user data folder to keep downloaded grids in."))?;
    let task = Task::start(format!("Datum grid {}", entry.name)).cancellable();
    download_to(&entry, &entry.url, &dir, &task)?;
    Ok(list().into_iter().find(|g| g.name == name).unwrap_or(entry))
}

/// Remove a grid the user downloaded. Grids that ship with the app are never touched.
pub fn remove(name: &str) -> Result<()> {
    let entry = find(name)?;
    let dir = resources::grid_dir().ok_or_else(|| anyhow!("There is no per-user data folder."))?;
    let f = dir.join(&entry.name);
    if !f.is_file() {
        bail!("{} is not one of the grids you downloaded, so it cannot be removed here.", entry.name);
    }
    std::fs::remove_file(&f).map_err(|e| anyhow!("Could not remove {}: {e}", entry.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write as _};
    use std::net::TcpListener;

    /// Serve `body` once on a local port; returns the URL.
    fn serve(body: Vec<u8>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                while r.read_line(&mut line).map(|n| n > 2).unwrap_or(false) {
                    line.clear();
                }
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}/grid.tif")
    }

    fn entry(body: &[u8], sha: Option<&str>) -> GridEntry {
        GridEntry {
            name: "test_grid.tif".into(), country: "Testland".into(), continent: "Europe".into(), source: "s".into(), source_id: "s".into(), kind: "horizontal".into(),
            area: "a".into(), from: "A".into(), to: "B".into(), description: "A to B".into(), size: body.len() as u64,
            sha256: sha.map(String::from).unwrap_or_else(|| hex(&Sha256::digest(body))), url: String::new(), bundled: false, installed: false, location: String::new(),
        }
    }

    #[test]
    fn the_catalog_is_sorted_complete_and_knows_what_ships() {
        let c = catalog();
        assert!(c.len() > 350, "{}", c.len());
        assert!(c.iter().all(|g| g.sha256.len() == 64 && g.url.starts_with("https://cdn.proj.org/") && g.size > 0));
        let shipped: Vec<_> = c.iter().filter(|g| g.bundled).collect();
        assert!(shipped.len() >= 40, "{}", shipped.len());
        let mb: f64 = shipped.iter().map(|g| g.size as f64).sum::<f64>() / 1e6;
        assert!(mb < 50.0, "{mb} MB shipped");
        for k in ["horizontal", "vertical"] {
            assert!(c.iter().any(|g| g.kind == k), "{k}");
        }
        // Every manifest line names a catalog grid.
        for l in include_str!("../assets/bundled_grids.txt").lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let n = l.split_whitespace().next().unwrap();
            let sha = l.split_whitespace().nth(1).unwrap();
            let g = c.iter().find(|g| g.name == n).unwrap_or_else(|| panic!("{n} not in the catalog"));
            assert_eq!(g.sha256, sha, "{n}");
        }
    }

    #[test]
    fn a_download_is_checked_and_only_a_correct_file_appears() {
        let dir = tempfile::tempdir().unwrap();
        let body = vec![7u8; 600_000];
        let e = entry(&body, None);
        let f = download_to(&e, &serve(body.clone()), dir.path(), &Task::start("t")).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), body);
        assert!(!dir.path().join("test_grid.tif.part").exists());

        // Wrong checksum: refused, nothing left behind.
        let dir2 = tempfile::tempdir().unwrap();
        let bad = entry(&body, Some(&"0".repeat(64)));
        let err = download_to(&bad, &serve(body.clone()), dir2.path(), &Task::start("t")).unwrap_err().to_string();
        assert!(err.contains("checksum"), "{err}");
        assert_eq!(std::fs::read_dir(dir2.path()).unwrap().count(), 0);

        // Wrong size: refused.
        let dir3 = tempfile::tempdir().unwrap();
        let short = entry(&vec![1u8; 100], None);
        let err = download_to(&short, &serve(vec![1u8; 99]), dir3.path(), &Task::start("t")).unwrap_err().to_string();
        assert!(err.contains("should be 100"), "{err}");
        assert_eq!(std::fs::read_dir(dir3.path()).unwrap().count(), 0);
    }

    #[test]
    fn unknown_names_and_big_grids_are_refused_in_words() {
        assert!(download("nope.tif", false).unwrap_err().to_string().contains("not a datum grid"));
        let big = catalog().iter().find(|g| g.size > CONFIRM_ABOVE_BYTES).expect("a big grid exists");
        let err = download(&big.name, false).unwrap_err().to_string();
        assert!(err.contains("Confirm to download"), "{err}");
        assert!(remove("ca_nrc_ntv2_0.tif").is_err());
    }
}
