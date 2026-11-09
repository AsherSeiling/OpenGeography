//! Python scripts for the field calculator and the Definition Query node (Batch D, UR-20/UR-21): `def calculate(row): ...`.
//!
//! The script runs in its own CPython process (python-build-standalone, `resources::python_exe`), never inside the app:
//! * the process gets an empty environment (no Census key, Earthdata token or anything else from ours) and an empty working folder;
//! * rows travel in a small binary column protocol (see `pyscript_worker.py`), 50 000 rows at a time, so progress is real and memory is bounded;
//! * a run is cancelled by killing the process;
//! * a script runs only after the user approved its exact text on this computer (`approve`, `is_approved`); the record is in the app-data
//!   folder, never in a project, so a shared project cannot mark itself trusted;
//! * what a script returns is checked strictly (no truthy coercion, no silent null for a failure); failed rows are counted and reported
//!   with their row numbers, and by default a run with any failed row produces nothing.
//!
//! Rows are attributes only. Types reach Python as: integer types → `int` (exact, 64-bit), `FLOAT`/`DOUBLE`/`DECIMAL` → `float`, `BOOLEAN` →
//! `bool`, `DATE` → `datetime.date`, `TIMESTAMP` → naive `datetime.datetime` (microseconds), `VARCHAR` → `str`, NULL → `None`. Everything
//! else (UBIGINT/HUGEINT, timestamps with a zone, BLOB, lists, structs …) arrives as `str` (its text form). A NaN stored in a DOUBLE column
//! arrives as `float('nan')`. A script that returns NaN or ±infinity fails that row ("return None for a missing value").

use crate::progress::{self, Task};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const WORKER: &str = include_str!("pyscript_worker.py");
/// Rows sent to the script process per round.
pub const CHUNK_ROWS: usize = 50_000;
/// Failed rows named in a report.
pub const REPORT_ROWS: usize = 20;

/// Start of the error a node gives while its script waits for approval; the UI keys on it to show the approval notice.
pub const WAITING: &str = "Script waiting for approval";

/// Shown to the user whenever they are asked to approve a script. It must stay this plain.
pub const TRUST_WARNING: &str = "An approved script can do anything you can do on this computer: read, change or delete your files and use the network. \
GIS Studio runs it in a separate Python process with an empty environment, so your API keys and tokens held in environment variables stay out of reach, \
but that is all the protection there is. Approve it only if you wrote it or trust the person who did.";

// ---------- trust ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Trusted {
    approved: Vec<Approval>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub hash: String,
    /// The first line of the script, for the user's own list.
    pub preview: String,
    pub approved_at: u64,
}

fn trust_lock() -> &'static Mutex<()> {
    static L: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
    L.get_or_init(Default::default)
}

/// Fingerprint of a script's text (line endings ignored): an edited script has a different one and asks again.
pub fn script_hash(script: &str) -> String {
    let text = script.replace("\r\n", "\n");
    let mut h = Sha256::new();
    h.update(b"gis-studio-script-v1\n");
    h.update(text.as_bytes());
    hex::encode(h.finalize())
}

fn load_trusted() -> Trusted {
    crate::resources::trusted_scripts_file().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// True when the user approved exactly this script on this computer.
pub fn is_approved(script: &str) -> bool {
    let h = script_hash(script);
    let _g = trust_lock().lock();
    load_trusted().approved.iter().any(|a| a.hash == h)
}

/// Record that the user approved this script. Called after the user clicked Approve in the dialog showing `TRUST_WARNING`, and when they
/// wrote the script here themselves.
pub fn approve(script: &str) -> Result<()> {
    let path = crate::resources::trusted_scripts_file().ok_or_else(|| anyhow!("There is no per-user folder to keep the list of approved scripts in"))?;
    let h = script_hash(script);
    let _g = trust_lock().lock();
    let mut t = load_trusted();
    if t.approved.iter().any(|a| a.hash == h) {
        return Ok(());
    }
    let preview: String = script.lines().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(80).collect();
    let approved_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    t.approved.push(Approval { hash: h, preview, approved_at });
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&t)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// What the UI needs to ask for approval.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrustInfo {
    pub hash: String,
    pub approved: bool,
    pub warning: &'static str,
}

pub fn trust_info(script: &str) -> TrustInfo {
    TrustInfo { hash: script_hash(script), approved: is_approved(script), warning: TRUST_WARNING }
}

/// Whether scripts can run at all on this computer, and with which Python.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PythonStatus {
    pub available: bool,
    pub path: Option<String>,
}

pub fn python_status() -> PythonStatus {
    let p = crate::resources::python_exe();
    PythonStatus { available: p.is_some(), path: p.map(|p| p.display().to_string()) }
}

// ---------- data ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Int,
    Float,
    Bool,
    Date,
    Timestamp,
    Text,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Int => "int",
            Kind::Float => "float",
            Kind::Bool => "bool",
            Kind::Date => "date",
            Kind::Timestamp => "timestamp",
            Kind::Text => "text",
        }
    }

    /// The kind a DuckDB column type reaches Python as, and the SQL that makes the column readable as that kind.
    pub fn of_sql_type(ty: &str) -> Kind {
        let t = ty.trim().to_ascii_uppercase();
        match t.as_str() {
            "TINYINT" | "SMALLINT" | "INTEGER" | "BIGINT" | "UTINYINT" | "USMALLINT" | "UINTEGER" => Kind::Int,
            "FLOAT" | "REAL" | "DOUBLE" => Kind::Float,
            "BOOLEAN" => Kind::Bool,
            "DATE" => Kind::Date,
            "TIMESTAMP" => Kind::Timestamp,
            _ if t.starts_with("DECIMAL") => Kind::Float,
            _ => Kind::Text,
        }
    }

    fn select_expr(self, col: &str) -> String {
        let c = crate::db::ident(col);
        match self {
            Kind::Int => format!("CAST({c} AS BIGINT)"),
            Kind::Float => format!("CAST({c} AS DOUBLE)"),
            Kind::Bool => format!("CAST({c} AS BOOLEAN)"),
            Kind::Date => format!("CAST(date_diff('day', DATE '1970-01-01', {c}) AS INTEGER)"),
            Kind::Timestamp => format!("CAST(epoch_us({c}) AS BIGINT)"),
            Kind::Text => format!("CAST({c} AS VARCHAR)"),
        }
    }
}

/// One column of a chunk of rows.
#[derive(Clone, Debug)]
pub enum ColData {
    Int(Vec<Option<i64>>),
    Float(Vec<Option<f64>>),
    Bool(Vec<Option<bool>>),
    Date(Vec<Option<i32>>),
    Timestamp(Vec<Option<i64>>),
    Text(Vec<Option<String>>),
}

impl ColData {
    fn new(k: Kind) -> ColData {
        match k {
            Kind::Int => ColData::Int(vec![]),
            Kind::Float => ColData::Float(vec![]),
            Kind::Bool => ColData::Bool(vec![]),
            Kind::Date => ColData::Date(vec![]),
            Kind::Timestamp => ColData::Timestamp(vec![]),
            Kind::Text => ColData::Text(vec![]),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub n: usize,
    pub cols: Vec<ColData>,
}

/// The chunk as the bytes the worker reads: row count, then per column a validity byte per row and the values.
pub fn encode_chunk(c: &Chunk) -> Vec<u8> {
    let n = c.n;
    let mut b = Vec::with_capacity(4 + n * 9 * c.cols.len().max(1));
    b.extend_from_slice(&(n as u32).to_le_bytes());
    macro_rules! fixed {
        ($v:expr, $zero:expr) => {{
            b.extend($v.iter().map(|x| x.is_some() as u8));
            for x in $v {
                b.extend_from_slice(&x.unwrap_or($zero).to_le_bytes());
            }
        }};
    }
    for col in &c.cols {
        match col {
            ColData::Int(v) => fixed!(v, 0i64),
            ColData::Float(v) => fixed!(v, 0f64),
            ColData::Date(v) => fixed!(v, 0i32),
            ColData::Timestamp(v) => fixed!(v, 0i64),
            ColData::Bool(v) => {
                b.extend(v.iter().map(|x| x.is_some() as u8));
                b.extend(v.iter().map(|x| x.unwrap_or(false) as u8));
            }
            ColData::Text(v) => {
                b.extend(v.iter().map(|x| x.is_some() as u8));
                for x in v {
                    b.extend_from_slice(&(x.as_ref().map_or(0, |s| s.len()) as u32).to_le_bytes());
                }
                for x in v.iter().flatten() {
                    b.extend_from_slice(x.as_bytes());
                }
            }
        }
    }
    b
}

/// What the script returned for one row.
#[derive(Clone, Debug, PartialEq)]
pub enum Res {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Date(i32),
    Timestamp(i64),
    /// The script raised an error or returned something unusable; the reason is in the report.
    Failed,
}

impl Res {
    pub fn describe(&self) -> &'static str {
        match self {
            Res::Null => "nothing (None)",
            Res::Bool(_) => "a boolean",
            Res::Int(_) => "an integer",
            Res::Float(_) => "a decimal number",
            Res::Text(_) => "text",
            Res::Date(_) => "a date",
            Res::Timestamp(_) => "a date and time",
            Res::Failed => "an error",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Each row returns True or False.
    Filter,
    /// Each row returns the field's value.
    Field,
}

/// Rows whose script failed: how many, and the first few with their 1-based row numbers.
#[derive(Clone, Debug, Default)]
pub struct Failures {
    pub count: u64,
    pub first: Vec<(u64, String)>,
}

impl Failures {
    pub fn add(&mut self, row: u64, msg: String) {
        self.count += 1;
        if self.first.len() < REPORT_ROWS {
            self.first.push((row, msg));
        }
    }

    /// "3 of 1000 rows failed. Row 5: ZeroDivisionError … ; row 9: …"
    pub fn report(&self, total: u64) -> String {
        let shown: Vec<String> = self.first.iter().map(|(r, m)| format!("row {r}: {m}")).collect();
        let more = if self.count as usize > self.first.len() { format!("; and {} more", self.count as usize - self.first.len()) } else { String::new() };
        format!("{} of {} rows failed ({}{})", self.count, total, shown.join("; "), more)
    }
}

pub struct RunResult {
    pub results: Vec<Res>,
    pub failures: Failures,
    pub python_version: String,
}

// ---------- the process ----------

fn read_frame(r: &mut impl Read) -> std::io::Result<(u8, Vec<u8>)> {
    let mut head = [0u8; 5];
    r.read_exact(&mut head)?;
    let n = u32::from_le_bytes([head[1], head[2], head[3], head[4]]) as usize;
    let mut payload = vec![0u8; n];
    r.read_exact(&mut payload)?;
    Ok((head[0], payload))
}

fn frame(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(5 + payload.len());
    f.push(kind);
    f.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    f.extend_from_slice(payload);
    f
}

enum Msg {
    Frame(u8, Vec<u8>),
    Ended,
}

struct ScratchDir(std::path::PathBuf);
impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run `script` over the rows that `next` hands out, one chunk at a time (None = no more rows). `total` is the row count, for progress.
/// Checks approval first. Cancelling (the task's Cancel, or the model run's) kills the process and returns the "Cancelled" error.
pub fn run(script: &str, mode: Mode, fields: &[(String, Kind)], total: u64, task: &Task, mut next: impl FnMut() -> Result<Option<Chunk>>) -> Result<RunResult> {
    if !is_approved(script) {
        bail!("{WAITING}. Review the script and approve it to run it. {TRUST_WARNING}");
    }
    let exe = crate::resources::python_exe().ok_or_else(|| {
        anyhow!("Python scripts need the Python that comes with GIS Studio, and it is not installed with this copy. Reinstall the app (or, for development, build with the dev-python feature and set GIS_STUDIO_PYTHON).")
    })?;
    let scratch = ScratchDir(std::env::temp_dir().join(format!("gis-studio-script-{}-{}", std::process::id(), progress_seq())));
    std::fs::create_dir_all(&scratch.0)?;
    let mut cmd = Command::new(&exe);
    // -I: isolated (ignores PYTHON* variables and the user's site packages); -B: write no .pyc files; utf8 mode for text.
    // Its own process group (unix), so cancelling or finishing can kill whatever the script left running in the background.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.args(["-I", "-B", "-X", "utf8", "-c", WORKER]).current_dir(&scratch.0).env_clear().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // Nothing from our environment goes in. Windows needs its system folder to start the interpreter at all.
    #[cfg(windows)]
    {
        if let Some(v) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", v);
        }
    }
    let mut child = cmd.spawn().map_err(|e| anyhow!("Could not start Python ({}): {e}", exe.display()))?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let err_text = std::sync::Arc::new(Mutex::new(String::new()));
    let err_for_thread = err_text.clone();
    let err_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            let mut s = err_for_thread.lock();
            if s.len() < 8000 {
                s.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
        }
    });
    let (rtx, rrx) = mpsc::channel::<Msg>();
    let reader = std::thread::spawn(move || {
        while let Ok((k, p)) = read_frame(&mut stdout) {
            let done = k == 3;
            if rtx.send(Msg::Frame(k, p)).is_err() || done {
                return;
            }
        }
        let _ = rtx.send(Msg::Ended);
    });
    let (wtx, wrx) = mpsc::sync_channel::<Vec<u8>>(2);
    let writer = std::thread::spawn(move || {
        for f in wrx {
            if stdin.write_all(&f).and_then(|_| stdin.flush()).is_err() {
                return;
            }
        }
    });

    let fields_json: Vec<serde_json::Value> = fields.iter().map(|(n, k)| serde_json::json!({ "name": n, "type": k.name() })).collect();
    let init = serde_json::json!({ "script": script, "mode": if mode == Mode::Filter { "filter" } else { "field" }, "fields": fields_json });
    let outcome: Result<RunResult> = (|| {
        let stop = |child: &mut std::process::Child| -> Result<()> {
            if task.check().is_err() || progress::run_cancelled() {
                let _ = child.kill();
                bail!(progress::CANCELLED);
            }
            Ok(())
        };
        let died = |child: &mut std::process::Child, err: &Mutex<String>| -> anyhow::Error {
            let _ = child.kill();
            std::thread::sleep(Duration::from_millis(50));
            let e = err.lock().trim().to_string();
            anyhow!("The Python process stopped unexpectedly{}", if e.is_empty() { String::new() } else { format!(": {}", e.lines().last().unwrap_or("")) })
        };
        let send = |child: &mut std::process::Child, bytes: Vec<u8>| -> Result<()> {
            let mut bytes = Some(bytes);
            while let Some(b) = bytes.take() {
                match wtx.try_send(b) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(b)) => {
                        stop(child)?;
                        bytes = Some(b);
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => bail!("The Python process stopped unexpectedly"),
                }
            }
            Ok(())
        };
        send(&mut child, frame(1, init.to_string().as_bytes()))?;
        // The script is compiled before any row is sent, so a syntax error shows at once.
        let ready = loop {
            stop(&mut child)?;
            match rrx.recv_timeout(Duration::from_millis(50)) {
                Ok(Msg::Frame(1, p)) => break p,
                Ok(Msg::Frame(..)) => bail!("The Python process answered out of turn"),
                Ok(Msg::Ended) | Err(mpsc::RecvTimeoutError::Disconnected) => return Err(died(&mut child, &err_text)),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        };
        let ready: serde_json::Value = serde_json::from_slice(&ready)?;
        if ready["ok"] != true {
            bail!("The script could not be loaded: {}", ready["error"].as_str().unwrap_or("unknown error"));
        }
        let python_version = ready["python"].as_str().unwrap_or("").to_string();

        let mut results: Vec<Res> = Vec::with_capacity(total.min(5_000_000) as usize);
        let mut failures = Failures::default();
        let mut sent = 0usize; // chunks sent
        let mut got = 0usize; // chunks answered
        let mut all_sent = false;
        let mut done = false;
        let mut row_base = 0u64;
        let mut chunk_sizes: std::collections::VecDeque<usize> = Default::default();
        while !done {
            stop(&mut child)?;
            // Feed the worker while it has fewer than two chunks in flight.
            if !all_sent && sent - got < 2 {
                match next()? {
                    Some(c) if c.n > 0 => {
                        chunk_sizes.push_back(c.n);
                        send(&mut child, frame(2, &encode_chunk(&c)))?;
                        sent += 1;
                    }
                    _ => {
                        send(&mut child, frame(3, &[]))?;
                        all_sent = true;
                    }
                }
                continue;
            }
            match rrx.recv_timeout(Duration::from_millis(50)) {
                Ok(Msg::Frame(2, p)) => {
                    let n = chunk_sizes.pop_front().ok_or_else(|| anyhow!("The Python process sent an unexpected answer"))?;
                    decode_result(&p, n, row_base, &mut results, &mut failures)?;
                    row_base += n as u64;
                    got += 1;
                    if total > 0 {
                        task.set((row_base as f64 / total as f64).min(1.0), Some(format!("{row_base} of {total} rows")));
                    }
                }
                Ok(Msg::Frame(3, _)) => done = true,
                Ok(Msg::Frame(..)) => bail!("The Python process answered out of turn"),
                Ok(Msg::Ended) | Err(mpsc::RecvTimeoutError::Disconnected) => return Err(died(&mut child, &err_text)),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        Ok(RunResult { results, failures, python_version })
    })();
    drop(wtx);
    if outcome.is_err() {
        let _ = child.kill();
    }
    kill_group(&child);
    let _ = child.wait();
    // A script can leave a child running that still holds our pipes (on Windows nothing kills it): never wait for the threads indefinitely.
    for h in [reader, writer, err_thread] {
        join_within(h, Duration::from_secs(2));
    }
    outcome
}

/// Kill the script's whole process group, children it started included (unix; the worker is the group's leader).
fn kill_group(child: &std::process::Child) {
    #[cfg(unix)]
    {
        // SAFETY: killpg with the pid of our own just-spawned child, which called process_group(0), so the group is its own.
        unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
    }
    #[cfg(not(unix))]
    let _ = child;
}

/// Join a helper thread, but give up (leave it running, detached) after `limit`.
fn join_within(h: std::thread::JoinHandle<()>, limit: Duration) {
    let t0 = std::time::Instant::now();
    while !h.is_finished() && t0.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(10));
    }
    if h.is_finished() {
        let _ = h.join();
    }
}

fn progress_seq() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn decode_result(p: &[u8], expect_n: usize, row_base: u64, results: &mut Vec<Res>, failures: &mut Failures) -> Result<()> {
    let bad = || anyhow!("The Python process sent a damaged answer");
    let u32_at = |pos: usize| -> Result<u32> { Ok(u32::from_le_bytes(p.get(pos..pos + 4).ok_or_else(bad)?.try_into().unwrap())) };
    let n = u32_at(0)? as usize;
    let nerr = u32_at(4)? as u64;
    if n != expect_n {
        return Err(bad());
    }
    let tags = p.get(8..8 + n).ok_or_else(bad)?;
    let slots = p.get(8 + n..8 + 9 * n).ok_or_else(bad)?;
    let lens_at = 8 + 9 * n;
    let mut tpos = lens_at + 4 * n;
    let slot = |i: usize| -> [u8; 8] { slots[8 * i..8 * i + 8].try_into().unwrap() };
    for i in 0..n {
        results.push(match tags[i] {
            0 => Res::Null,
            1 => Res::Bool(i64::from_le_bytes(slot(i)) != 0),
            2 => Res::Int(i64::from_le_bytes(slot(i))),
            3 => Res::Float(f64::from_le_bytes(slot(i))),
            4 => {
                let len = u32_at(lens_at + 4 * i)? as usize;
                let s = std::str::from_utf8(p.get(tpos..tpos + len).ok_or_else(bad)?).map_err(|_| bad())?.to_string();
                tpos += len;
                Res::Text(s)
            }
            5 => Res::Date(i64::from_le_bytes(slot(i)) as i32),
            6 => Res::Timestamp(i64::from_le_bytes(slot(i))),
            255 => Res::Failed,
            _ => return Err(bad()),
        });
    }
    let shown = u32_at(tpos)? as usize;
    tpos += 4;
    let mut seen = 0u64;
    for _ in 0..shown {
        let idx = u32_at(tpos)? as u64;
        let len = u32_at(tpos + 4)? as usize;
        let msg = String::from_utf8_lossy(p.get(tpos + 8..tpos + 8 + len).ok_or_else(bad)?).to_string();
        tpos += 8 + len;
        failures.add(row_base + idx + 1, msg);
        seen += 1;
    }
    // Failures beyond the messages the worker kept are still counted.
    failures.count += nerr.saturating_sub(seen);
    Ok(())
}

// ---------- database side ----------

/// Which type the output field gets, from the user's choice (`auto` looks at what the script returned).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutType {
    Auto,
    Integer,
    Double,
    Text,
    Boolean,
    Date,
    Timestamp,
}

impl OutType {
    pub fn parse(s: Option<&str>) -> Result<OutType> {
        Ok(match s.unwrap_or("auto") {
            "auto" => OutType::Auto,
            "integer" => OutType::Integer,
            "double" => OutType::Double,
            "text" => OutType::Text,
            "boolean" => OutType::Boolean,
            "date" => OutType::Date,
            "timestamp" => OutType::Timestamp,
            other => bail!("'{other}' is not a field type (auto, integer, double, text, boolean, date or timestamp)"),
        })
    }
}

fn type_name(t: OutType) -> &'static str {
    match t {
        OutType::Auto => "automatic",
        OutType::Integer => "integer",
        OutType::Double => "decimal",
        OutType::Text => "text",
        OutType::Boolean => "boolean",
        OutType::Date => "date",
        OutType::Timestamp => "date and time",
    }
}

/// `auto`: all-integer results make an integer field, integers with decimals a decimal field, one kind of anything else that kind. When
/// kinds are mixed in a way that cannot be one field, the first non-null value decides and the rows that do not fit fail.
fn infer(results: &[Res]) -> OutType {
    let (mut b, mut i, mut f, mut t, mut d, mut ts) = (false, false, false, false, false, false);
    let mut first = None;
    for r in results {
        match r {
            Res::Bool(_) => b = true,
            Res::Int(_) => i = true,
            Res::Float(_) => f = true,
            Res::Text(_) => t = true,
            Res::Date(_) => d = true,
            Res::Timestamp(_) => ts = true,
            _ => continue,
        }
        first.get_or_insert(match r {
            Res::Bool(_) => OutType::Boolean,
            Res::Int(_) => OutType::Integer,
            Res::Float(_) => OutType::Double,
            Res::Text(_) => OutType::Text,
            Res::Date(_) => OutType::Date,
            _ => OutType::Timestamp,
        });
    }
    let kinds = [b, i || f, t, d, ts].iter().filter(|x| **x).count();
    if kinds == 1 {
        return if b {
            OutType::Boolean
        } else if f {
            OutType::Double
        } else if i {
            OutType::Integer
        } else if t {
            OutType::Text
        } else if d {
            OutType::Date
        } else {
            OutType::Timestamp
        };
    }
    match first {
        // Integers and decimals among other kinds: the numbers still share one decimal field rather than failing the decimals.
        Some(OutType::Integer) if f => OutType::Double,
        Some(t) => t,
        None => OutType::Text,
    }
}

/// The result of running a script over a table, ready for the node to turn into its output with SQL.
pub struct Evaluated {
    /// The input as it was read: its columns plus `__rn` (1-based row number). Drop it when done.
    pub snapshot: String,
    /// `__rn, i BIGINT, f DOUBLE, s VARCHAR` for the rows that have a value (field) or that are kept (filter). Drop it when done.
    pub staging: String,
    pub out_type: OutType,
    pub total: u64,
    pub failures: Failures,
    pub python_version: String,
}

/// Run `script` over every row of `table` (attributes only). Creates the two scratch tables named in the result; on error nothing is left behind.
/// `skip_failed`: rows that failed are left out (filter) or get NULL (field) and are reported, instead of stopping the run.
pub fn evaluate(conn: &Connection, table: &str, script: &str, mode: Mode, out_type: OutType, skip_failed: bool) -> Result<Evaluated> {
    let seq = progress_seq();
    let snapshot = format!("__pyin_{}_{seq}", std::process::id());
    let staging = format!("__pyout_{}_{seq}", std::process::id());
    let r = evaluate_inner(conn, table, script, mode, out_type, skip_failed, &snapshot, &staging);
    if r.is_err() {
        let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}; DROP TABLE IF EXISTS {}", crate::db::ident(&snapshot), crate::db::ident(&staging)));
    }
    r
}

#[allow(clippy::too_many_arguments)]
fn evaluate_inner(conn: &Connection, table: &str, script: &str, mode: Mode, out_type: OutType, skip_failed: bool, snapshot: &str, staging: &str) -> Result<Evaluated> {
    use crate::db::ident;
    // Approval and the interpreter are checked before any work is done on the data.
    if !is_approved(script) {
        bail!("{WAITING}. Review the script and approve it to run it. {TRUST_WARNING}");
    }
    let described: Vec<(String, String)> = {
        let mut st = conn.prepare(&format!("DESCRIBE SELECT * FROM {}", ident(table)))?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let fields: Vec<(String, Kind)> = described.iter().filter(|(n, _)| n != "geom" && !n.starts_with("__")).map(|(n, t)| (n.clone(), Kind::of_sql_type(t))).collect();
    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {} AS SELECT row_number() OVER () AS __rn, * FROM {}", ident(snapshot), ident(table)))?;
    let total: u64 = conn.query_row(&format!("SELECT count(*) FROM {}", ident(snapshot)), [], |r| r.get::<_, i64>(0))? as u64;
    let task = Task::start("Running Python script").cancellable();
    let select = if fields.is_empty() {
        format!("SELECT __rn FROM {} ORDER BY __rn", ident(snapshot))
    } else {
        format!("SELECT {} FROM {} ORDER BY __rn", fields.iter().map(|(n, k)| k.select_expr(n)).collect::<Vec<_>>().join(", "), ident(snapshot))
    };
    let mut stmt = conn.prepare(&select)?;
    let mut rows = stmt.query([])?;
    let mut exhausted = false;
    let next = || -> Result<Option<Chunk>> {
        if exhausted {
            return Ok(None);
        }
        let mut cols: Vec<ColData> = fields.iter().map(|(_, k)| ColData::new(*k)).collect();
        let mut n = 0;
        while n < CHUNK_ROWS {
            let Some(row) = rows.next()? else {
                exhausted = true;
                break;
            };
            for (j, c) in cols.iter_mut().enumerate() {
                match c {
                    ColData::Int(v) => v.push(row.get::<_, Option<i64>>(j)?),
                    ColData::Float(v) => v.push(row.get::<_, Option<f64>>(j)?),
                    ColData::Bool(v) => v.push(row.get::<_, Option<bool>>(j)?),
                    ColData::Date(v) => v.push(row.get::<_, Option<i32>>(j)?),
                    ColData::Timestamp(v) => v.push(row.get::<_, Option<i64>>(j)?),
                    ColData::Text(v) => v.push(row.get::<_, Option<String>>(j)?),
                }
            }
            n += 1;
        }
        Ok(if n == 0 { None } else { Some(Chunk { n, cols }) })
    };
    // With no attribute columns the worker still needs a row count: a chunk of zero columns.
    let run_out = if fields.is_empty() { run_blank(script, mode, total, &task) } else { run(script, mode, &fields, total, &task, next) };
    let mut ran = run_out?;
    drop(rows);
    drop(stmt);
    let mut failures = std::mem::take(&mut ran.failures);

    // Fit what the script returned to the field, strictly.
    let target = match mode {
        Mode::Filter => OutType::Boolean,
        Mode::Field if out_type == OutType::Auto => infer(&ran.results),
        Mode::Field => out_type,
    };
    let fit = |r: &Res| -> Option<&'static str> {
        // None = fits (or is a null / already failed); Some(reason) = does not fit.
        let ok = match (target, r) {
            (_, Res::Null | Res::Failed) => true,
            (OutType::Boolean, Res::Bool(_)) => true,
            (OutType::Integer, Res::Int(_)) => true,
            (OutType::Double, Res::Float(_)) => true,
            (OutType::Double, Res::Int(v)) => v.unsigned_abs() <= (1u64 << 53),
            (OutType::Text, Res::Text(_)) => true,
            (OutType::Date, Res::Date(_)) => true,
            (OutType::Timestamp, Res::Timestamp(_)) => true,
            _ => false,
        };
        if ok {
            None
        } else {
            Some(r.describe())
        }
    };
    let mut keep: Vec<(i64, Res)> = Vec::new();
    for (i, r) in ran.results.iter().enumerate() {
        let rn = i as i64 + 1;
        if let Some(got) = fit(r) {
            failures.add(rn as u64, format!("returned {got}, but the field is {}", type_name(target)));
            continue;
        }
        match (mode, r) {
            (_, Res::Failed) => {}
            (Mode::Filter, Res::Bool(true)) => keep.push((rn, Res::Bool(true))),
            (Mode::Filter, _) => {}
            (Mode::Field, Res::Null) => {}
            (Mode::Field, v) => keep.push((rn, v.clone())),
        }
    }
    if failures.count > 0 && !skip_failed {
        bail!("{}. Nothing was changed. Fix the script, or choose to skip the failed rows to accept the rest.", failures.report(total));
    }
    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {} (__rn BIGINT, i BIGINT, f DOUBLE, s VARCHAR)", ident(staging)))?;
    {
        let mut ap = conn.appender(staging)?;
        for (rn, v) in &keep {
            let (iv, fv, sv): (Option<i64>, Option<f64>, Option<&str>) = match v {
                Res::Bool(b) => (Some(*b as i64), None, None),
                Res::Int(x) if target == OutType::Double => (None, Some(*x as f64), None),
                Res::Int(x) => (Some(*x), None, None),
                Res::Float(x) => (None, Some(*x), None),
                Res::Text(s) => (None, None, Some(s.as_str())),
                Res::Date(d) => (Some(*d as i64), None, None),
                Res::Timestamp(t) => (Some(*t), None, None),
                _ => (None, None, None),
            };
            ap.append_row(duckdb::params![rn, iv, fv, sv])?;
        }
        ap.flush()?;
    }
    Ok(Evaluated { snapshot: snapshot.to_string(), staging: staging.to_string(), out_type: target, total, failures, python_version: std::mem::take(&mut ran.python_version) })
}

/// A table with no attribute columns: run the script on empty rows.
fn run_blank(script: &str, mode: Mode, total: u64, task: &Task) -> Result<RunResult> {
    let mut left = total as usize;
    run(script, mode, &[], total, task, || {
        if left == 0 {
            return Ok(None);
        }
        let n = left.min(CHUNK_ROWS);
        left -= n;
        Ok(Some(Chunk { n, cols: vec![] }))
    })
}

impl OutType {
    /// SQL for the field's value from the staging row `o`.
    pub fn value_sql(self, o: &str) -> String {
        match self {
            OutType::Boolean => format!("({o}.i <> 0)"),
            OutType::Integer => format!("{o}.i"),
            OutType::Double => format!("{o}.f"),
            OutType::Text => format!("{o}.s"),
            OutType::Date => format!("(DATE '1970-01-01' + CAST({o}.i AS INTEGER))"),
            OutType::Timestamp => format!("make_timestamp({o}.i)"),
            OutType::Auto => format!("{o}.s"),
        }
    }
}
