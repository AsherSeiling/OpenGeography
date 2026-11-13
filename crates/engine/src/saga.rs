//! Runs SAGA GIS's `saga_cmd` (GPL; https://saga-gis.sourceforge.io) as a child process, like `whitebox.rs` does for WhiteboxTools.
//! SAGA is a separate program that is never linked into GIS Studio: grids go in and come out as SAGA's own `.sgrd/.sdat` files, written and read
//! by our GDAL ("SAGA" driver), and the input layer's CRS is written into the result by the caller (SAGA's own CRS handling is not used).
//! The child is killed when the model run is cancelled.

use anyhow::{anyhow, bail, Context, Result};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// True when the program is installed (the SAGA nodes explain this when it is not).
pub fn available() -> bool {
    crate::resources::saga_exe().is_some()
}

/// Run `saga_cmd <library> <tool> -FLAG value ...`. `args` are (flag without dash, value); grids are `.sgrd` paths.
pub fn run(library: &str, tool: &str, args: &[(&str, String)]) -> Result<()> {
    let exe = crate::resources::saga_exe().ok_or_else(|| {
        anyhow!("This tool needs SAGA GIS, which is not installed with this copy of GIS Studio. Reinstall the app, or set GIS_STUDIO_SAGA to the saga_cmd program.")
    })?;
    let tools = exe.parent().map(|d| d.join("tools")).unwrap_or_default();
    let mut cmd = Command::new(&exe);
    // `-f=s`: silent (no progress bars or banners; errors still reach the output). Tool libraries are looked up in the folder beside the program.
    cmd.arg("-f=s").arg(library).arg(tool);
    for (k, v) in args {
        cmd.arg(format!("-{k}")).arg(v);
    }
    cmd.env("SAGA_TLB", &tools).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| anyhow!("Could not start SAGA: {e}"))?;
    let (tx, rx) = mpsc::channel::<String>();
    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)].into_iter().flatten() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(|l| l.ok()) {
                let _ = tx.send(line);
            }
        });
    }
    drop(tx);
    let mut tail: Vec<String> = vec![];
    let keep = |line: String, tail: &mut Vec<String>| {
        if !line.trim().is_empty() && !line.contains('%') {
            tail.push(line);
            if tail.len() > 12 {
                tail.remove(0);
            }
        }
    };
    let status = loop {
        if let Ok(line) = rx.recv_timeout(Duration::from_millis(100)) {
            keep(line, &mut tail);
        }
        if crate::progress::run_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            bail!("{}", crate::progress::CANCELLED);
        }
        if let Some(s) = child.try_wait()? {
            while let Ok(line) = rx.recv_timeout(Duration::from_millis(50)) {
                keep(line, &mut tail);
            }
            break s;
        }
    };
    let said = tail.join("\n");
    // saga_cmd prints "[Error] ..." lines for bad arguments and failed tools, and does not always set the exit code.
    if !status.success() || tail.iter().any(|l| l.contains("[Error]") || l.contains("Execution failed")) {
        bail!("SAGA {library} {tool} failed: {}", if said.is_empty() { "no message".to_string() } else { said });
    }
    Ok(())
}

/// A scratch folder for one run (SAGA grids in and out), removed when dropped.
pub struct Work {
    dir: PathBuf,
}

impl Work {
    pub fn new() -> Result<Work> {
        let dir = crate::raster::new_store_path("saga_work", "d");
        std::fs::create_dir_all(&dir).with_context(|| format!("Could not create a work folder in {}", dir.display()))?;
        Ok(Work { dir })
    }

    /// `<work>/<name>.sgrd`, the path SAGA is given for a grid (input or output).
    pub fn grid(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.sgrd"))
    }

    /// A path in the folder for outputs that are not grids (shapefiles SAGA insists on writing and we discard).
    pub fn path(&self, file: &str) -> PathBuf {
        self.dir.join(file)
    }

    /// Copy a raster band into a SAGA grid with GDAL's SAGA driver (extent, cell size and no-data value come across; the CRS is not used).
    pub fn import(&self, src: &Path, name: &str) -> Result<PathBuf> {
        let ds = gdal::Dataset::open(src).map_err(|e| anyhow!("Could not open the raster: {e}"))?;
        if ds.raster_count() != 1 {
            bail!("SAGA tools take a single-band raster; this one has {} bands. Use Select Bands first.", ds.raster_count());
        }
        let drv = gdal::DriverManager::get_driver_by_name("SAGA").map_err(|_| anyhow!("This copy of GDAL cannot write SAGA grids."))?;
        let sdat = self.dir.join(format!("{name}.sdat"));
        ds.create_copy(&drv, &sdat, &gdal::raster::RasterCreationOptions::new()).map_err(|e| anyhow!("Could not hand the raster to SAGA: {e}"))?;
        // The input's CRS stays with GIS Studio: SAGA would otherwise try to run its projection tools (not shipped) when it reads the grid.
        let _ = std::fs::remove_file(self.dir.join(format!("{name}.prj")));
        Ok(self.grid(name))
    }

    /// Convert a SAGA result grid into a GeoTIFF at `dst`. The caller writes the CRS.
    pub fn export(&self, name: &str, dst: &Path) -> Result<()> {
        let sdat = self.dir.join(format!("{name}.sdat"));
        if !sdat.is_file() {
            bail!("SAGA did not produce the result grid '{name}'.");
        }
        let ds = gdal::Dataset::open(&sdat).map_err(|e| anyhow!("Could not read SAGA's result: {e}"))?;
        let drv = gdal::DriverManager::get_driver_by_name("GTiff")?;
        let opts = gdal::raster::RasterCreationOptions::from_iter(["COMPRESS=LZW", "TILED=YES", "BIGTIFF=IF_SAFER"]);
        ds.create_copy(&drv, dst, &opts).map_err(|e| anyhow!("Could not write the result raster: {e}"))?;
        Ok(())
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
