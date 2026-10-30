//! Runs one WhiteboxTools program (MIT, John Lindsay) as a child process for the hydrology nodes (phase H4).
//! The program reads and writes GeoTIFF; callers pass absolute paths. The child is killed when the model run is cancelled.

use anyhow::{anyhow, bail, Result};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// True when the program is installed (the hydrology nodes explain this when it is not).
pub fn available() -> bool {
    crate::resources::whitebox_exe().is_some()
}

/// `tool` is WhiteboxTools' tool name (e.g. "FillDepressions"); `args` are (flag, value) pairs without dashes.
/// WhiteboxTools v2.4.0 sometimes panics with "Error unwrapping 'output'" when its worker threads finish (a race in the program, seen
/// in FillDepressions); the run is repeated (single-threaded) up to 3 more times for that message only. Any other failure is reported at once.
pub fn run(tool: &str, args: &[(&str, String)]) -> Result<()> {
    let mut attempt = 1;
    loop {
        // The first try uses every core; the repeats run the program on one thread (`--max_procs=1`), where the race cannot happen.
        let mut a: Vec<(&str, String)> = args.to_vec();
        if attempt > 1 {
            a.push(("max_procs", "1".into()));
        }
        match run_once(tool, &a) {
            Err(e) if attempt < 4 && e.to_string().contains("Error unwrapping") => attempt += 1,
            other => return other,
        }
    }
}

fn run_once(tool: &str, args: &[(&str, String)]) -> Result<()> {
    let exe = crate::resources::whitebox_exe().ok_or_else(|| {
        anyhow!("The hydrology tools need WhiteboxTools, which is not installed with this copy of GIS Studio. Reinstall the app, or set GIS_STUDIO_WHITEBOX to the whitebox_tools program.")
    })?;
    let mut cmd = Command::new(exe);
    cmd.arg(format!("--run={tool}"));
    for (k, v) in args {
        cmd.arg(format!("--{k}={v}"));
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| anyhow!("Could not start WhiteboxTools: {e}"))?;
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
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                if !line.trim().is_empty() && !line.contains('%') {
                    tail.push(line);
                    if tail.len() > 12 {
                        tail.remove(0);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) | Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }
        if crate::progress::run_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            bail!("{}", crate::progress::CANCELLED);
        }
        if let Some(s) = child.try_wait()? {
            // Collect what the readers still hold.
            while let Ok(line) = rx.recv_timeout(Duration::from_millis(50)) {
                if !line.trim().is_empty() && !line.contains('%') {
                    tail.push(line);
                    if tail.len() > 12 {
                        tail.remove(0);
                    }
                }
            }
            break s;
        }
    };
    let said = tail.join("\n");
    // WhiteboxTools reports some failures on its output and still exits 0.
    if !status.success() || tail.iter().any(|l| l.to_ascii_lowercase().starts_with("error")) {
        bail!("WhiteboxTools {tool} failed: {}", if said.is_empty() { "no message".to_string() } else { said });
    }
    Ok(())
}
