//! Files shipped inside the installed app: DuckDB's spatial extension and
//! GDAL/PROJ data. The desktop shell calls `init()` at startup; dev builds
//! without bundled resources fall back to the system (Homebrew) copies.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Candidate resource folders next to the running executable, per platform
/// layout (macOS .app, Windows install dir, Linux deb/AppImage).
fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(dir) = std::env::var_os("GIS_STUDIO_RESOURCES") {
        v.push(PathBuf::from(dir));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin) = exe.parent() {
            v.push(bin.join("../Resources/resources")); // macOS: Contents/Resources/resources
            v.push(bin.join("resources")); // Windows: <install>/resources
            v.push(bin.join("../lib/gis-studio/resources")); // Linux deb / AppImage
            v.push(bin.join("../lib/GIS Studio/resources"));
        }
    }
    v
}

/// The bundled resource folder, if this is an installed app.
pub fn dir() -> Option<&'static Path> {
    DIR.get_or_init(|| candidates().into_iter().find(|d| d.join("duckdb").is_dir()).map(|d| d.canonicalize().unwrap_or(d)))
        .as_deref()
}

/// DuckDB spatial extension shipped with the app.
///
/// It ships gzip-compressed: Linux packagers (AppImage) rewrite every ELF
/// library they find, which corrupts the metadata DuckDB appends to an
/// extension. It is unpacked once into a per-user cache and loaded from there.
pub fn spatial_extension() -> Option<PathBuf> {
    let dir = dir()?.join("duckdb");
    let plain = dir.join("spatial.duckdb_extension");
    if plain.is_file() {
        return Some(plain);
    }
    let gz = dir.join("spatial.duckdb_extension.gz");
    let size = std::fs::metadata(&gz).ok()?.len();
    // Keyed by the archive's size so an updated app unpacks its own copy.
    let out = std::env::temp_dir().join(format!("gis-studio-{size}")).join("spatial.duckdb_extension");
    if out.is_file() {
        return Some(out);
    }
    let unpack = || -> std::io::Result<()> {
        std::fs::create_dir_all(out.parent().unwrap())?;
        let tmp = out.with_extension("part");
        let mut r = flate2::read::GzDecoder::new(std::fs::File::open(&gz)?);
        std::io::copy(&mut r, &mut std::fs::File::create(&tmp)?)?;
        std::fs::rename(&tmp, &out)
    };
    match unpack() {
        Ok(()) => Some(out),
        Err(e) => {
            eprintln!("Could not unpack the DuckDB spatial extension: {e}");
            None
        }
    }
}

/// Point GDAL and PROJ at the bundled data. Call once, early, before any
/// GDAL or engine use. PROJ downloads datum grids on demand (cdn.proj.org).
pub fn init() {
    let Some(dir) = dir() else { return };
    let set = |k: &str, v: &Path| {
        if v.exists() && std::env::var_os(k).is_none() {
            std::env::set_var(k, v);
        }
    };
    set("GDAL_DATA", &dir.join("gdal"));
    set("PROJ_DATA", &dir.join("proj"));
    set("PROJ_LIB", &dir.join("proj"));
    if std::env::var_os("PROJ_NETWORK").is_none() {
        std::env::set_var("PROJ_NETWORK", "ON");
    }
    // Don't pick up GDAL plugins from a system install.
    if std::env::var_os("GDAL_DRIVER_PATH").is_none() {
        std::env::set_var("GDAL_DRIVER_PATH", dir.join("gdalplugins"));
    }
}
