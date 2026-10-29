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

/// Per-user folder for downloaded map tiles (layout basemap previews), created on first use. `GIS_STUDIO_CACHE`
/// overrides it (tests, portable installs). Safe to delete: tiles are fetched again.
pub fn tile_cache_dir() -> Option<PathBuf> {
    let dir = if let Some(d) = std::env::var_os("GIS_STUDIO_CACHE") {
        PathBuf::from(d)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Caches/GIS Studio/tiles")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("GIS Studio/cache/tiles")
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?.join("gis-studio/tiles")
    };
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Per-user app-data folder (not created). `GIS_STUDIO_DATA` overrides it (tests, portable installs).
pub fn data_dir() -> Option<PathBuf> {
    Some(if let Some(d) = std::env::var_os("GIS_STUDIO_DATA") {
        PathBuf::from(d)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/GIS Studio")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA").or_else(|| std::env::var_os("LOCALAPPDATA"))?).join("GIS Studio")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?.join("gis-studio")
    })
}

/// Per-user folder for datum grids the user downloaded (Datum Grids menu). PROJ is told to search it (`PROJ_USER_WRITABLE_DIRECTORY`).
pub fn grid_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("proj-grids"))
}

/// Tell PROJ (GDAL's and DuckDB's copies alike) to look for datum grids in the user's download folder. Call before PROJ starts.
pub fn use_user_grid_dir() {
    if std::env::var_os("PROJ_USER_WRITABLE_DIRECTORY").is_none() {
        if let Some(d) = grid_dir() {
            let _ = std::fs::create_dir_all(&d);
            std::env::set_var("PROJ_USER_WRITABLE_DIRECTORY", d);
        }
    }
}

/// Make GDAL's PROJ search `PROJ_DATA`/`PROJ_LIB` and then the user's grid folder (its own copy of PROJ ignores
/// `PROJ_USER_WRITABLE_DIRECTORY`; DuckDB's copy honours the variable).
pub fn apply_proj_search_paths() {
    let mut dirs: Vec<PathBuf> = vec![];
    for var in ["PROJ_DATA", "PROJ_LIB"] {
        if let Some(p) = std::env::var_os(var) {
            for d in std::env::split_paths(&p) {
                if !dirs.contains(&d) {
                    dirs.push(d);
                }
            }
        }
    }
    if dirs.is_empty() {
        return; // PROJ's own defaults stay
    }
    if let Some(g) = grid_dir() {
        dirs.push(g);
    }
    let c: Vec<std::ffi::CString> = dirs.iter().filter_map(|d| std::ffi::CString::new(d.to_string_lossy().as_bytes()).ok()).collect();
    let mut ptrs: Vec<*const std::ffi::c_char> = c.iter().map(|s| s.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    // SAFETY: the strings outlive the call, which copies them.
    unsafe { gdal_sys::OSRSetPROJSearchPaths(ptrs.as_ptr()) };
}

/// Per-user folder for the user's layout templates, created on first use. `GIS_STUDIO_DATA` overrides the base folder
/// (tests, portable installs).
pub fn templates_dir() -> Option<PathBuf> {
    let base = if let Some(d) = std::env::var_os("GIS_STUDIO_DATA") {
        PathBuf::from(d)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/GIS Studio")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA").or_else(|| std::env::var_os("LOCALAPPDATA"))?).join("GIS Studio")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?.join("gis-studio")
    };
    let dir = base.join("layout_templates");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// DuckDB spatial extension shipped with the app.
///
/// It ships gzip-compressed: Linux packagers (AppImage) rewrite every ELF
/// library they find, which corrupts the metadata DuckDB appends to an
/// extension. It is unpacked once into a per-user cache and loaded from there.
pub fn spatial_extension() -> Option<PathBuf> {
    bundled_extension("spatial")
}

/// A DuckDB extension shipped with the app (`spatial`, `httpfs`), unpacked once; None in a dev build, where DuckDB installs it itself.
pub fn bundled_extension(name: &str) -> Option<PathBuf> {
    let dir = dir()?.join("duckdb");
    let file = format!("{name}.duckdb_extension");
    let plain = dir.join(&file);
    if plain.is_file() {
        return Some(plain);
    }
    let gz = dir.join(format!("{file}.gz"));
    let size = std::fs::metadata(&gz).ok()?.len();
    // Keyed by the archive's size so an updated app unpacks its own copy.
    let out = std::env::temp_dir().join(format!("gis-studio-{size}")).join(&file);
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
            eprintln!("Could not unpack the DuckDB {name} extension: {e}");
            None
        }
    }
}

/// Make PROJ find its data when this isn't an installed app (dev builds, tests, the browser dev server). DuckDB Spatial carries its
/// own PROJ, which otherwise falls back to a small built-in database without the datum files: NAD83 ↔ WGS 84 then comes out as no
/// change and NAD27 is a few metres off (bug C-1). The data folder GDAL's PROJ uses is the right one, so it is handed to both.
/// An environment that already sets `PROJ_DATA` or `PROJ_LIB` (the installed app, `resources::init`, a test) is left alone.
pub fn ensure_proj_data() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        use_user_grid_dir();
        if std::env::var_os("PROJ_DATA").is_some() || std::env::var_os("PROJ_LIB").is_some() {
            apply_proj_search_paths();
            return;
        }
        let mut dirs: Vec<PathBuf> = Vec::new();
        // The folders GDAL's own PROJ searches.
        unsafe {
            let list = gdal_sys::OSRGetPROJSearchPaths();
            if !list.is_null() {
                let n = gdal_sys::CSLCount(list);
                for i in 0..n {
                    let p = *list.offset(i as isize);
                    if !p.is_null() {
                        dirs.push(PathBuf::from(std::ffi::CStr::from_ptr(p).to_string_lossy().to_string()));
                    }
                }
                gdal_sys::CSLDestroy(list);
            }
        }
        dirs.extend(["/opt/homebrew/share/proj", "/usr/local/share/proj", "/usr/share/proj"].map(PathBuf::from));
        if let Some(d) = dirs.into_iter().find(|d| d.join("proj.db").is_file()) {
            std::env::set_var("PROJ_DATA", &d);
            std::env::set_var("PROJ_LIB", &d);
        }
        apply_proj_search_paths();
    });
}

/// The folder PROJ is told to use, if any.
pub fn proj_data_dir() -> Option<PathBuf> {
    std::env::var_os("PROJ_DATA").or_else(|| std::env::var_os("PROJ_LIB")).map(PathBuf::from)
}

/// Point GDAL and PROJ at the bundled data. Call once, early, before any
/// GDAL or engine use. PROJ downloads datum grids on demand (cdn.proj.org).
pub fn init() {
    let Some(dir) = dir() else { return };
    // The installed app always uses its own GDAL/PROJ data: a GDAL_DATA or
    // PROJ_DATA left by another GIS install would pair this GDAL/PROJ with
    // mismatched files (wrong or missing coordinate systems). Set
    // GIS_STUDIO_KEEP_ENV=1 to keep the environment's values (debugging).
    let keep = std::env::var_os("GIS_STUDIO_KEEP_ENV").is_some();
    let set = |k: &str, v: &Path| {
        if v.exists() && (!keep || std::env::var_os(k).is_none()) {
            if let Some(old) = std::env::var_os(k).filter(|o| Path::new(o) != v) {
                eprintln!("GIS Studio: using bundled {k} instead of {}", Path::new(&old).display());
            }
            std::env::set_var(k, v);
        }
    };
    set("GDAL_DATA", &dir.join("gdal"));
    set("PROJ_DATA", &dir.join("proj"));
    set("PROJ_LIB", &dir.join("proj"));
    // PROJ fetches nothing on its own: datum grids come from the installer or from the Datum Grids menu, on the user's click.
    if std::env::var_os("PROJ_NETWORK").is_none() {
        std::env::set_var("PROJ_NETWORK", "OFF");
    }
    use_user_grid_dir();
    apply_proj_search_paths();
    // Don't pick up GDAL plugins from a system install.
    if !keep || std::env::var_os("GDAL_DRIVER_PATH").is_none() {
        std::env::set_var("GDAL_DRIVER_PATH", dir.join("gdalplugins"));
    }
}

/// The WhiteboxTools command-line program (MIT, https://github.com/jblindsay/whitebox-tools) shipped for the hydrology tools.
/// `GIS_STUDIO_WHITEBOX` points at a copy elsewhere (development, tests); None when it is not installed.
pub fn whitebox_exe() -> Option<PathBuf> {
    let name = format!("whitebox_tools{}", std::env::consts::EXE_SUFFIX);
    let candidates = [std::env::var_os("GIS_STUDIO_WHITEBOX").map(PathBuf::from), dir().map(|d| d.join("whitebox").join(&name))];
    candidates.into_iter().flatten().find(|p| p.is_file())
}
