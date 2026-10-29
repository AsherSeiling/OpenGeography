//! DuckDB connection pool. DuckDB connections are not `Sync`, so each worker
//! thread checks one out; all connections share the same database.

use anyhow::Result;
use duckdb::Connection;
use parking_lot::Mutex;
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
}

impl Pool {
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let root = match path {
            Some(p) => Connection::open(p)?,
            None => Connection::open_in_memory()?,
        };
        load_spatial(&root)?;
        Ok(Self { root: Mutex::new(root), idle: Mutex::new(Vec::new()) })
    }

    pub fn get(&self) -> Result<Conn<'_>> {
        if let Some(c) = self.idle.lock().pop() {
            return Ok(Conn { conn: Some(c), pool: self });
        }
        let c = self.root.lock().try_clone()?;
        load_spatial(&c)?;
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
