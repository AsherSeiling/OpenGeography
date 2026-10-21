//! DuckDB connection pool. DuckDB connections are not `Sync`, so each worker
//! thread checks one out; all connections share the same database.

use anyhow::Result;
use duckdb::Connection;
use parking_lot::Mutex;
use std::ops::{Deref, DerefMut};
use std::path::Path;

const INIT_SQL: &str = "
    INSTALL spatial;
    LOAD spatial;
    SET geometry_always_xy = true;
";

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
        root.execute_batch(INIT_SQL)?;
        Ok(Self { root: Mutex::new(root), idle: Mutex::new(Vec::new()) })
    }

    pub fn get(&self) -> Result<Conn<'_>> {
        if let Some(c) = self.idle.lock().pop() {
            return Ok(Conn { conn: Some(c), pool: self });
        }
        let c = self.root.lock().try_clone()?;
        c.execute_batch("LOAD spatial; SET geometry_always_xy = true;")?;
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
