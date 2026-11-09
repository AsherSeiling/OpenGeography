//! DuckDB connection pool. DuckDB connections are not `Sync`, so each worker
//! thread checks one out; all connections share the same database.

use anyhow::Result;
use duckdb::Connection;
use parking_lot::{Condvar, Mutex};
use std::ops::{Deref, DerefMut};
use std::path::Path;

/// Load the spatial extension on a connection: from the copy bundled with the
/// app when there is one (no internet needed), otherwise DuckDB's normal
/// install (dev builds and tests).
pub fn load_spatial(conn: &Connection) -> Result<()> {
    match crate::resources::spatial_extension() {
        Some(path) => conn.execute_batch(&format!("LOAD {}; SET geometry_always_xy = true;", lit(&path.display().to_string())))?,
        None => {
            // Install at most once per process: concurrent INSTALLs into
            // ~/.duckdb collide on Windows ("Could not move file").
            static INSTALLED: Mutex<bool> = Mutex::new(false);
            let mut done = INSTALLED.lock();
            if !*done {
                conn.execute_batch("INSTALL spatial;")?;
                *done = true;
            }
            drop(done);
            conn.execute_batch("LOAD spatial; SET geometry_always_xy = true;")?;
        }
    }
    Ok(())
}

/// Load DuckDB's httpfs extension (reading files over http and S3): the one shipped with the app, or installed on first use in a dev build.
pub fn load_httpfs(conn: &Connection) -> Result<()> {
    match crate::resources::bundled_extension("httpfs") {
        Some(path) => conn.execute_batch(&format!("LOAD {};", lit(&path.display().to_string())))?,
        None => {
            static INSTALLED: Mutex<bool> = Mutex::new(false);
            let mut done = INSTALLED.lock();
            if !*done {
                conn.execute_batch("INSTALL httpfs;").map_err(|e| anyhow::anyhow!("Reading remote files needs DuckDB's httpfs extension, which could not be installed (is there a network connection?): {e}"))?;
                *done = true;
            }
            drop(done);
            conn.execute_batch("LOAD httpfs;")?;
        }
    }
    Ok(())
}

pub struct Pool {
    root: Mutex<Connection>,
    idle: Mutex<Vec<Connection>>,
    gate: std::sync::Arc<Gate>,
    /// The database file this pool has open (None: in memory).
    path: Option<std::path::PathBuf>,
}

/// Counts the connections in use. Whatever a connection is doing (an import, a display table, a model step), it is counted, so Save can wait
/// for the work in flight instead of colliding with it (SAVE-1). The engine owns one for good, so Save can wait without holding the pool.
#[derive(Default)]
pub struct Gate {
    active: Mutex<usize>,
    quiet: Condvar,
}

impl Gate {
    /// Block until no connection is in use, or `until` passes. Returns whether it is quiet. The caller must not hold a connection.
    pub fn wait_quiet(&self, until: std::time::Instant) -> bool {
        let mut n = self.active.lock();
        while *n > 0 {
            if self.quiet.wait_until(&mut n, until).timed_out() {
                return *n == 0;
            }
        }
        true
    }

    /// How many connections are in use right now.
    pub fn in_use(&self) -> usize {
        *self.active.lock()
    }
}

impl Pool {
    pub fn open(path: Option<&Path>, gate: std::sync::Arc<Gate>) -> Result<Self> {
        let root = match path {
            Some(p) => Connection::open(p)?,
            None => Connection::open_in_memory()?,
        };
        load_spatial(&root)?;
        Ok(Self { root: Mutex::new(root), idle: Mutex::new(Vec::new()), gate, path: path.map(|p| p.to_path_buf()) })
    }

    /// Whether this pool has `path` open (the same file under any spelling).
    pub fn has_open(&self, path: &Path) -> bool {
        match (&self.path, std::fs::canonicalize(path)) {
            (Some(p), Ok(q)) => std::fs::canonicalize(p).map(|p| p == q).unwrap_or(false),
            _ => false,
        }
    }

    pub fn get(&self) -> Result<Conn<'_>> {
        let c = match self.idle.lock().pop() {
            Some(c) => c,
            None => {
                let c = self.root.lock().try_clone()?;
                load_spatial(&c)?;
                c
            }
        };
        *self.gate.active.lock() += 1;
        Ok(Conn { conn: Some(c), pool: self })
    }
}

pub struct Conn<'a> {
    conn: Option<Connection>,
    pool: &'a Pool,
}

impl Deref for Conn<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.conn.as_ref().unwrap()
    }
}

impl DerefMut for Conn<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        self.conn.as_mut().unwrap()
    }
}

impl Drop for Conn<'_> {
    fn drop(&mut self) {
        if let Some(c) = self.conn.take() {
            let mut idle = self.pool.idle.lock();
            if idle.len() < 16 {
                idle.push(c);
            }
            drop(idle);
            *self.pool.gate.active.lock() -= 1;
            self.pool.gate.quiet.notify_all();
        }
    }
}

/// Quote an SQL identifier.
pub fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Quote an SQL string literal.
pub fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
